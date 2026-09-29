use chrono::{DateTime, Utc};
use reader_collectors::ParsedRecord;
use reader_core::{
    ArticleId, ArticleLocation, DedupKey, PendingRuleEvaluation, RuleId, SourceId, SourceRecordId,
    SubscriptionId, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(Uuid);
impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}
impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeaseToken(Uuid);
impl LeaseToken {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}
impl Default for LeaseToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltInAdapter {
    Cloudera {
        listing_url: Url,
    },
    Digoal {
        listing_url: Url,
    },
    Mirrorship,
    Dropbox,
    /// Account-owned session; only public author article listings are collected.
    Zhihu {
        max_pages: std::num::NonZeroUsize,
    },
    /// Public Telegram history window; empty source title is preserved.
    Telegram {
        max_pages: std::num::NonZeroUsize,
    },
    Pingkai {
        listing_url: Url,
    },
    ModbNews,
    InfoqBigdata,
    /// Public latest-articles window, using the publisher-provided page size.
    Volcengine,
    JdCloud {
        page_size: std::num::NonZeroUsize,
    },
    Highgo {
        max_pages: usize,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
// Recipes are configuration objects, and boxing this branch would complicate
// validated serde construction for no measured hot-path benefit.
#[allow(clippy::large_enum_variant)]
pub enum SourceKind {
    Auto,
    XmlFeed,
    /// Operator-selected RSS policy; only byte-identical repeated items coalesce.
    /// Never selected by URL discovery or used as the default feed kind.
    RssCoalesceIdentical,
    JsonFeed,
    WebPage(WebFeedRecipe),
    BuiltIn(BuiltInAdapter),
}

pub use reader_core::{
    SelectorLanguage, WebExtraction, WebFeedActions, WebFeedRecipe, WebLoading, WebSelector,
    WebViewport,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SourceDefinitionDraft")]
pub struct SourceDefinition {
    id: SourceId,
    url: Url,
    kind: SourceKind,
    revision: u64,
}
#[derive(Deserialize)]
struct SourceDefinitionDraft {
    id: SourceId,

    url: Url,

    kind: SourceKind,

    revision: u64,
}
impl TryFrom<SourceDefinitionDraft> for SourceDefinition {
    type Error = ModelError;
    fn try_from(raw: SourceDefinitionDraft) -> Result<Self, Self::Error> {
        let mut source = Self::new(raw.id, raw.url, raw.kind)?;
        source.revision = raw.revision;
        Ok(source)
    }
}
impl SourceDefinition {
    pub fn new(id: SourceId, url: Url, kind: SourceKind) -> Result<Self, ModelError> {
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(ModelError::InvalidSourceUrl);
        }
        if matches!(kind, SourceKind::BuiltIn(BuiltInAdapter::Telegram { .. })) {
            let channel = url.path().trim_matches('/');
            if url.scheme() != "https"
                || url.host_str() != Some("t.me")
                || channel.is_empty()
                || !channel
                    .bytes()
                    .all(|v| v.is_ascii_alphanumeric() || v == b'_')
                || url.query().is_some()
                || url.fragment().is_some()
                || url.port().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(ModelError::InvalidSourceUrl);
            }
        }
        if matches!(kind, SourceKind::BuiltIn(BuiltInAdapter::Zhihu { .. }))
            && crate::zhihu::author(&url).is_none()
        {
            return Err(ModelError::InvalidSourceUrl);
        }
        Ok(Self {
            id,
            url,
            kind,
            revision: 0,
        })
    }
    pub fn id(&self) -> SourceId {
        self.id
    }
    pub fn url(&self) -> &Url {
        &self.url
    }
    pub fn kind(&self) -> &SourceKind {
        &self.kind
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn with_url(&self, url: Url) -> Result<Self, ModelError> {
        let mut value = Self::new(self.id, url, self.kind.clone())?;
        value.revision = self.revision;
        Ok(value)
    }
    pub fn revise_kind(&self, kind: SourceKind) -> Result<Self, ModelError> {
        let mut value = Self::new(self.id, self.url.clone(), kind)?;
        value.revision = self.revision.saturating_add(1);
        Ok(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    id: SourceRecordId,
    source_id: SourceId,
    upstream_id: String,
    key: DedupKey,
    description_media_type: Option<String>,
    feed_content_html: Option<String>,
    published_at: Option<reader_core::PublicationDate>,
    revision: u64,
}
impl SourceRecord {
    pub fn from_parsed(
        id: SourceRecordId,
        source_id: SourceId,
        value: ParsedRecord,
    ) -> Result<Self, ModelError> {
        if value.upstream_id.is_empty() {
            return Err(ModelError::EmptyUpstreamIdentity);
        }
        let location = article_location(&value, source_id);
        Ok(Self {
            id,
            source_id,
            upstream_id: value.upstream_id,
            key: DedupKey {
                location,
                title: value.title,
                description: value.description,
            },
            description_media_type: value.description_media_type,
            feed_content_html: value.content_html,
            published_at: value.published_at,
            revision: 0,
        })
    }
    pub fn id(&self) -> SourceRecordId {
        self.id
    }
    pub fn source_id(&self) -> SourceId {
        self.source_id
    }
    pub fn upstream_id(&self) -> &str {
        &self.upstream_id
    }
    pub fn key(&self) -> &DedupKey {
        &self.key
    }
    pub fn description_media_type(&self) -> Option<&str> {
        self.description_media_type.as_deref()
    }
    pub fn feed_content_html(&self) -> Option<&str> {
        self.feed_content_html.as_deref()
    }
    pub fn published_at(&self) -> Option<&reader_core::PublicationDate> {
        self.published_at.as_ref()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// A repeated upstream identity updates the same source record. The caller
    /// persists this value with expected `revision`; a changed exact key then
    /// triggers library regrouping, while any content change schedules a fresh
    /// fulltext attempt. The previous successful content manifest stays current
    /// until that attempt is fully published.
    pub fn revise(&self, value: ParsedRecord) -> Result<RecordRevision, ModelError> {
        if value.upstream_id != self.upstream_id {
            return Err(ModelError::UpstreamIdentityChanged);
        }
        let next = Self {
            id: self.id,
            source_id: self.source_id,
            upstream_id: self.upstream_id.clone(),
            key: DedupKey {
                location: article_location(&value, self.source_id),
                title: value.title,
                description: value.description,
            },
            description_media_type: value.description_media_type,
            feed_content_html: value.content_html,
            published_at: value.published_at,
            revision: self.revision + 1,
        };
        let effect =
            if next.key != self.key || next.description_media_type != self.description_media_type {
                RecordRevisionEffect::RegroupAndRefresh
            } else if next.feed_content_html != self.feed_content_html
                || next.published_at != self.published_at
            {
                RecordRevisionEffect::RefreshContent
            } else {
                RecordRevisionEffect::Unchanged
            };
        Ok(RecordRevision {
            record: next,
            effect,
        })
    }

    /// Explicit Dropbox archive update: listing HTML supplies title and a day,
    /// not RSS description/body or timestamp precision. Retain those existing
    /// fields verbatim. Missing old dates may be filled by the authored day;
    /// contradictory days fail before any record is committed. Other adapters
    /// continue using complete replacement through `revise_from_record`.
    pub(crate) fn revise_from_dropbox_listing(
        &self,
        mut value: SourceRecord,
    ) -> Result<RecordRevision, ModelError> {
        if self
            .published_at
            .as_ref()
            .zip(value.published_at.as_ref())
            .is_some_and(|(old, new)| old.day() != new.day())
        {
            return Err(ModelError::DropboxPublicationDayConflict);
        }
        value.key.description = self.key.description.clone();
        value.description_media_type = self.description_media_type.clone();
        value.feed_content_html = self.feed_content_html.clone();
        if self.published_at.is_some() {
            value.published_at = self.published_at.clone();
        }
        self.revise_from_record(value)
    }

    /// Reconciles collector output by stable upstream identity. Web collectors
    /// may propose fresh ids, but persisted identity and revision always win.
    pub fn revise_from_record(&self, value: SourceRecord) -> Result<RecordRevision, ModelError> {
        if value.upstream_id != self.upstream_id || value.source_id != self.source_id {
            return Err(ModelError::UpstreamIdentityChanged);
        }
        let next = Self {
            id: self.id,
            source_id: self.source_id,
            upstream_id: self.upstream_id.clone(),
            key: value.key,
            description_media_type: value.description_media_type,
            feed_content_html: value.feed_content_html,
            published_at: value.published_at,
            revision: self.revision + 1,
        };
        let effect =
            if next.key != self.key || next.description_media_type != self.description_media_type {
                RecordRevisionEffect::RegroupAndRefresh
            } else if next.feed_content_html != self.feed_content_html
                || next.published_at != self.published_at
            {
                RecordRevisionEffect::RefreshContent
            } else {
                RecordRevisionEffect::Unchanged
            };
        Ok(RecordRevision {
            record: next,
            effect,
        })
    }
}

fn article_location(value: &ParsedRecord, source_id: SourceId) -> ArticleLocation {
    match &value.absolute_url {
        Some(fetch) => {
            let exact = Url::parse(&value.original_url)
                .ok()
                .filter(|url| url.has_host())
                .map(|_| value.original_url.clone())
                .unwrap_or_else(|| fetch.as_str().to_owned());
            ArticleLocation::url(exact, fetch.clone())
        }
        None => ArticleLocation::SourceRecord {
            source_id,
            upstream_id: value.upstream_id.clone(),
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordRevisionEffect {
    Unchanged,
    RefreshContent,
    RegroupAndRefresh,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordRevision {
    pub record: SourceRecord,
    pub effect: RecordRevisionEffect,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeliveryTarget {
    pub workspace_id: WorkspaceId,
    pub subscription_id: SubscriptionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkItem {
    PollSource {
        source_id: SourceId,
    },
    RefreshSource {
        source_id: SourceId,
    },
    FanOut {
        source_id: SourceId,
        record_id: SourceRecordId,
        after_subscription: Option<SubscriptionId>,
    },
    ExtractFullText {
        record_id: SourceRecordId,
        source_revision: u64,
        url: Url,
        manual: bool,
    },
    CollectWebFeed {
        source_id: SourceId,
    },
    CleanupContent {
        record_id: SourceRecordId,
        keep_refresh_id: Uuid,
    },
    EvaluateArticleRules {
        workspace_id: WorkspaceId,
        article_id: ArticleId,
        evaluations: Vec<PendingRuleEvaluation>,
    },
    ApplyRule {
        workspace_id: WorkspaceId,
        rule_id: RuleId,
        rule_version: u64,
        after_article: Option<ArticleId>,
        through_article: Option<ArticleId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LeasedWork {
    pub job_id: JobId,
    pub item: WorkItem,
    pub token: LeaseToken,
    pub deadline: DateTime<Utc>,
    pub attempt: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PollCommit {
    pub source_id: SourceId,
    pub source_revision: u64,
    pub records: Vec<PolledRecord>,
    /// Already received records deferred only for initial visible-depth policy.
    /// Storage retains them atomically with the first batch and its validators.
    pub remainder: Vec<PolledRecord>,
    pub fetched_at: DateTime<Utc>,
    pub final_url: Url,
    pub validators: CacheValidators,
    pub incomplete: bool,
    pub duration_ms: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PollAction {
    Deliver,
    RefreshContent,
    Regroup,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolledRecord {
    pub record: SourceRecord,
    pub action: PollAction,
}
impl std::ops::Deref for PolledRecord {
    type Target = SourceRecord;
    fn deref(&self) -> &Self::Target {
        &self.record
    }
}
impl Serialize for PolledRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.record.serialize(serializer)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheValidators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryCommit {
    pub target: DeliveryTarget,
    pub record: SourceRecord,
    pub proposed_article_id: ArticleId,
    pub delivered_at: DateTime<Utc>,
}

/// `SkippedInactive` is decided inside the same transaction that would attach
/// the origin. This closes the pause/archive race after target enumeration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryResult {
    Delivered(ArticleId),
    AlreadyDelivered(ArticleId),
    SkippedInactive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentChunk {
    pub ordinal: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentRevision {
    pub publication: Vec<reader_core::PublicationEvidence>,
    pub record_id: SourceRecordId,
    pub source_revision: u64,
    pub refresh_id: Uuid,
    pub raw_chunks: Vec<ContentChunk>,
    pub safe_html_chunks: Vec<ContentChunk>,
    pub fetched_at: DateTime<Utc>,
    pub final_url: Url,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentManifestPointer {
    /// None means metadata has not been extracted; Some(empty) means no declaration.
    pub publication: Option<Vec<reader_core::PublicationEvidence>>,
    pub record_id: SourceRecordId,
    pub source_revision: u64,
    pub refresh_id: Uuid,
    pub raw_chunks: u32,
    pub safe_html_chunks: u32,
    pub fetched_at: DateTime<Utc>,
    pub final_url: Url,
}
impl From<&ContentRevision> for ContentManifestPointer {
    fn from(v: &ContentRevision) -> Self {
        Self {
            publication: Some(v.publication.clone()),
            record_id: v.record_id,
            source_revision: v.source_revision,
            refresh_id: v.refresh_id,
            raw_chunks: v.raw_chunks.len() as u32,
            safe_html_chunks: v.safe_html_chunks.len() as u32,
            fetched_at: v.fetched_at,
            final_url: v.final_url.clone(),
        }
    }
}
impl From<&&mut ContentRevision> for ContentManifestPointer {
    fn from(v: &&mut ContentRevision) -> Self {
        Self::from(&**v)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserCapability {
    Available,
    Degraded,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelError {
    #[error("Dropbox archive day conflicts with the retained publication date")]
    DropboxPublicationDayConflict,
    #[error("source URL must be an absolute HTTP(S) URL")]
    InvalidSourceUrl,
    #[error("upstream identity must not be empty")]
    EmptyUpstreamIdentity,
    #[error("a source record revision cannot change upstream identity")]
    UpstreamIdentityChanged,
    #[error("configured limit {field} must be greater than zero")]
    ZeroLimit { field: &'static str },
}

/// Validated immutable execution limits. All counts and byte limits are positive.
/// Construction is the only input boundary; cloning preserves the invariants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestLimits {
    initial_feed_items: std::num::NonZeroUsize,

    fanout_batch: std::num::NonZeroUsize,

    content_chunk_bytes: std::num::NonZeroUsize,

    max_input_bytes: std::num::NonZeroUsize,

    max_extracted_bytes: std::num::NonZeroUsize,
}
impl IngestLimits {
    pub fn new(
        initial_feed_items: usize,
        fanout_batch: usize,
        content_chunk_bytes: usize,
        max_input_bytes: usize,
        max_extracted_bytes: usize,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            initial_feed_items: std::num::NonZeroUsize::new(initial_feed_items).ok_or(
                ModelError::ZeroLimit {
                    field: "initial_feed_items",
                },
            )?,
            fanout_batch: std::num::NonZeroUsize::new(fanout_batch).ok_or(
                ModelError::ZeroLimit {
                    field: "fanout_batch",
                },
            )?,
            content_chunk_bytes: std::num::NonZeroUsize::new(content_chunk_bytes).ok_or(
                ModelError::ZeroLimit {
                    field: "content_chunk_bytes",
                },
            )?,
            max_input_bytes: std::num::NonZeroUsize::new(max_input_bytes).ok_or(
                ModelError::ZeroLimit {
                    field: "max_input_bytes",
                },
            )?,
            max_extracted_bytes: std::num::NonZeroUsize::new(max_extracted_bytes).ok_or(
                ModelError::ZeroLimit {
                    field: "max_extracted_bytes",
                },
            )?,
        })
    }
    pub fn initial_feed_items(&self) -> usize {
        self.initial_feed_items.get()
    }
    pub fn fanout_batch(&self) -> usize {
        self.fanout_batch.get()
    }
    pub fn content_chunk_bytes(&self) -> usize {
        self.content_chunk_bytes.get()
    }
    pub fn max_input_bytes(&self) -> usize {
        self.max_input_bytes.get()
    }
    pub fn max_extracted_bytes(&self) -> usize {
        self.max_extracted_bytes.get()
    }
}

#[cfg(test)]
mod tests;
