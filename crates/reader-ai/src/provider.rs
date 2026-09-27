use async_trait::async_trait;
use chrono::Utc;
use http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use reader_web_runtime::{
    DnsResolver, ExternalRequestObserver, OutboundHttpClient, OutboundTransport, PreparedRequest,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use url::Url;

use crate::{
    stream::VerifiedSegments, AiError, AiProvider, Balance, BalanceAmount, CompletedGeneration,
    GenerationInput, GenerationMode, GenerationProgress, Usage,
};

pub const TRANSPORT_INSTRUCTION: &str =
    include_str!("../../../prompts/reading-data-news/transport.md");

pub struct DeepSeekProvider<R, T, O> {
    pub(crate) http: OutboundHttpClient<R, T, O>,
    cancellation_poll: Duration,
}
impl<R, T, O> DeepSeekProvider<R, T, O> {
    pub fn new(
        http: OutboundHttpClient<R, T, O>,
        cancellation_poll: Duration,
    ) -> Result<Self, AiError> {
        if cancellation_poll.is_zero() {
            return Err(AiError::Configuration);
        }
        Ok(Self {
            http,
            cancellation_poll,
        })
    }
}

pub(crate) fn request(
    path: &str,
    key: &str,
    body: Option<Value>,
) -> Result<PreparedRequest, AiError> {
    if key.is_empty() {
        return Err(AiError::InvalidKey);
    }
    let mut authorization =
        HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| AiError::InvalidKey)?;
    authorization.set_sensitive(true);
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    Ok(PreparedRequest {
        method: if body.is_some() {
            Method::POST
        } else {
            Method::GET
        },
        url: Url::parse(&format!("https://api.deepseek.com{path}"))
            .map_err(|_| AiError::Configuration)?,
        headers,
        body: body
            .map(|v| serde_json::to_vec(&v))
            .transpose()
            .map_err(|_| AiError::Protocol)?,
    })
}

pub(crate) fn check_status(status: StatusCode) -> Result<(), AiError> {
    if status.is_success() {
        return Ok(());
    }
    Err(match status.as_u16() {
        401 | 403 => AiError::InvalidKey,
        402 => AiError::Balance,
        429 => AiError::RateLimit,
        413 => AiError::Context,
        400 | 422 => AiError::Rejected,
        _ => AiError::Provider,
    })
}

