use crate::{BotToken, GlossaryError, GlossaryPolicy, ObservedPost, CHANNEL_USERNAME};
use async_trait::async_trait;
use http::{header, HeaderMap, HeaderValue, Method};
use reader_web_runtime::{
    DnsResolver, ExternalRequestObserver, OutboundHttpClient, OutboundTransport, PreparedRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "BindingWire", rename_all = "camelCase")]
pub struct ChannelBinding {
    bot_id: i64,
    bot_username: String,
    channel_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BindingWire {
    bot_id: i64,
    bot_username: String,
    channel_id: i64,
}
impl TryFrom<BindingWire> for ChannelBinding {
    type Error = GlossaryError;
    fn try_from(v: BindingWire) -> Result<Self, Self::Error> {
        Self::new(v.bot_id, v.bot_username, v.channel_id)
    }
}
impl ChannelBinding {
    pub fn new(bot_id: i64, bot_username: String, channel_id: i64) -> Result<Self, GlossaryError> {
        if bot_id <= 0
            || channel_id >= 0
            || bot_username.is_empty()
            || !bot_username
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(GlossaryError::Identity);
        }
        Ok(Self {
            bot_id,
            bot_username,
            channel_id,
        })
    }
    pub fn bot_id(&self) -> i64 {
        self.bot_id
    }
    pub fn bot_username(&self) -> &str {
        &self.bot_username
    }
    pub fn channel_id(&self) -> i64 {
        self.channel_id
    }
}

