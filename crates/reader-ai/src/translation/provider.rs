use super::*;
use crate::{
    provider::{check_status, request},
    DeepSeekProvider,
};
use reader_web_runtime::{DnsResolver, ExternalRequestObserver, OutboundTransport};

impl<R, T, O> DeepSeekProvider<R, T, O>
where
    R: DnsResolver + 'static,
    T: OutboundTransport + 'static,
    O: ExternalRequestObserver + 'static,
{
    pub(crate) async fn translate_paragraph(
        &self,
        key: &str,
        input: TranslationInput,
    ) -> Result<(ParagraphTranslation, Usage), AiError> {
        let mut response = self
            .http
            .execute_stream(
                request("/chat/completions", key, Some(input.body))?,
                "deepseek",
                "paragraph_translation",
            )
            .await
            .map_err(|_| AiError::Provider)?;
        check_status(response.status)?;
        // The shared outbound boundary enforces the configured response byte
        // limit and overall deadline on every chunk, including redirects.
        let mut bytes = Vec::new();
        while let Some(chunk) = response.next_chunk().await.map_err(|_| AiError::Provider)? {
            bytes.extend(chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| AiError::Protocol)?;
        let choices = value["choices"].as_array().ok_or(AiError::Protocol)?;
        if choices.len() != 1 {
            return Err(AiError::Protocol);
        }
        if choices[0]["finish_reason"] != "stop" {
            return Err(AiError::Context);
        }
        let result = ParagraphTranslation::from_response(
            &input.source,
            choices[0]["message"]["content"]
                .as_str()
                .ok_or(AiError::Protocol)?,
        )?;
        let raw = &value["usage"];
        let prompt = raw["prompt_tokens"].as_u64().ok_or(AiError::Protocol)?;
        let hit = raw["prompt_cache_hit_tokens"].as_u64().unwrap_or(0);
        let miss = raw["prompt_cache_miss_tokens"]
            .as_u64()
            .unwrap_or(prompt.checked_sub(hit).ok_or(AiError::Protocol)?);
        if hit.checked_add(miss) != Some(prompt) {
            return Err(AiError::Protocol);
        }
        Ok((
            result,
            Usage {
                prompt_tokens: prompt,
                completion_tokens: raw["completion_tokens"].as_u64().ok_or(AiError::Protocol)?,
                prompt_cache_hit_tokens: hit,
                prompt_cache_miss_tokens: miss,
                estimated_cost_usd: None,
            },
        ))
    }
}
