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
    Cloudera { listing_url: Url },
    Digoal { listing_url: Url },
    Mirrorship,
    Pingkai { listing_url: Url },
    ModbNews,
    InfoqBigdata,
    Highgo { max_pages: usize },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
// Recipes are configuration objects, and boxing this branch would complicate
// validated serde construction for no measured hot-path benefit.
#[allow(clippy::large_enum_variant)]
pub enum SourceKind {
    Auto,
    XmlFeed,
    JsonFeed,
    WebPage(WebFeedRecipe),
    BuiltIn(BuiltInAdapter),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebLoading {
    Automatic,
    Static,
    Browser,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorLanguage {
    Css,
    XPath,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebSelector {
    language: SelectorLanguage,
    expression: String,
}
impl WebSelector {
    pub fn new(language: SelectorLanguage, expression: String) -> Result<Self, ModelError> {
        if expression.trim().is_empty() {
            return Err(ModelError::EmptySelector);
        }
        if language == SelectorLanguage::Css {
            scraper::Selector::parse(&expression).map_err(|_| ModelError::InvalidSelector)?;
        } else if !expression.trim_start().starts_with('/')
            && !expression.trim_start().starts_with('(')
        {
            return Err(ModelError::InvalidSelector);
        }
        Ok(Self {
            language,
            expression,
        })
    }
    pub fn language(&self) -> SelectorLanguage {
        self.language
    }
    pub fn expression(&self) -> &str {
        &self.expression
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebViewport {
    Desktop,
    Mobile,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebFeedActions {
    viewport: WebViewport,
    hide_overlays: Vec<WebSelector>,
    start_pages: Vec<Url>,
    next_page: Option<WebSelector>,
    load_more: Option<WebSelector>,
    load_more_clicks: usize,
    scrolls: usize,
}
impl WebFeedActions {
    // Both configured limits are part of the construction boundary: keeping
    // them here prevents callers from creating an unchecked action plan.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        viewport: WebViewport,
        hide_overlays: Vec<WebSelector>,
        start_pages: Vec<Url>,
        next_page: Option<WebSelector>,
        load_more: Option<WebSelector>,
        load_more_clicks: usize,
        scrolls: usize,
        max_pages: usize,
        max_actions: usize,
    ) -> Result<Self, ModelError> {
        let value = Self {
            viewport,
            hide_overlays,
            start_pages,
            next_page,
            load_more,
            load_more_clicks,
            scrolls,
        };
        if value.page_count() > max_pages {
            return Err(ModelError::LimitExceeded("start_pages"));
        }
        if value.action_count() > max_actions {
            return Err(ModelError::LimitExceeded("actions"));
        }
        Ok(value)
    }
    pub fn viewport(&self) -> WebViewport {
        self.viewport
    }
    pub fn hide_overlays(&self) -> &[WebSelector] {
        &self.hide_overlays
    }
    pub fn start_pages(&self) -> &[Url] {
        &self.start_pages
    }
    pub fn next_page(&self) -> Option<&WebSelector> {
        self.next_page.as_ref()
    }
    pub fn load_more(&self) -> Option<&WebSelector> {
        self.load_more.as_ref()
    }
    pub fn load_more_clicks(&self) -> usize {
        self.load_more_clicks
    }
    pub fn scrolls(&self) -> usize {
        self.scrolls
    }
    pub fn page_count(&self) -> usize {
        self.start_pages.len().saturating_add(1)
    }
    pub fn action_count(&self) -> usize {
        self.hide_overlays
            .len()
            .saturating_add(self.load_more_clicks)
            .saturating_add(self.scrolls)
            .saturating_add(usize::from(self.next_page.is_some()))
    }
}
impl Default for WebFeedActions {
    fn default() -> Self {
        Self {
            viewport: WebViewport::Desktop,
            hide_overlays: vec![],
            start_pages: vec![],
            next_page: None,
            load_more: None,
            load_more_clicks: 0,
            scrolls: 0,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebExtraction {
    listing_url: Option<Url>,
    card_selector: Option<WebSelector>,
    title_selector: Option<WebSelector>,
    date_selector: Option<WebSelector>,
    content_selector: Option<WebSelector>,
    wait_selector: Option<WebSelector>,
    url_pattern: Option<String>,
}
impl WebExtraction {
    pub fn new(
        listing_url: Option<Url>,
        card_selector: Option<String>,
        title_selector: Option<String>,
        date_selector: Option<String>,
        content_selector: Option<String>,
        wait_selector: Option<String>,
        url_pattern: Option<String>,
    ) -> Result<Self, ModelError> {
        let css = |value: Option<String>| {
            value
                .map(|value| WebSelector::new(SelectorLanguage::Css, value))
                .transpose()
        };
        if let Some(value) = url_pattern.as_deref() {
            regex::Regex::new(value).map_err(|_| ModelError::InvalidUrlPattern)?;
        }
        Ok(Self {
            listing_url,
            card_selector: css(card_selector)?,
            title_selector: css(title_selector)?,
            date_selector: css(date_selector)?,
            content_selector: css(content_selector)?,
            wait_selector: css(wait_selector)?,
            url_pattern,
        })
    }
    pub fn listing_url(&self) -> Option<&Url> {
        self.listing_url.as_ref()
    }
    pub fn card_selector(&self) -> Option<&WebSelector> {
        self.card_selector.as_ref()
    }
    pub fn title_selector(&self) -> Option<&WebSelector> {
        self.title_selector.as_ref()
    }
    pub fn date_selector(&self) -> Option<&WebSelector> {
        self.date_selector.as_ref()
    }
    pub fn content_selector(&self) -> Option<&WebSelector> {
        self.content_selector.as_ref()
    }
    pub fn wait_selector(&self) -> Option<&WebSelector> {
        self.wait_selector.as_ref()
    }
    pub fn url_pattern(&self) -> Option<&str> {
        self.url_pattern.as_deref()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WebFeedRecipe {
    selector: WebSelector,
    loading: WebLoading,
    actions: WebFeedActions,
    #[serde(default)]
    extraction: WebExtraction,
    #[serde(default = "one_page")]
    max_pages: usize,
}
const fn one_page() -> usize {
    1
}
impl WebFeedRecipe {
    pub fn new(selector: String, loading: WebLoading) -> Result<Self, ModelError> {
        Self::advanced(
            WebSelector::new(SelectorLanguage::Css, selector)?,
            loading,
            WebFeedActions::default(),
        )
    }
    pub fn advanced(
        selector: WebSelector,
        loading: WebLoading,
        actions: WebFeedActions,
    ) -> Result<Self, ModelError> {
        Self::legacy(selector, loading, actions, WebExtraction::default(), 1)
    }
    pub fn legacy(
        selector: WebSelector,
        loading: WebLoading,
        actions: WebFeedActions,
        extraction: WebExtraction,
        max_pages: usize,
    ) -> Result<Self, ModelError> {
        if loading == WebLoading::Static && selector.language() == SelectorLanguage::XPath {
            return Err(ModelError::XPathRequiresBrowser);
        }
        if max_pages == 0 {
            return Err(ModelError::ZeroLimit {
                field: "web_feed.max_pages",
            });
        }
        Ok(Self {
            selector,
            loading,
            actions,
            extraction,
            max_pages,
        })
    }
    pub fn selector(&self) -> &str {
        self.selector.expression()
    }
    pub fn selector_kind(&self) -> SelectorLanguage {
        self.selector.language()
    }
    pub fn loading(&self) -> WebLoading {
        self.loading
    }
    pub fn actions(&self) -> &WebFeedActions {
        &self.actions
    }
    pub fn extraction(&self) -> &WebExtraction {
        &self.extraction
    }
    pub fn max_pages(&self) -> usize {
        self.max_pages
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceDefinition {
    id: SourceId,
    url: Url,
    kind: SourceKind,
    revision: u64,
}
impl SourceDefinition {
    pub fn new(id: SourceId, url: Url, kind: SourceKind) -> Result<Self, ModelError> {
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
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
    feed_content_html: Option<String>,
    published_at: Option<DateTime<Utc>>,
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
    pub fn feed_content_html(&self) -> Option<&str> {
        self.feed_content_html.as_deref()
    }
    pub fn published_at(&self) -> Option<DateTime<Utc>> {
        self.published_at
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
            feed_content_html: value.content_html,
            published_at: value.published_at,
            revision: self.revision + 1,
        };
        let effect = if next.key != self.key {
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
            feed_content_html: value.feed_content_html,
            published_at: value.published_at,
            revision: self.revision + 1,
        };
        let effect = if next.key != self.key {
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
    pub fetched_at: DateTime<Utc>,
    pub final_url: Url,
    pub validators: CacheValidators,
    pub incomplete: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    #[error("source URL must be an absolute HTTP(S) URL")]
    InvalidSourceUrl,
    #[error("upstream identity must not be empty")]
    EmptyUpstreamIdentity,
    #[error("a source record revision cannot change upstream identity")]
    UpstreamIdentityChanged,
    #[error("configured limit {field} must be greater than zero")]
    ZeroLimit { field: &'static str },
    #[error("Web feed selector must not be empty")]
    EmptySelector,
    #[error("selector syntax is invalid")]
    InvalidSelector,
    #[error("URL pattern syntax is invalid")]
    InvalidUrlPattern,
    #[error("XPath selection requires browser or automatic loading")]
    XPathRequiresBrowser,
    #[error("configured Web feed limit exceeded: {0}")]
    LimitExceeded(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngestLimits {
    pub initial_feed_items: usize,
    pub fanout_batch: usize,
    pub content_chunk_bytes: usize,
    pub max_input_bytes: usize,
    pub max_extracted_bytes: usize,
}
impl IngestLimits {
    pub fn new(
        initial_feed_items: usize,
        fanout_batch: usize,
        content_chunk_bytes: usize,
    ) -> Result<Self, ModelError> {
        for (field, value) in [
            ("initial_feed_items", initial_feed_items),
            ("fanout_batch", fanout_batch),
            ("content_chunk_bytes", content_chunk_bytes),
        ] {
            if value == 0 {
                return Err(ModelError::ZeroLimit { field });
            }
        }
        Ok(Self {
            initial_feed_items,
            fanout_batch,
            content_chunk_bytes,
            max_input_bytes: usize::MAX,
            max_extracted_bytes: usize::MAX,
        })
    }
    pub fn with_payload_limits(
        mut self,
        max_input_bytes: usize,
        max_extracted_bytes: usize,
    ) -> Result<Self, ModelError> {
        if max_input_bytes == 0 {
            return Err(ModelError::ZeroLimit {
                field: "max_input_bytes",
            });
        }
        if max_extracted_bytes == 0 {
            return Err(ModelError::ZeroLimit {
                field: "max_extracted_bytes",
            });
        }
        self.max_input_bytes = max_input_bytes;
        self.max_extracted_bytes = max_extracted_bytes;
        Ok(self)
    }
}

#[cfg(test)]
mod tests;
