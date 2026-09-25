use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_core::{
    ArticleId, PendingRuleEvaluation, RuleId, SourceId, SourceRecordId, SubscriptionId, WorkspaceId,
};
use thiserror::Error;
use url::Url;

use crate::{
    BrowserCapability, CacheValidators, ContentRevision, DeliveryCommit, DeliveryResult,
    DeliveryTarget, JobId, LeaseToken, LeasedWork, PollCommit, SourceDefinition, SourceRecord,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchedPage {
    pub final_url: Url,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub validators: CacheValidators,
    pub not_modified: bool,
}

#[derive(Clone, Debug, Error)]
pub enum FetchError {
    #[error("outbound request was rejected: {0}")]
    Rejected(String),
    #[error("remote server returned HTTP {0}")]
    Http(u16),
}

#[async_trait]
pub trait FeedFetcher: Send + Sync {
    async fn fetch(
        &self,
        url: &Url,
        validators: &CacheValidators,
    ) -> Result<FetchedPage, FetchError>;
}

#[async_trait]
pub trait FullTextExtractor: Send + Sync {
    async fn extract(&self, url: &Url) -> Result<FetchedPage, FetchError>;
}

#[async_trait]
pub trait BrowserCollector: Send + Sync {
    fn capability(&self) -> BrowserCapability;
    async fn collect(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError>;
}

/// Explicit adapter used when the configured CDP endpoint failed its startup
/// probe. It keeps ordinary feeds operational while every Web-feed job remains
/// durable and retryable with a visible degraded diagnostic.
#[derive(Clone, Copy, Debug, Default)]
pub struct DegradedBrowserCollector;
#[async_trait]
impl BrowserCollector for DegradedBrowserCollector {
    fn capability(&self) -> BrowserCapability {
        BrowserCapability::Degraded
    }
    async fn collect(&self, _: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        Err(FetchError::Rejected("browser_degraded".to_owned()))
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StoreError {
    #[error("record not found")]
    NotFound,
    #[error("lease is stale or belongs to another worker")]
    StaleLease,
    #[error("concurrent revision conflict")]
    Conflict,
    #[error("storage unavailable: {0}")]
    Unavailable(String),
}

/// YDB is the source of truth for all methods. Implementations must use the
/// lease token as a fencing token. `commit_poll` atomically upserts immutable
/// source-record revisions and a fan-out outbox entry. `deliver` atomically
/// confirms exact key equality, attaches the origin or creates the library
/// entry, and queues fulltext/rules. Exact equality means the complete URL,
/// title and optional description values compare equal; a URL-only index may
/// narrow candidates but must never decide equality. A changed record key must
/// regroup all affected library origins using `Article::merge`/
/// `Article::split_with_keys`, preserving their defined states. `publish_content` switches the readable
/// manifest only after every chunk is durable.
#[async_trait]
pub trait IngestStore: Send + Sync {
    async fn claim(
        &self,
        worker: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> Result<Option<LeasedWork>, StoreError>;
    async fn renew(
        &self,
        job: JobId,
        token: LeaseToken,
        lease_until: DateTime<Utc>,
    ) -> Result<(), StoreError>;
    async fn complete(&self, job: JobId, token: LeaseToken) -> Result<(), StoreError>;
    async fn retry(
        &self,
        job: JobId,
        token: LeaseToken,
        run_at: DateTime<Utc>,
        diagnostic: &str,
    ) -> Result<(), StoreError>;
    async fn fail(&self, job: JobId, token: LeaseToken, diagnostic: &str)
        -> Result<(), StoreError>;
    async fn source(&self, id: SourceId) -> Result<SourceDefinition, StoreError>;
    async fn has_committed_poll(&self, source: SourceId) -> Result<bool, StoreError>;
    async fn source_validators(&self, source: SourceId) -> Result<CacheValidators, StoreError>;
    async fn active_delivery_count(&self, source: SourceId) -> Result<u64, StoreError>;
    async fn commit_poll(&self, lease: &LeasedWork, commit: PollCommit) -> Result<(), StoreError>;
    async fn record_source_success(
        &self,
        lease: &LeasedWork,
        source: SourceId,
        at: DateTime<Utc>,
        incomplete: bool,
        duration_ms: u64,
    ) -> Result<(), StoreError>;
    async fn record(&self, id: SourceRecordId) -> Result<SourceRecord, StoreError>;
    async fn record_by_upstream(
        &self,
        source: SourceId,
        upstream_id: &str,
    ) -> Result<Option<SourceRecord>, StoreError>;
    async fn delivery_targets(
        &self,
        source: SourceId,
        after: Option<SubscriptionId>,
        limit: usize,
    ) -> Result<Vec<DeliveryTarget>, StoreError>;
    async fn deliver(
        &self,
        lease: &LeasedWork,
        commit: DeliveryCommit,
    ) -> Result<DeliveryResult, StoreError>;
    /// Persists the cursor and enqueues the next bounded fan-out page when more
    /// targets exist. It must be idempotent for `(job, token, after)`.
    async fn advance_fanout(
        &self,
        lease: &LeasedWork,
        after: SubscriptionId,
    ) -> Result<(), StoreError>;
    async fn publish_content(
        &self,
        lease: &LeasedWork,
        revision: ContentRevision,
    ) -> Result<(), StoreError>;
    async fn cleanup_content(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        keep_refresh: uuid::Uuid,
    ) -> Result<(), StoreError>;
    async fn record_refresh_failure(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        diagnostic: &str,
    ) -> Result<(), StoreError>;
    async fn evaluate_article_rules(
        &self,
        lease: &LeasedWork,
        workspace: WorkspaceId,
        article: ArticleId,
        evaluations: &[PendingRuleEvaluation],
    ) -> Result<(), StoreError>;
    /// Applies at most `limit` articles and returns the last processed id when
    /// another page may exist. A missing, disabled, or newer rule is a
    /// successful cancellation and returns `None` without mutating articles.
    // These values form the complete durable fencing cursor; grouping them in
    // a loose DTO would weaken the port's explicit mutation contract.
    #[allow(clippy::too_many_arguments)]
    async fn apply_rule_batch(
        &self,
        lease: &LeasedWork,
        workspace: WorkspaceId,
        rule: RuleId,
        version: u64,
        after: Option<ArticleId>,
        through: Option<ArticleId>,
        limit: usize,
    ) -> Result<Option<ArticleId>, StoreError>;
}
