use crate::{ChannelBinding, Definition, GlossaryError, PublicPage, UpdateBatch};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelStatus {
    pub channel: String,
    pub configured: bool,
    pub bot_username: Option<String>,
    pub index_ready: bool,
    pub revision: i64,
    pub posts: i64,
    pub definitions: i64,
    pub unindexed: i64,
    pub pending: i64,
    pub conflicts: i64,
    pub last_poll: Option<DateTime<Utc>>,
    pub last_history: Option<DateTime<Utc>>,
    pub poll_error: Option<String>,
    pub history_error: Option<String>,
    pub history_incomplete: bool,
    pub coverage_note: String,
    pub sync_pending: bool,
    pub generation_allowed: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownDefinition {
    pub definition: Definition,
    pub permalink: String,
    pub published_at: Option<DateTime<Utc>>,
    pub stale: bool,
}

pub struct PollClaim {
    pub owner: Uuid,
    pub workspace: Uuid,
    pub binding: ChannelBinding,
    pub encrypted_token: Vec<u8>,
    pub offset: Option<i64>,
    pub lease: Uuid,
}
pub struct HistoryClaim {
    pub owner: Uuid,
    pub workspace: Uuid,
    pub before: Option<i64>,
    pub lease: Uuid,
}

#[async_trait]
pub trait GlossaryStore: Send + Sync {
    async fn status(&self, owner: Uuid, workspace: Uuid) -> Result<ChannelStatus, GlossaryError>;
    async fn configure(
        &self,
        owner: Uuid,
        workspace: Uuid,
        binding: &ChannelBinding,
        encrypted_token: Vec<u8>,
    ) -> Result<(), GlossaryError>;
    async fn request_sync(&self, owner: Uuid, workspace: Uuid) -> Result<(), GlossaryError>;
    async fn lookup(
        &self,
        owner: Uuid,
        workspace: Uuid,
        terms: &[String],
    ) -> Result<Vec<KnownDefinition>, GlossaryError>;
    async fn claim_poll(&self, lease_seconds: u64) -> Result<Option<PollClaim>, GlossaryError>;
    /// This commit is the only authority for advancing the next getUpdates offset.
    async fn commit_batch(
        &self,
        claim: &PollClaim,
        batch: &UpdateBatch,
    ) -> Result<(), GlossaryError>;
    async fn finish_poll(
        &self,
        claim: &PollClaim,
        error: Option<&str>,
        retry_seconds: u64,
    ) -> Result<(), GlossaryError>;
    async fn apply_pending(&self, batch_size: u16) -> Result<usize, GlossaryError>;
    async fn claim_history(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<HistoryClaim>, GlossaryError>;
    async fn commit_page(
        &self,
        claim: &HistoryClaim,
        page: &PublicPage,
        next_delay_seconds: u64,
    ) -> Result<(), GlossaryError>;
    async fn fail_history(
        &self,
        claim: &HistoryClaim,
        error: &str,
        retry_seconds: u64,
    ) -> Result<(), GlossaryError>;
}

/// Composition supplies the existing authenticated encryption implementation.
/// The glossary domain neither depends on AI nor implements its own cipher.
pub trait GlossaryVault: Send + Sync {
    fn seal(&self, owner: Uuid, value: &str) -> Result<Vec<u8>, GlossaryError>;
    fn open(&self, owner: Uuid, value: &[u8]) -> Result<String, GlossaryError>;
}
