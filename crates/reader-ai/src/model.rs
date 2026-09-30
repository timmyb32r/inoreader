use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CostRates, GenerationMode};

/// AI snapshot extraction preserves archived HTML alongside the exact text used
/// for quote verification. Each HTML text-node boundary becomes one newline.
pub fn article_plain_text(html: &str) -> String {
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("Daily DeepSeek budget reached. Automatic summaries wait until the next Moscow day; manual requests are unavailable.")]
    Budget,
    #[error("resource not found")]
    NotFound,
    #[error("another operation is pending or the operation ID was reused")]
    Conflict,
    #[error("DeepSeek is not enabled for this account")]
    Unavailable,
    #[error("The author-style prompt is awaiting evaluation and approval")]
    PromptPending,
    #[error("Configure a DeepSeek API key in Settings")]
    MissingKey,
    #[error("The provider rejected this key")]
    InvalidKey,
    #[error("The provider account has insufficient balance")]
    Balance,
    #[error("The provider rate limit was reached; retry explicitly later")]
    RateLimit,
    #[error("The provider request failed or was interrupted; it may have been charged")]
    Provider,
    #[error("The provider rejected the configured model or request; no automatic retry was made")]
    Rejected,
    #[error("The provider returned an invalid or incomplete response")]
    Protocol,
    #[error(
        "Не удалось разобрать ответ DeepSeek: {0}. Исходный текст сохранён; попробуйте ещё раз."
    )]
    Translation(&'static str),
    #[error("A proposed verbatim quotation was absent from the article snapshot")]
    Quote,
    #[error("Verification could not be completed. The saved summary is unchanged; retry only when requested.")]
    Review,
    #[error("The complete article and conversation exceed the configured input/context limit; start a new conversation or adjust the limit")]
    Context,
    #[error("The article has no available full text; the RSS excerpt will not be substituted")]
    FullText,
    #[error("Message must be nonempty and within the configured size limit")]
    Message,
    #[error("AI configuration is invalid")]
    Configuration,
    #[error("Credential encryption is unavailable or the stored credential is corrupt")]
    Encryption,
    #[error("AI storage operation failed")]
    Storage,
    #[error("Generation was cancelled")]
    Cancelled,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub available: bool,
    pub balances: Vec<BalanceAmount>,
    pub updated_at: DateTime<Utc>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceAmount {
    pub currency: String,
    pub total: String,
    pub granted: String,
    pub topped_up: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfile {
    pub models: crate::ModelPreferences,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spending: Option<crate::AiSpending>,
    pub configured: bool,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<Balance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_reason: Option<String>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Streaming,
    Complete,
    Interrupted,
    Failed,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatStatus {
    WaitingContent,
    Queued,
    Generating,
    Verifying,
    Completed,
    Failed,
    Interrupted,
    Cancelled,
}
impl ChatStatus {
    pub fn pending(self) -> bool {
        matches!(
            self,
            Self::WaitingContent | Self::Queued | Self::Generating | Self::Verifying
        )
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub status: MessageStatus,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<GenerationPhase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<MessagePurpose>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleChat {
    pub id: Uuid,
    pub article_id: Uuid,
    pub workspace_id: Uuid,
    pub title: String,
    pub source_url: String,
    pub created_at: DateTime<Utc>,
    pub model: String,
    pub prompt_version: String,
    pub status: ChatStatus,
    pub messages: Vec<ChatMessage>,
    pub provider_calls: Vec<ProviderCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Revision is an opaque decimal string: no JavaScript precision loss. A missing
/// chat means unchanged relative to the caller's revision, never missing ownership.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatPoll {
    pub revision: String,
    pub chat: Option<ArticleChat>,
}

/// Private persistent record. The source and prompt are retained verbatim for
/// every conversation version, independently of content-chunk garbage collection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatRecord {
    pub owner: Uuid,
    pub view: ArticleChat,
    pub snapshot: Option<ArticleSnapshot>,
    pub system_prompt: String,
    pub generation_mode: GenerationMode,
    pub cost_rates: CostRates,
    pub max_output_tokens: usize,
    pub limits: crate::InputLimits,
    pub review: crate::ReviewSnapshot,
    pub operations: Vec<Operation>,
}

impl ChatRecord {
    /// Public summaries show a complete, transport-validated first-pass preview
    /// until verification finishes. Preview status remains non-complete; partial
    /// verifier output is never exposed. The original envelope stays persisted
    /// in the operation for retry/provenance, including after a failed check.
    pub fn into_public_view(mut self) -> Result<ArticleChat, AiError> {
        for message in &mut self.view.messages {
            if message.purpose != Some(MessagePurpose::Summary)
                || message.status == MessageStatus::Complete
            {
                continue;
            }
            message.content.clear();
            let operation = self
                .operations
                .iter()
                .find(|op| op.assistant_id == message.id)
                .ok_or(AiError::Storage)?;
            if let AttemptTask::Summary { draft: Some(draft) } = &operation.task {
                let source = self.snapshot.as_ref().ok_or(AiError::Storage)?;
                let mut parser = crate::stream::VerifiedSegments::new(
                    &source.text,
                    self.limits.max_response_bytes,
                );
                parser.push(draft)?;
                message.content = parser.finish()?.to_owned();
            }
        }
        Ok(self.view)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArticleSnapshot {
    pub title: String,
    pub source_url: String,
    pub safe_html: String,
    pub text: String,
    pub source_revision: String,
}

pub enum ArticleInput {
    Waiting { title: String, source_url: String },
    Ready(ArticleSnapshot),
    Failed,
}

/// Exact command payload is retained so an operation ID cannot be reused for a
/// different paid action. Every assistant attempt has its own durable message.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum OperationKind {
    Start { regenerate: bool },
    Message { content: String },
    Retry,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id: Uuid,
    pub kind: OperationKind,
    pub assistant_id: Uuid,
    pub task: AttemptTask,
}

/// Summary drafts are internal account data, never an ArticleChat message. A
/// retry copies the last attempt's task so completed drafts survive failures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttemptTask {
    Summary { draft: Option<String> },
    Reply,
}

impl AttemptTask {
    pub fn phase(&self) -> GenerationPhase {
        match self {
            Self::Summary { draft: Some(_) } => GenerationPhase::Verifying,
            _ => GenerationPhase::Generating,
        }
    }
    pub fn purpose(&self) -> MessagePurpose {
        match self {
            Self::Summary { .. } => MessagePurpose::Summary,
            Self::Reply => MessagePurpose::Chat,
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagePurpose {
    Summary,
    Chat,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationPhase {
    Generating,
    Verifying,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallStatus {
    Started,
    Completed,
    Failed,
    Interrupted,
    Cancelled,
}

/// One potentially billable provider request. No usage means unknown, not zero.
/// Request IDs and assistant IDs retain provenance across explicit retries.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCall {
    pub id: Uuid,
    pub assistant_id: Uuid,
    pub phase: GenerationPhase,
    pub status: CallStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

pub enum CallUpdate {
    Usage(Usage),
    Response {
        content: String,
        system: String,
    },
    Finished {
        status: CallStatus,
        draft: Option<String>,
    },
}

#[derive(Clone)]
pub struct ClaimedChat {
    pub record: ChatRecord,
    pub lease: Uuid,
}

/// Usage comes directly from provider completion metadata. Decimal monetary
/// strings are retained exactly; a missing provider usage event stays unknown.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub prompt_cache_hit_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<String>,
}

/// One immutable provider response per non-streaming attempt. Publication rights
/// can expire without discarding the paid response; retries use a new job id.
#[derive(Clone, Copy)]
pub enum ReplyKind {
    Translation,
    Definitions,
}

#[async_trait]
pub trait AiStore: Send + Sync {
    async fn spending(&self, owner: Uuid, limit: &str) -> Result<crate::AiSpending, AiError>;
    async fn reserve(
        &self,
        owner: Uuid,
        id: Uuid,
        reservation: &crate::SpendReservation,
    ) -> Result<(), AiError>;
    async fn settle(&self, owner: Uuid, id: Uuid, amount: &str) -> Result<(), AiError>;
    async fn enroll_summaries(&self, owners: &[Uuid]) -> Result<(), AiError>;
    async fn next_summary(
        &self,
        owners: &[Uuid],
        attempts: u32,
        retry_seconds: u64,
    ) -> Result<Option<crate::AutoSummary>, AiError>;
    async fn finish_summary(
        &self,
        target: &crate::AutoSummary,
        error: Option<&str>,
    ) -> Result<(), AiError>;
    async fn prioritize(&self, owner: Uuid, chat: Uuid) -> Result<(), AiError>;
    async fn defer_budget(&self, claim: &ClaimedChat) -> Result<(), AiError>;

    async fn retain_reply(
        &self,
        kind: ReplyKind,
        owner: Uuid,
        job: Uuid,
        reply: &crate::ProviderReply,
    ) -> Result<(), AiError>;
    async fn definitions(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Option<crate::DefinitionsJob>, AiError>;
    async fn create_definitions(
        &self,
        record: crate::DefinitionsRecord,
        operation: Uuid,
        regenerate: bool,
    ) -> Result<crate::DefinitionsJob, AiError>;
    async fn claim_definitions(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<crate::ClaimedDefinitions>, AiError>;
    async fn finish_definitions(
        &self,
        claim: &crate::ClaimedDefinitions,
        state: crate::DefinitionState,
        usage: Option<Usage>,
    ) -> Result<(), AiError>;

    async fn translations(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<crate::ParagraphJob>, AiError>;
    async fn create_translation(
        &self,
        record: crate::TranslationRecord,
    ) -> Result<crate::ParagraphJob, AiError>;
    async fn claim_translation(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<crate::ClaimedTranslation>, AiError>;
    async fn finish_translation(
        &self,
        claim: &crate::ClaimedTranslation,
        state: crate::TranslationState,
        usage: Option<Usage>,
    ) -> Result<(), AiError>;

    async fn model_preferences(&self, owner: Uuid) -> Result<crate::ModelPreferences, AiError>;
    async fn save_model_preferences(
        &self,
        owner: Uuid,
        models: crate::ModelPreferences,
    ) -> Result<(), AiError>;

    async fn credential(&self, owner: Uuid) -> Result<Option<Vec<u8>>, AiError>;
    async fn profile(
        &self,
        owner: Uuid,
    ) -> Result<(bool, Option<Balance>, Option<String>), AiError>;
    async fn save_credential(
        &self,
        owner: Uuid,
        encrypted: Vec<u8>,
        balance: Balance,
    ) -> Result<(), AiError>;
    async fn delete_credential(&self, owner: Uuid) -> Result<(), AiError>;
    async fn save_balance(
        &self,
        owner: Uuid,
        expected_credential: &[u8],
        balance: Option<Balance>,
        error: Option<String>,
    ) -> Result<(), AiError>;
    /// Exact membership in the owned article's displayed title or RSS description.
    /// No normalization or arbitrary client text is accepted.
    async fn article_intro_contains(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        source: &str,
    ) -> Result<bool, AiError>;
    async fn article_input(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<ArticleInput, AiError>;
    async fn create_chat(
        &self,
        record: ChatRecord,
        operation: Uuid,
        regenerate: bool,
    ) -> Result<ChatRecord, AiError>;
    async fn chats(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ChatRecord>, AiError>;
    async fn public_chats(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ArticleChat>, AiError>;
    async fn public_chat(
        &self,
        owner: Uuid,
        id: Uuid,
        after: Option<&str>,
    ) -> Result<ChatPoll, AiError>;
    async fn chat(&self, owner: Uuid, id: Uuid) -> Result<ChatRecord, AiError>;
    async fn append(
        &self,
        owner: Uuid,
        id: Uuid,
        operation: Uuid,
        kind: OperationKind,
    ) -> Result<ChatRecord, AiError>;
    async fn stop(&self, owner: Uuid, id: Uuid) -> Result<ChatRecord, AiError>;
    /// Expired in-flight leases become interrupted, never automatically retried.
    async fn claim(&self, lease_seconds: u64) -> Result<Option<ClaimedChat>, AiError>;
    async fn active(&self, owner: Uuid, id: Uuid, lease: Uuid) -> Result<bool, AiError>;
    async fn begin_call(
        &self,
        claim: &ClaimedChat,
        phase: GenerationPhase,
        reservation: &crate::SpendReservation,
        model: &crate::CallModel,
    ) -> Result<Uuid, AiError>;
    /// Billing is recorded even after cancellation/lease expiry; publication
    /// remains fenced. Only an existing request owned by this attempt is updated.
    async fn update_call(
        &self,
        claim: &ClaimedChat,
        id: Uuid,
        update: CallUpdate,
    ) -> Result<(), AiError>;
    /// Atomically publish a retained, validated draft only when fact-check is
    /// currently disabled. Returns false when enabled, without changing state.
    /// Requires the active lease and a completed generation; makes no paid call.
    async fn finish_unchecked(&self, claim: &ClaimedChat) -> Result<bool, AiError>;
    async fn update_claim(
        &self,
        claim: &ClaimedChat,
        status: ChatStatus,
        content: Option<&str>,
        snapshot: Option<&ArticleSnapshot>,
        error: Option<&str>,
    ) -> Result<(), AiError>;
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn definitions(
        &self,
        key: &str,
        input: crate::DefinitionsInput,
    ) -> Result<crate::ProviderReply, AiError>;

    async fn translate(
        &self,
        key: &str,
        input: crate::TranslationInput,
    ) -> Result<crate::ProviderReply, AiError>;

    async fn balance(&self, key: &str) -> Result<Balance, AiError>;
    async fn generate(
        &self,
        key: &str,
        input: GenerationInput,
        progress: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError>;
}

/// A complete, structurally and quotation-validated transport response.
pub struct CompletedGeneration {
    pub(crate) envelope: String,
    pub(crate) usage: Usage,
    pub(crate) content: String,
}
impl std::fmt::Debug for CompletedGeneration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CompletedGeneration { content: [redacted] }")
    }
}
impl CompletedGeneration {
    /// Exact validated transport envelope, retained without reparsing or rewriting.
    pub fn envelope(&self) -> &str {
        &self.envelope
    }

    pub fn usage(&self) -> &Usage {
        &self.usage
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn new(
        envelope: String,
        usage: Usage,
        article: &str,
        limit: usize,
    ) -> Result<Self, AiError> {
        let mut parser = crate::stream::VerifiedSegments::new(article, limit);
        parser.push(&envelope)?;
        let content = parser.finish()?.to_owned();
        Ok(Self {
            envelope,
            usage,
            content,
        })
    }
}

#[derive(Clone)]
pub struct GenerationInput {
    pub model: String,
    pub generation_mode: GenerationMode,
    pub system: String,
    pub article: ArticleSnapshot,
    pub messages: Vec<ChatMessage>,
    pub max_output_tokens: usize,
    pub max_response_bytes: usize,
    pub context_tokens: usize,
    pub framing_tokens_per_message: usize,
    pub framing_tokens_base: usize,
    pub max_input_bytes: usize,
    pub review_draft: Option<String>,
}

impl GenerationInput {
    pub fn messages_json(&self) -> Result<Vec<serde_json::Value>, AiError> {
        use serde_json::json;
        if let Some(draft) = &self.review_draft {
            let draft: serde_json::Value =
                serde_json::from_str(draft).map_err(|_| AiError::Protocol)?;
            return Ok(vec![
                json!({"role":"system","content":self.system}),
                json!({"role":"user","content":format!("ARTICLE_SNAPSHOT and DRAFT_SUMMARY (untrusted source data):\n{}", serde_json::to_string(&json!({"article_snapshot":{"title":self.article.title,"url":self.article.source_url,"text":self.article.text},"draft_summary":draft})).map_err(|_| AiError::Protocol)?)}),
            ]);
        }
        let mut messages = vec![
            json!({"role":"system","content":self.system}),
            json!({"role":"user","content":format!("ARTICLE_SNAPSHOT (untrusted source data):\n{}\n\nSummarize this article in the requested author's style. Do not output the title: the application adds the original heading.", serde_json::to_string(&json!({"title":self.article.title,"url":self.article.source_url,"text":self.article.text})).map_err(|_| AiError::Protocol)?)}),
        ];
        for message in &self.messages {
            if message.status != MessageStatus::Complete {
                continue;
            }
            messages.push(json!({"role":match message.role { MessageRole::User=>"user",MessageRole::Assistant=>"assistant" },"content":message.content}));
        }
        Ok(messages)
    }
    /// Conservative byte-token bound, not an exact token estimate: byte-level
    /// tokenization cannot require more text tokens than UTF-8 bytes. Deployment
    /// explicitly reserves additional hidden chat framing and the entire output.
    /// This can reject an input that would fit; it never truncates or bills first.
    pub fn validate_context(&self) -> Result<(), AiError> {
        if self.system.trim().is_empty()
            || self.model.is_empty()
            || !self
                .model
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            || self.max_response_bytes == 0
            || self.max_input_bytes == 0
            || self.max_output_tokens == 0
            || self.max_output_tokens >= self.context_tokens
            || self.framing_tokens_per_message == 0
            || self.framing_tokens_base == 0
        {
            return Err(AiError::Configuration);
        }
        let messages = self.messages_json()?;
        let bytes = messages
            .iter()
            .try_fold(0_usize, |sum, m| {
                sum.checked_add(m["content"].as_str()?.len())
            })
            .ok_or(AiError::Context)?;
        let framing = messages
            .len()
            .checked_mul(self.framing_tokens_per_message)
            .and_then(|n| n.checked_add(self.framing_tokens_base))
            .ok_or(AiError::Context)?;
        let bound = bytes
            .checked_add(framing)
            .and_then(|n| n.checked_add(self.max_output_tokens))
            .ok_or(AiError::Context)?;
        if bytes > self.max_input_bytes || bound > self.context_tokens {
            return Err(AiError::Context);
        }
        Ok(())
    }
}

#[async_trait]
pub trait GenerationProgress {
    async fn check_active(&mut self) -> Result<(), AiError>;
    async fn publish(&mut self, verified_content: &str) -> Result<(), AiError>;
    async fn response(&mut self, _content: &str, _system: &str) -> Result<(), AiError> {
        Ok(())
    }
    async fn usage(&mut self, _usage: &Usage) -> Result<(), AiError> {
        Ok(())
    }
}

/// Exact decimal rates, bounded only by checked integer arithmetic. Unsupported
/// precision/overflow fails explicitly; there is no float conversion or rounding.
pub struct DecimalRate {
    integer: u128,
    scale: u32,
}
impl DecimalRate {
    pub fn parse(value: &str) -> Result<Self, AiError> {
        let mut parts = value.split('.');
        let whole = parts.next().ok_or(AiError::Configuration)?;
        let fractional = parts.next().unwrap_or("");
        if parts.next().is_some()
            || whole.is_empty()
            || !whole
                .bytes()
                .chain(fractional.bytes())
                .all(|b| b.is_ascii_digit())
        {
            return Err(AiError::Configuration);
        }
        let scale = u32::try_from(fractional.len()).map_err(|_| AiError::Configuration)?;
        10_u128
            .checked_pow(scale + 6)
            .ok_or(AiError::Configuration)?;
        let integer = format!("{whole}{fractional}")
            .parse()
            .map_err(|_| AiError::Configuration)?;
        Ok(Self { integer, scale })
    }
    pub fn cost(rates: &[(&str, u64)]) -> Result<String, AiError> {
        let rates = rates
            .iter()
            .map(|(rate, tokens)| Ok((Self::parse(rate)?, *tokens)))
            .collect::<Result<Vec<_>, AiError>>()?;
        let scale = rates
            .iter()
            .map(|(r, _)| r.scale + 6)
            .max()
            .ok_or(AiError::Configuration)?;
        let mut total = 0_u128;
        for (rate, tokens) in rates {
            let product = rate
                .integer
                .checked_mul(u128::from(tokens))
                .and_then(|v| v.checked_mul(10_u128.checked_pow(scale - rate.scale - 6)?))
                .ok_or(AiError::Configuration)?;
            total = total.checked_add(product).ok_or(AiError::Configuration)?;
        }
        let divisor = 10_u128.checked_pow(scale).ok_or(AiError::Configuration)?;
        Ok(format!(
            "{}.{:0width$}",
            total / divisor,
            total % divisor,
            width = scale as usize
        ))
    }
}