/// The raw response must be committed before next_offset can be acknowledged.
pub struct UpdateBatch {
    raw: String,
    updates: Vec<Value>,
    next_offset: Option<i64>,
}
impl UpdateBatch {
    pub fn new(raw: String) -> Result<Self, GlossaryError> {
        let value: Value = serde_json::from_str(&raw).map_err(|_| GlossaryError::Protocol)?;
        let result = api_result(&value)?;
        let updates = result.as_array().ok_or(GlossaryError::Protocol)?.clone();
        let mut previous = None;
        for update in &updates {
            let id = update["update_id"]
                .as_i64()
                .filter(|id| *id >= 0)
                .ok_or(GlossaryError::Protocol)?;
            if previous.is_some_and(|last| last >= id) {
                return Err(GlossaryError::Conflict);
            }
            previous = Some(id);
        }
        let next_offset = previous
            .map(|id| id.checked_add(1).ok_or(GlossaryError::Protocol))
            .transpose()?;
        Ok(Self {
            raw,
            updates,
            next_offset,
        })
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn updates(&self) -> &[Value] {
        &self.updates
    }
    pub fn next_offset(&self) -> Option<i64> {
        self.next_offset
    }
}

pub struct PublicPage {
    pub raw: String,
    pub posts: Vec<ObservedPost>,
    pub before: Option<i64>,
}

#[async_trait]
pub trait TelegramGateway: Send + Sync {
    async fn discover(&self, token: &BotToken) -> Result<ChannelBinding, GlossaryError>;
    async fn updates(
        &self,
        token: &BotToken,
        offset: Option<i64>,
    ) -> Result<UpdateBatch, GlossaryError>;
    async fn history(&self, before: Option<i64>) -> Result<PublicPage, GlossaryError>;
}

pub struct TelegramClient<R, T, O> {
    api: OutboundHttpClient<R, T, O>,
    public: OutboundHttpClient<R, T, O>,
    policy: GlossaryPolicy,
}
impl<R: DnsResolver, T: OutboundTransport, O: ExternalRequestObserver> TelegramClient<R, T, O> {
    pub fn new(
        api: OutboundHttpClient<R, T, O>,
        public: OutboundHttpClient<R, T, O>,
        policy: GlossaryPolicy,
    ) -> Result<Self, GlossaryError> {
        let origin =
            Url::parse("https://api.telegram.org").map_err(|_| GlossaryError::Configuration)?;
        Ok(Self {
            api: api
                .restricted_to_origin(&origin)
                .map_err(|_| GlossaryError::Configuration)?,
            public,
            policy,
        })
    }
    async fn call(
        &self,
        token: &BotToken,
        method: &'static str,
        parameters: Value,
    ) -> Result<String, GlossaryError> {
        let url = Url::parse(&format!(
            "https://api.telegram.org/bot{}/{method}",
            token.expose()
        ))
        .map_err(|_| GlossaryError::Identity)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        let request = PreparedRequest {
            method: Method::POST,
            url,
            headers,
            body: Some(serde_json::to_vec(&parameters).map_err(|_| GlossaryError::Protocol)?),
        };
        let mut response = self
            .api
            .execute_stream(request, "telegram", method)
            .await
            .map_err(|_| GlossaryError::Transport)?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .next_chunk()
            .await
            .map_err(|_| GlossaryError::Transport)?
        {
            bytes.extend(chunk);
        }
        let raw = String::from_utf8(bytes).map_err(|_| GlossaryError::Protocol)?;
        let value: Value = serde_json::from_str(&raw).map_err(|_| GlossaryError::Protocol)?;
        api_result(&value)?;
        if !response.status.is_success() {
            return Err(GlossaryError::Transport);
        }
        Ok(raw)
    }
    async fn result(
        &self,
        token: &BotToken,
        method: &'static str,
        parameters: Value,
    ) -> Result<Value, GlossaryError> {
        let value: Value = serde_json::from_str(&self.call(token, method, parameters).await?)
            .map_err(|_| GlossaryError::Protocol)?;
        Ok(api_result(&value)?.clone())
    }
}

#[async_trait]
impl<R, T, O> TelegramGateway for TelegramClient<R, T, O>
where
    R: DnsResolver + 'static,
    T: OutboundTransport + 'static,
    O: ExternalRequestObserver + 'static,
{
    async fn discover(&self, token: &BotToken) -> Result<ChannelBinding, GlossaryError> {
        let me = self.result(token, "getMe", json!({})).await?;
        if me["is_bot"].as_bool() != Some(true) {
            return Err(GlossaryError::Identity);
        }
        let id = me["id"].as_i64().ok_or(GlossaryError::Identity)?;
        let webhook = self.result(token, "getWebhookInfo", json!({})).await?;
        if webhook["url"].as_str() != Some("") {
            return Err(GlossaryError::ReceiverConflict);
        }
        let chat = self
            .result(
                token,
                "getChat",
                json!({"chat_id":format!("@{CHANNEL_USERNAME}")}),
            )
            .await?;
        if chat["type"].as_str() != Some("channel")
            || chat["username"].as_str() != Some(CHANNEL_USERNAME)
        {
            return Err(GlossaryError::Identity);
        }
        let channel = chat["id"].as_i64().ok_or(GlossaryError::Identity)?;
        let member = self
            .result(
                token,
                "getChatMember",
                json!({"chat_id":channel,"user_id":id}),
            )
            .await?;
        if !matches!(member["status"].as_str(), Some("administrator" | "member")) {
            return Err(GlossaryError::NotConnected);
        }
        ChannelBinding::new(
            id,
            me["username"]
                .as_str()
                .ok_or(GlossaryError::Identity)?
                .into(),
            channel,
        )
    }
    async fn updates(
        &self,
        token: &BotToken,
        offset: Option<i64>,
    ) -> Result<UpdateBatch, GlossaryError> {
        let raw = self.call(token, "getUpdates", json!({"offset":offset,"timeout":self.policy.config().polling_timeout_seconds,"limit":self.policy.config().batch_size,"allowed_updates":["channel_post","edited_channel_post","my_chat_member"]})).await?;
        UpdateBatch::new(raw)
    }
    async fn history(&self, before: Option<i64>) -> Result<PublicPage, GlossaryError> {
        let mut url = Url::parse(&format!("https://t.me/s/{CHANNEL_USERNAME}"))
            .map_err(|_| GlossaryError::Identity)?;
        if let Some(before) = before {
            url.query_pairs_mut()
                .append_pair("before", &before.to_string());
        }
        let request = PreparedRequest {
            method: Method::GET,
            url,
            headers: HeaderMap::new(),
            body: None,
        };
        let mut response = self
            .public
            .execute_stream(request, "telegram_public", "history")
            .await
            .map_err(|_| GlossaryError::Transport)?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .next_chunk()
            .await
            .map_err(|_| GlossaryError::Transport)?
        {
            bytes.extend(chunk);
        }
        if !response.status.is_success() {
            return Err(GlossaryError::Transport);
        }
        crate::parse_public_page(
            String::from_utf8(bytes).map_err(|_| GlossaryError::Protocol)?,
            before,
        )
    }
}

fn api_result(value: &Value) -> Result<&Value, GlossaryError> {
    if value["ok"].as_bool() != Some(true) {
        if let Some(retry) = value["parameters"]["retry_after"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= i64::MAX as u64)
        {
            return Err(GlossaryError::RateLimited(retry));
        }
        return Err(if value["error_code"].as_u64() == Some(409) {
            GlossaryError::ReceiverConflict
        } else {
            GlossaryError::Transport
        });
    }
    value.get("result").ok_or(GlossaryError::Protocol)
}
