use super::*;
use crate::*;
use async_trait::async_trait;
use chrono::TimeZone;
use reader_core::{SourceId, SubscriptionId, WorkspaceId};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use url::Url;

struct FetchStub {
    page: Result<FetchedPage, FetchError>,
    calls: AtomicUsize,
}
#[async_trait]
impl FeedFetcher for FetchStub {
    async fn fetch(&self, _: &Url, _: &CacheValidators) -> Result<FetchedPage, FetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.page.clone()
    }
}
#[async_trait]
impl FullTextExtractor for FetchStub {
    async fn extract(&self, _: &Url) -> Result<FetchedPage, FetchError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.page.clone()
    }
}

struct BrowserStub(BrowserCapability);
#[async_trait]
impl BrowserCollector for BrowserStub {
    fn capability(&self) -> BrowserCapability {
        self.0
    }
    async fn collect(&self, _: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        Ok(vec![])
    }
}

struct BrowserRecords(Vec<SourceRecord>);
#[async_trait]
impl BrowserCollector for BrowserRecords {
    fn capability(&self) -> BrowserCapability {
        BrowserCapability::Available
    }
    async fn collect(&self, _: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        Ok(self.0.clone())
    }
}

struct StoreStub {
    source: SourceDefinition,
    active: u64,
    had_poll: bool,
    record: Option<SourceRecord>,
    targets: Vec<DeliveryTarget>,
    polls: Mutex<Vec<PollCommit>>,
    deliveries: Mutex<Vec<DeliveryCommit>>,
    manifests: Mutex<Vec<ContentRevision>>,
    refresh_failures: Mutex<Vec<SourceRecordId>>,
    validators: CacheValidators,
}

#[async_trait]
impl IngestStore for StoreStub {
    async fn claim(
        &self,
        _: &str,
        _: DateTime<Utc>,
        _: DateTime<Utc>,
    ) -> Result<Option<LeasedWork>, StoreError> {
        Ok(None)
    }
    async fn renew(&self, _: JobId, _: LeaseToken, _: DateTime<Utc>) -> Result<(), StoreError> {
        Ok(())
    }
    async fn complete(&self, _: JobId, _: LeaseToken) -> Result<(), StoreError> {
        Ok(())
    }
    async fn retry(
        &self,
        _: JobId,
        _: LeaseToken,
        _: DateTime<Utc>,
        _: &str,
    ) -> Result<(), StoreError> {
        Ok(())
    }
    async fn fail(&self, _: JobId, _: LeaseToken, _: &str) -> Result<(), StoreError> {
        Ok(())
    }
    async fn source(&self, _: SourceId) -> Result<SourceDefinition, StoreError> {
        Ok(self.source.clone())
    }
    async fn has_committed_poll(&self, _: SourceId) -> Result<bool, StoreError> {
        Ok(self.had_poll)
    }
    async fn source_validators(&self, _: SourceId) -> Result<CacheValidators, StoreError> {
        Ok(self.validators.clone())
    }
    async fn active_delivery_count(&self, _: SourceId) -> Result<u64, StoreError> {
        Ok(self.active)
    }
    async fn commit_poll(&self, _: &LeasedWork, value: PollCommit) -> Result<(), StoreError> {
        self.polls.lock().unwrap().push(value);
        Ok(())
    }
    async fn record_source_success(
        &self,
        _: &LeasedWork,
        _: SourceId,
        _: DateTime<Utc>,
        _: bool,
        _: u64,
    ) -> Result<(), StoreError> {
        Ok(())
    }
    async fn record(&self, _: SourceRecordId) -> Result<SourceRecord, StoreError> {
        self.record.clone().ok_or(StoreError::NotFound)
    }
    async fn record_by_upstream(
        &self,
        _: SourceId,
        upstream_id: &str,
    ) -> Result<Option<SourceRecord>, StoreError> {
        Ok(self
            .record
            .clone()
            .filter(|v| v.upstream_id() == upstream_id))
    }
    async fn delivery_targets(
        &self,
        _: SourceId,
        _: Option<SubscriptionId>,
        _: usize,
    ) -> Result<Vec<DeliveryTarget>, StoreError> {
        Ok(self.targets.clone())
    }
    async fn deliver(
        &self,
        _: &LeasedWork,
        value: DeliveryCommit,
    ) -> Result<DeliveryResult, StoreError> {
        let id = value.proposed_article_id;
        self.deliveries.lock().unwrap().push(value);
        Ok(DeliveryResult::Delivered(id))
    }
    async fn advance_fanout(&self, _: &LeasedWork, _: SubscriptionId) -> Result<(), StoreError> {
        Ok(())
    }
    async fn publish_content(
        &self,
        _: &LeasedWork,
        value: ContentRevision,
    ) -> Result<(), StoreError> {
        self.manifests.lock().unwrap().push(value);
        Ok(())
    }
    async fn cleanup_content(
        &self,
        _: &LeasedWork,
        _: SourceRecordId,
        _: uuid::Uuid,
    ) -> Result<(), StoreError> {
        Ok(())
    }
    async fn record_refresh_failure(
        &self,
        _: &LeasedWork,
        id: SourceRecordId,
        _: &str,
    ) -> Result<(), StoreError> {
        self.refresh_failures.lock().unwrap().push(id);
        Ok(())
    }
    async fn evaluate_article_rules(
        &self,
        _: &LeasedWork,
        _: reader_core::WorkspaceId,
        _: reader_core::ArticleId,
        _: &[reader_core::PendingRuleEvaluation],
    ) -> Result<(), StoreError> {
        Ok(())
    }
    async fn apply_rule_batch(
        &self,
        _: &LeasedWork,
        _: reader_core::WorkspaceId,
        _: reader_core::RuleId,
        _: u64,
        _: Option<reader_core::ArticleId>,
        _: Option<reader_core::ArticleId>,
        _: usize,
    ) -> Result<Option<reader_core::ArticleId>, StoreError> {
        Ok(None)
    }
}