#[async_trait]
impl<R, T, O> AiProvider for DeepSeekProvider<R, T, O>
where
    R: DnsResolver + 'static,
    T: OutboundTransport + 'static,
    O: ExternalRequestObserver + 'static,
{
    async fn definitions(
        &self,
        key: &str,
        input: crate::DefinitionsInput,
    ) -> Result<crate::ProviderReply, AiError> {
        self.define_entities(key, input).await
    }
    async fn translate(
        &self,
        key: &str,
        input: crate::TranslationInput,
    ) -> Result<crate::ProviderReply, AiError> {
        self.translate_paragraph(key, input).await
    }

    async fn balance(&self, key: &str) -> Result<Balance, AiError> {
        let mut response = self
            .http
            .execute_stream(request("/user/balance", key, None)?, "deepseek", "balance")
            .await
            .map_err(|_| AiError::Provider)?;
        check_status(response.status)?;
        let mut body = Vec::new();
        while let Some(chunk) = response.next_chunk().await.map_err(|_| AiError::Provider)? {
            body.extend(chunk);
        }
        #[derive(Deserialize)]
        struct WireBalance {
            is_available: bool,
            balance_infos: Vec<WireAmount>,
        }
        #[derive(Deserialize)]
        struct WireAmount {
            currency: String,
            total_balance: String,
            granted_balance: String,
            topped_up_balance: String,
        }
        let value: WireBalance = serde_json::from_slice(&body).map_err(|_| AiError::Protocol)?;
        Ok(Balance {
            available: value.is_available,
            balances: value
                .balance_infos
                .into_iter()
                .map(|v| BalanceAmount {
                    currency: v.currency,
                    total: v.total_balance,
                    granted: v.granted_balance,
                    topped_up: v.topped_up_balance,
                })
                .collect(),
            updated_at: Utc::now(),
        })
    }

    async fn generate(
        &self,
        key: &str,
        input: GenerationInput,
        progress: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError> {
        input.validate_context()?;
        let messages = input.messages_json()?;
        let mut body = json!({"model":input.model,"messages":messages,"max_tokens":input.max_output_tokens,"stream":true,"stream_options":{"include_usage":true},"response_format":{"type":"json_object"}});
        match input.generation_mode {
            GenerationMode::Standard { temperature } => {
                body["temperature"] = json!(temperature.value());
                body["thinking"] = json!({"type":"disabled"});
            }
            GenerationMode::Thinking { effort } => {
                body["thinking"] = json!({"type":"enabled"});
                body["reasoning_effort"] = json!(effort);
            }
        }
        progress.check_active().await?;
        let started = Instant::now();
        let mut cancel = tokio::time::interval(self.cancellation_poll);
        let opening = self.http.execute_stream(
            request("/chat/completions", key, Some(body))?,
            "deepseek",
            "chat_completion",
        );
        tokio::pin!(opening);
        let mut response = loop {
            tokio::select! {
                response=&mut opening => break response.map_err(|_| AiError::Provider)?,
                _=cancel.tick()=>progress.check_active().await?,
            }
        };
        check_status(response.status)?;
        let mut parser = VerifiedSegments::new(&input.article.text, input.max_response_bytes);
        let mut pending = Vec::new();
        let mut data = String::new();
        let mut done = false;
        let mut stopped = false;
        let mut usage = None;
        let mut first = true;
        loop {
            let chunk = tokio::select! {
                chunk = response.next_chunk() => chunk.map_err(|_| AiError::Provider)?,
                _ = cancel.tick() => { progress.check_active().await?; continue; }
            };
            let Some(chunk) = chunk else {
                break;
            };
            pending.extend(chunk);
            while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                let bytes: Vec<u8> = pending.drain(..=end).collect();
                let line = std::str::from_utf8(&bytes)
                    .map_err(|_| AiError::Protocol)?
                    .trim_end_matches(['\r', '\n']);
                if let Some(value) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(value.strip_prefix(' ').unwrap_or(value));
                    continue;
                }
                if !line.is_empty() {
                    continue;
                }
                if data.is_empty() {
                    continue;
                }
                if data == "[DONE]" {
                    done = true;
                    data.clear();
                    continue;
                }
                if done {
                    return Err(AiError::Protocol);
                }
                let event: Value = serde_json::from_str(&data).map_err(|_| AiError::Protocol)?;
                data.clear();
                if event.get("error").is_some() {
                    return Err(AiError::Provider);
                }
                if let Some(value) = event.get("usage").filter(|v| !v.is_null()) {
                    let prompt = value
                        .get("prompt_tokens")
                        .and_then(Value::as_u64)
                        .ok_or(AiError::Protocol)?;
                    let hit = value
                        .get("prompt_cache_hit_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let miss = value
                        .get("prompt_cache_miss_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(prompt.checked_sub(hit).ok_or(AiError::Protocol)?);
                    if hit.checked_add(miss) != Some(prompt) {
                        return Err(AiError::Protocol);
                    }
                    usage = Some(Usage {
                        prompt_tokens: prompt,
                        completion_tokens: value
                            .get("completion_tokens")
                            .and_then(Value::as_u64)
                            .ok_or(AiError::Protocol)?,
                        prompt_cache_hit_tokens: hit,
                        prompt_cache_miss_tokens: miss,
                        estimated_cost_usd: None,
                    });
                    progress
                        .usage(usage.as_ref().ok_or(AiError::Protocol)?)
                        .await?;
                }
                for choice in event
                    .get("choices")
                    .and_then(Value::as_array)
                    .ok_or(AiError::Protocol)?
                {
                    if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                        if reason != "stop" {
                            return Err(AiError::Context);
                        }
                        stopped = true;
                    }
                    // Reasoning deltas are intentionally never published, retained
                    // in conversation history, or logged. Completion-token usage
                    // already includes them and must not be narrowed to visible text.
                    if let Some(text) = choice.pointer("/delta/content").and_then(Value::as_str) {
                        if first && !text.is_empty() {
                            log::info!(
                                "ai_first_delta elapsed_ms={}",
                                started.elapsed().as_millis()
                            );
                            first = false;
                        }
                        if let Some(content) = parser.push(text)? {
                            progress.publish(content).await?;
                        }
                    }
                }
            }
        }
        if !done || !stopped || !data.is_empty() || pending.iter().any(|b| !b.is_ascii_whitespace())
        {
            return Err(AiError::Protocol);
        }
        let content = parser.finish()?.to_owned();
        Ok(CompletedGeneration {
            envelope: parser.envelope().to_owned(),
            usage: usage.ok_or(AiError::Protocol)?,
            content,
        })
    }
}