fn source() -> SourceDefinition {
    SourceDefinition::new(
        SourceId::new(),
        Url::parse("https://example.test/feed").unwrap(),
        SourceKind::XmlFeed,
    )
    .unwrap()
}
fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 25, 12, 0, 0).unwrap()
}
fn lease(item: WorkItem) -> LeasedWork {
    LeasedWork {
        job_id: JobId::new(),
        item,
        token: LeaseToken::new(),
        deadline: now() + Duration::minutes(1),
        attempt: 0,
    }
}
fn store(source: SourceDefinition) -> StoreStub {
    StoreStub {
        source,
        active: 1,
        had_poll: false,
        record: None,
        targets: vec![],
        polls: Mutex::new(vec![]),
        deliveries: Mutex::new(vec![]),
        manifests: Mutex::new(vec![]),
        refresh_failures: Mutex::new(vec![]),
        validators: Default::default(),
    }
}
fn fetch(page: Result<FetchedPage, FetchError>) -> Arc<FetchStub> {
    Arc::new(FetchStub {
        page,
        calls: AtomicUsize::new(0),
    })
}
fn worker(
    store: Arc<StoreStub>,
    fetcher: Arc<FetchStub>,
    browser: BrowserCapability,
    initial: usize,
) -> IngestWorker<StoreStub, FetchStub, FetchStub, BrowserStub> {
    IngestWorker::new(
        store,
        fetcher.clone(),
        fetcher,
        Arc::new(BrowserStub(browser)),
        IngestLimits::new(initial, 2, 4).unwrap(),
        Duration::seconds(30),
    )
    .unwrap()
}

#[tokio::test]
async fn initial_poll_persists_only_the_configured_visible_depth() {
    let source = source();
    let source_id = source.id();
    let store = Arc::new(store(source));
    let body=br#"<feed xmlns="http://www.w3.org/2005/Atom"><title>x</title><id>x</id><updated>2026-09-25T00:00:00Z</updated><entry><id>1</id><title>A</title><link href="/same"/><updated>2026-09-25T00:00:00Z</updated><summary>D</summary></entry><entry><id>2</id><title>B</title><link href="/same"/><updated>2026-09-25T00:00:00Z</updated><summary>D</summary></entry></feed>"#.to_vec();
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/feed").unwrap(),
        content_type: None,
        body,
        validators: Default::default(),
        not_modified: false,
    }));
    worker(store.clone(), fetcher, BrowserCapability::Available, 1)
        .handle(&lease(WorkItem::PollSource { source_id }), now())
        .await
        .unwrap();
    let polls = store.polls.lock().unwrap();
    assert_eq!(polls[0].records.len(), 1);
    assert_eq!(polls[0].records[0].record.key().title, "A");
}

#[tokio::test]
async fn polling_is_suppressed_when_every_subscription_is_inactive() {
    let source = source();
    let id = source.id();
    let mut raw = store(source);
    raw.active = 0;
    let store = Arc::new(raw);
    let fetcher = fetch(Err(FetchError::Rejected("must not run".into())));
    worker(
        store.clone(),
        fetcher.clone(),
        BrowserCapability::Available,
        10,
    )
    .handle(&lease(WorkItem::PollSource { source_id: id }), now())
    .await
    .unwrap();
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    assert!(store.polls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn not_modified_response_does_not_create_records_or_outbox_work() {
    let source = source();
    let id = source.id();
    let mut raw = store(source);
    raw.validators = CacheValidators {
        etag: Some("tag".into()),
        last_modified: None,
    };
    let store = Arc::new(raw);
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/feed").unwrap(),
        content_type: None,
        body: vec![],
        validators: Default::default(),
        not_modified: true,
    }));
    worker(store.clone(), fetcher, BrowserCapability::Available, 10)
        .handle(&lease(WorkItem::PollSource { source_id: id }), now())
        .await
        .unwrap();
    assert!(store.polls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn fanout_preserves_the_exact_url_title_description_key() {
    let source = source();
    let source_id = source.id();
    let parsed = reader_collectors::ParsedRecord {
        upstream_id: "one".into(),
        original_url: "/same".into(),
        absolute_url: Some(Url::parse("https://example.test/same").unwrap()),
        title: "Title A".into(),
        description: Some("Description A".into()),
        content_html: None,
        published_at: None,
    };
    let record = SourceRecord::from_parsed(SourceRecordId::new(), source_id, parsed).unwrap();
    let record_id = record.id();
    let mut raw = store(source);
    raw.record = Some(record);
    raw.targets = vec![DeliveryTarget {
        workspace_id: WorkspaceId::new(),
        subscription_id: SubscriptionId::new(),
    }];
    let store = Arc::new(raw);
    let fetcher = fetch(Err(FetchError::Rejected("unused".into())));
    worker(store.clone(), fetcher, BrowserCapability::Available, 10)
        .handle(
            &lease(WorkItem::FanOut {
                source_id,
                record_id,
                after_subscription: None,
            }),
            now(),
        )
        .await
        .unwrap();
    let deliveries = store.deliveries.lock().unwrap();
    let key = deliveries[0].record.key();
    assert_eq!(
        key.location,
        reader_core::ArticleLocation::from(Url::parse("https://example.test/same").unwrap())
    );
    assert_eq!(key.title, "Title A");
    assert_eq!(key.description.as_deref(), Some("Description A"));
}

#[tokio::test]
async fn successful_fulltext_is_chunked_and_published_as_one_revision() {
    let source = source();
    let store = Arc::new(store(source));
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/a").unwrap(),
        content_type: Some("text/html".into()),
        body: b"<p>hello</p>".to_vec(),
        validators: Default::default(),
        not_modified: false,
    }));
    let record_id = SourceRecordId::new();
    worker(store.clone(), fetcher, BrowserCapability::Available, 10)
        .handle(
            &lease(WorkItem::ExtractFullText {
                record_id,
                source_revision: 3,
                url: Url::parse("https://example.test/a").unwrap(),
                manual: false,
            }),
            now(),
        )
        .await
        .unwrap();
    let manifests = store.manifests.lock().unwrap();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].source_revision, 3);
    assert!(manifests[0].raw_chunks.len() > 1);
    assert!(manifests[0]
        .safe_html_chunks
        .iter()
        .all(|chunk| chunk.bytes.len() <= 4));
}

#[tokio::test]
async fn failed_refresh_records_diagnostic_without_publishing_a_manifest() {
    let source = source();
    let store = Arc::new(store(source));
    let fetcher = fetch(Err(FetchError::Http(503)));
    let record_id = SourceRecordId::new();
    let error = worker(store.clone(), fetcher, BrowserCapability::Available, 10)
        .handle(
            &lease(WorkItem::ExtractFullText {
                record_id,
                source_revision: 3,
                url: Url::parse("https://example.test/a").unwrap(),
                manual: true,
            }),
            now(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, IngestError::Fetch(FetchError::Http(503))));
    assert!(store.manifests.lock().unwrap().is_empty());
    assert_eq!(&*store.refresh_failures.lock().unwrap(), &[record_id]);
}

#[tokio::test]
async fn invalid_utf8_is_rejected_instead_of_becoming_replacement_characters() {
    let source = source();
    let store = Arc::new(store(source));
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/a").unwrap(),
        content_type: Some("text/html".into()),
        body: vec![0xff],
        validators: Default::default(),
        not_modified: false,
    }));
    let record_id = SourceRecordId::new();
    let error = worker(store.clone(), fetcher, BrowserCapability::Available, 10)
        .handle(
            &lease(WorkItem::ExtractFullText {
                record_id,
                source_revision: 1,
                url: Url::parse("https://example.test/a").unwrap(),
                manual: false,
            }),
            now(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, IngestError::Parse(_)));
    assert!(store.manifests.lock().unwrap().is_empty());
    assert_eq!(&*store.refresh_failures.lock().unwrap(), &[record_id]);
}

#[tokio::test]
async fn degraded_browser_is_explicit_and_does_not_publish_an_empty_poll() {
    let source = source();
    let id = source.id();
    let store = Arc::new(store(source));
    let fetcher = fetch(Err(FetchError::Rejected("unused".into())));
    let error = worker(store.clone(), fetcher, BrowserCapability::Degraded, 10)
        .handle(&lease(WorkItem::CollectWebFeed { source_id: id }), now())
        .await
        .unwrap_err();
    assert!(matches!(error, IngestError::BrowserDegraded));
    assert!(store.polls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn web_feed_initial_depth_is_marked_incomplete_and_refresh_uses_browser_path() {
    let source_id = SourceId::new();
    let recipe = WebFeedRecipe::new("article".into(), WebLoading::Static).unwrap();
    let source = SourceDefinition::new(
        source_id,
        Url::parse("https://example.test/news").unwrap(),
        SourceKind::WebPage(recipe),
    )
    .unwrap();
    let records = (0..2)
        .map(|index| {
            SourceRecord::from_parsed(
                SourceRecordId::new(),
                source_id,
                reader_collectors::ParsedRecord {
                    upstream_id: format!("item-{index}"),
                    original_url: format!("/item-{index}"),
                    absolute_url: Some(
                        Url::parse(&format!("https://example.test/item-{index}")).unwrap(),
                    ),
                    title: format!("Item {index}"),
                    description: None,
                    content_html: None,
                    published_at: None,
                },
            )
            .unwrap()
        })
        .collect();
    let store = Arc::new(store(source));
    let fetcher = fetch(Err(FetchError::Rejected(
        "feed parser must not run for Web feeds".into(),
    )));
    let worker = IngestWorker::new(
        store.clone(),
        fetcher.clone(),
        fetcher,
        Arc::new(BrowserRecords(records)),
        IngestLimits::new(1, 2, 4).unwrap(),
        Duration::seconds(30),
    )
    .unwrap();
    worker
        .handle(&lease(WorkItem::RefreshSource { source_id }), now())
        .await
        .unwrap();
    let polls = store.polls.lock().unwrap();
    assert_eq!(polls[0].records.len(), 1);
    assert!(polls[0].incomplete);
}
