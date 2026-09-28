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

#[derive(Default)]
struct HeartbeatProbe {
    job_lock: tokio::sync::Mutex<()>,
    renewal_started: tokio::sync::Notify,
}

struct StoreStub {
    heartbeat_probe: Option<Arc<HeartbeatProbe>>,
    claim: Mutex<Option<LeasedWork>>,
    retries: Mutex<Vec<DateTime<Utc>>>,
    failures: Mutex<Vec<Option<DateTime<Utc>>>>,
    source: SourceDefinition,
    active: u64,
    had_poll: bool,
    record: Option<SourceRecord>,
    targets: Vec<DeliveryTarget>,
    polls: Mutex<Vec<PollCommit>>,
    pending: Mutex<Option<PollCommit>>,
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
        Ok(self.claim.lock().unwrap().take())
    }
    async fn renew(&self, _: JobId, _: LeaseToken, _: DateTime<Utc>) -> Result<(), StoreError> {
        if let Some(probe) = &self.heartbeat_probe {
            probe.renewal_started.notify_one();
            let _lock = probe.job_lock.lock().await;
        }
        Ok(())
    }
    async fn complete(&self, _: JobId, _: LeaseToken) -> Result<(), StoreError> {
        Ok(())
    }
    async fn retry(
        &self,
        _: JobId,
        _: LeaseToken,
        run_at: DateTime<Utc>,
        _: &str,
    ) -> Result<(), StoreError> {
        self.retries.lock().unwrap().push(run_at);
        Ok(())
    }
    async fn fail(
        &self,
        _: JobId,
        _: LeaseToken,
        _: &str,
        not_before: Option<DateTime<Utc>>,
    ) -> Result<(), StoreError> {
        self.failures.lock().unwrap().push(not_before);
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
    async fn pending_poll(&self, _: SourceId, _: usize) -> Result<Option<PollCommit>, StoreError> {
        Ok(self.pending.lock().unwrap().take())
    }
    async fn active_delivery_count(&self, _: SourceId) -> Result<u64, StoreError> {
        Ok(self.active)
    }
    async fn commit_poll(&self, _: &LeasedWork, value: PollCommit) -> Result<(), StoreError> {
        if let Some(probe) = &self.heartbeat_probe {
            let _lock = probe.job_lock.lock().await;
            // Model a transaction that spans a heartbeat and holds the job row.
            probe.renewal_started.notified().await;
        }
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
        heartbeat_probe: None,
        claim: Mutex::new(None),
        retries: Mutex::new(vec![]),
        failures: Mutex::new(vec![]),
        source,
        active: 1,
        had_poll: false,
        record: None,
        targets: vec![],
        polls: Mutex::new(vec![]),
        pending: Mutex::new(None),
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
        IngestLimits::new(initial, 2, 4, 1_000_000, 1_000_000).unwrap(),
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
    let mut pending = {
        let polls = store.polls.lock().unwrap();
        assert_eq!(polls[0].records.len(), 1);
        assert_eq!(polls[0].records[0].record.key().title, "A");
        assert_eq!(polls[0].remainder.len(), 1);
        assert_eq!(polls[0].remainder[0].record.key().title, "B");
        polls[0].clone()
    };
    pending.records = std::mem::take(&mut pending.remainder);
    pending.incomplete = false;
    *store.pending.lock().unwrap() = Some(pending);
    let conditional = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/feed").unwrap(),
        content_type: None,
        body: vec![],
        validators: Default::default(),
        not_modified: true,
    }));
    worker(
        store.clone(),
        conditional.clone(),
        BrowserCapability::Degraded,
        1,
    )
    .handle(&lease(WorkItem::PollSource { source_id }), now())
    .await
    .unwrap();
    assert_eq!(conditional.calls.load(Ordering::SeqCst), 0);
    assert_eq!(store.polls.lock().unwrap()[1].records[0].key().title, "B");
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
        description_media_type: Some("text/plain".into()),
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
    let record_id = SourceRecordId::new();
    let store = Arc::new(fulltext_store(source, record_id));
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/a").unwrap(),
        content_type: Some("text/html".into()),
        body: b"<p>hello</p>".to_vec(),
        validators: Default::default(),
        not_modified: false,
    }));
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
    let record_id = SourceRecordId::new();
    let store = Arc::new(fulltext_store(source, record_id));
    let fetcher = fetch(Err(FetchError::Http(503)));
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
    let record_id = SourceRecordId::new();
    let store = Arc::new(fulltext_store(source, record_id));
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/a").unwrap(),
        content_type: Some("text/html".into()),
        body: vec![0xff],
        validators: Default::default(),
        not_modified: false,
    }));
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
                    description_media_type: Some("text/plain".into()),
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
        IngestLimits::new(1, 2, 4, 1_000_000, 1_000_000).unwrap(),
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

#[tokio::test]
async fn backoff_starts_after_failed_request_even_when_claim_is_old() {
    let source = source();
    let source_id = source.id();
    let store = Arc::new(store(source));
    *store.claim.lock().unwrap() = Some(lease(WorkItem::PollSource { source_id }));
    let before = Utc::now();
    let result = worker(
        store.clone(),
        fetch(Err(FetchError::Http(503))),
        BrowserCapability::Available,
        10,
    )
    .run_one("retry-test", before - Duration::minutes(1))
    .await;
    assert!(result.is_err());
    assert!(store.retries.lock().unwrap()[0] >= before + Duration::seconds(1));
}

#[tokio::test]
async fn paused_source_does_not_fetch_during_recovery_poll() {
    let source = source();
    let source_id = source.id();
    let mut storage = store(source);
    storage.active = 0;
    let storage = Arc::new(storage);
    let fetcher = fetch(Err(FetchError::Http(503)));
    worker(storage, fetcher.clone(), BrowserCapability::Available, 10)
        .handle(&lease(WorkItem::PollSource { source_id }), now())
        .await
        .unwrap();
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn server_retry_after_is_not_shortened_by_exponential_backoff() {
    let source = source();
    let source_id = source.id();
    let store = Arc::new(store(source));
    *store.claim.lock().unwrap() = Some(lease(WorkItem::PollSource { source_id }));
    let not_before = Utc::now() + Duration::hours(2);
    let result = worker(
        store.clone(),
        fetch(Err(FetchError::RetryAfter {
            status: 429,
            not_before,
        })),
        BrowserCapability::Available,
        10,
    )
    .run_one("retry-test", Utc::now())
    .await;
    assert!(result.is_err());
    assert_eq!(store.retries.lock().unwrap()[0], not_before);
}

#[tokio::test]
async fn rss_identical_repeat_policy_is_source_scoped_and_validates_before_commit() {
    let item = "<item><guid isPermaLink=\"false\">stable</guid><title>A</title><link>https://example.test/a</link><description>D</description></item>";
    for (kind, conflict, succeeds) in [
        (SourceKind::XmlFeed, false, false),
        (SourceKind::Auto, false, false),
        (SourceKind::RssCoalesceIdentical, false, true),
        (SourceKind::RssCoalesceIdentical, true, false),
    ] {
        let source = source().revise_kind(kind).unwrap();
        // Persisted kind must retain the explicit choice, without URL-based activation.
        let source: SourceDefinition =
            serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
        let source_id = source.id();
        let store = Arc::new(store(source));
        let second = if conflict {
            item.replace("</item>", "<category>changed</category></item>")
        } else {
            item.to_owned()
        };
        let body=format!("<rss version=\"2.0\"><channel><title>x</title><link>https://example.test</link><description>x</description>{item}{second}</channel></rss>").into_bytes();
        let fetcher = fetch(Ok(FetchedPage {
            final_url: url::Url::parse("https://example.test/feed").unwrap(),
            content_type: None,
            body,
            validators: Default::default(),
            not_modified: false,
        }));
        let result = worker(store.clone(), fetcher, BrowserCapability::Available, 10)
            .handle(&lease(WorkItem::PollSource { source_id }), now())
            .await;
        assert_eq!(result.is_ok(), succeeds);
        let polls = store.polls.lock().unwrap();
        if succeeds {
            assert_eq!(polls[0].records.len(), 1);
            assert_eq!(polls[0].records[0].record.upstream_id(), "stable");
            assert!(!polls[0].incomplete);
        } else {
            assert!(polls.is_empty());
        }
    }
}

#[tokio::test]
async fn heartbeat_renewal_does_not_stop_polling_the_commit_that_holds_its_lock() {
    let source = source();
    let source_id = source.id();
    let mut storage = store(source);
    storage.heartbeat_probe = Some(Arc::new(HeartbeatProbe::default()));
    let storage = Arc::new(storage);
    let fetcher = fetch(Ok(FetchedPage {
        final_url: Url::parse("https://example.test/feed").unwrap(),
        content_type: Some("application/rss+xml".into()),
        body: br#"<rss version="2.0"><channel><title>Test</title><link>https://example.test/</link><description>Test</description><item><guid>one</guid><title>One</title><link>https://example.test/one</link></item></channel></rss>"#.to_vec(),
        validators: CacheValidators::default(),
        not_modified: false,
    }));
    let worker = worker(storage.clone(), fetcher, BrowserCapability::Available, 10)
        .with_runtime_policy(
            Duration::milliseconds(10),
            5,
            Duration::seconds(300),
            Duration::zero(),
        )
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        worker.handle_with_heartbeat(&lease(WorkItem::PollSource { source_id })),
    )
    .await
    .expect("renewal must not deadlock against its own suspended commit")
    .unwrap();
    assert_eq!(storage.polls.lock().unwrap().len(), 1);
}

fn fulltext_store(source: SourceDefinition, id: SourceRecordId) -> StoreStub {
    let record = SourceRecord::from_parsed(
        id,
        source.id(),
        reader_collectors::ParsedRecord {
            upstream_id: "fixture".into(),
            original_url: "https://example.test/a".into(),
            absolute_url: Some(Url::parse("https://example.test/a").unwrap()),
            title: "Fixture".into(),
            description: None,
            description_media_type: None,
            content_html: Some(
                "<p>Exact caption</p><img src=\"https://example.test/photo.jpg\">".into(),
            ),
            published_at: None,
        },
    )
    .unwrap();
    let mut store = store(source);
    store.record = Some(record);
    store
}

#[tokio::test]
async fn telegram_fulltext_uses_persisted_post_body_without_fetching_landing_page() {
    let source = SourceDefinition::new(
        SourceId::new(),
        Url::parse("https://t.me/cdo_club").unwrap(),
        SourceKind::BuiltIn(BuiltInAdapter::Telegram {
            max_pages: std::num::NonZeroUsize::new(1).unwrap(),
        }),
    )
    .unwrap();
    let id = SourceRecordId::new();
    let storage = Arc::new(fulltext_store(source, id));
    let fetcher = fetch(Err(FetchError::Rejected(
        "must_not_fetch_telegram_landing_page".into(),
    )));
    worker(
        storage.clone(),
        fetcher.clone(),
        BrowserCapability::Available,
        10,
    )
    .handle(
        &lease(WorkItem::ExtractFullText {
            record_id: id,
            source_revision: 0,
            url: Url::parse("https://t.me/cdo_club/3078").unwrap(),
            manual: false,
        }),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    let manifest = storage.manifests.lock().unwrap();
    let raw: Vec<u8> = manifest[0]
        .raw_chunks
        .iter()
        .flat_map(|chunk| chunk.bytes.clone())
        .collect();
    assert!(String::from_utf8(raw).unwrap().contains("Exact caption"));
}

#[tokio::test]
async fn rate_limit_floor_jitter_and_longer_server_cooldown_are_preserved() {
    for (after_seconds, exhausted) in [
        (None, false),
        (Some(900), false),
        (None, true),
        (Some(900), true),
    ] {
        let source = source();
        let source_id = source.id();
        let storage = Arc::new(store(source));
        let mut job = lease(WorkItem::PollSource { source_id });
        job.job_id = JobId::from_uuid(Uuid::from_u128(42_000));
        *storage.claim.lock().unwrap() = Some(job);
        // Future logical claim time makes the exact configured jitter observable
        // without wall-clock sleeps or timing-sensitive upper bounds.
        let at = Utc::now() + Duration::days(1);
        let error = match after_seconds {
            Some(seconds) => FetchError::RetryAfter {
                status: 429,
                not_before: at + Duration::seconds(seconds),
            },
            None => FetchError::Http(429),
        };
        let worker = worker(
            storage.clone(),
            fetch(Err(error)),
            BrowserCapability::Available,
            10,
        )
        .with_runtime_policy(
            Duration::seconds(5),
            if exhausted { 1 } else { 5 },
            Duration::seconds(300),
            Duration::seconds(60),
        )
        .unwrap();
        assert!(worker.run_one("rate-limit-test", at).await.is_err());
        let expected = at + Duration::seconds(after_seconds.unwrap_or(342));
        if exhausted {
            assert!(storage.retries.lock().unwrap().is_empty());
            assert_eq!(
                storage.failures.lock().unwrap().as_slice(),
                &[Some(expected)]
            );
        } else {
            assert_eq!(storage.retries.lock().unwrap().as_slice(), &[expected]);
        }
    }
}

#[tokio::test]
async fn configured_retry_jitter_is_bounded_and_can_be_disabled() {
    for (id, spread, expected_ms) in [
        (0, 60, 0),
        (59_999, 60, 59_999),
        (60_000, 60, 0),
        (42, 0, 0),
    ] {
        let source = source();
        let source_id = source.id();
        let storage = Arc::new(store(source));
        let mut job = lease(WorkItem::PollSource { source_id });
        job.job_id = JobId::from_uuid(Uuid::from_u128(id));
        *storage.claim.lock().unwrap() = Some(job);
        let at = Utc::now() + Duration::days(1);
        let worker = worker(
            storage.clone(),
            fetch(Err(FetchError::Http(429))),
            BrowserCapability::Available,
            10,
        )
        .with_runtime_policy(
            Duration::seconds(5),
            5,
            Duration::seconds(300),
            Duration::seconds(spread),
        )
        .unwrap();
        assert!(worker.run_one("jitter-test", at).await.is_err());
        assert_eq!(
            storage.retries.lock().unwrap()[0],
            at + Duration::seconds(300) + Duration::milliseconds(expected_ms)
        );
    }
}

#[test]
fn retry_policy_rejects_invalid_or_unrepresentable_durations_before_execution() {
    for (delay, jitter) in [
        (Duration::zero(), Duration::zero()),
        (Duration::seconds(-1), Duration::zero()),
        (Duration::seconds(300), Duration::seconds(-1)),
        (Duration::MAX, Duration::zero()),
        (Duration::seconds(300), Duration::MAX),
    ] {
        let storage = Arc::new(store(source()));
        assert!(matches!(
            worker(
                storage,
                fetch(Err(FetchError::Http(429))),
                BrowserCapability::Available,
                10
            )
            .with_runtime_policy(Duration::seconds(5), 5, delay, jitter),
            Err(IngestError::InvalidRetryPolicy)
        ));
    }
}

#[tokio::test]
async fn dropbox_collection_preserves_rss_metadata_before_commit() {
    let source = SourceDefinition::new(
        SourceId::new(),
        Url::parse("https://dropbox.tech/feed").unwrap(),
        SourceKind::BuiltIn(BuiltInAdapter::Dropbox),
    )
    .unwrap();
    let source_id = source.id();
    let make = |title: &str, description: Option<&str>, body: Option<&str>, date: &str| {
        SourceRecord::from_parsed(
            SourceRecordId::new(),
            source_id,
            reader_collectors::ParsedRecord {
                upstream_id: "https://dropbox.tech/article".into(),
                original_url: "https://dropbox.tech/article".into(),
                absolute_url: Some(Url::parse("https://dropbox.tech/article").unwrap()),
                title: title.into(),
                description: description.map(str::to_owned),
                description_media_type: Some("text/html".into()),
                content_html: body.map(str::to_owned),
                published_at: reader_core::PublicationDate::parse(date),
            },
        )
        .unwrap()
    };
    let old = make(
        "Before",
        Some("Exact RSS description"),
        Some("<p>Entire RSS article</p>"),
        "2026-09-23T07:00:00+00:00",
    );
    let next = make("After", None, None, "2026-09-23");
    let mut storage = store(source);
    storage.had_poll = true;
    storage.record = Some(old.clone());
    let storage = Arc::new(storage);
    let fetcher = fetch(Err(FetchError::Rejected("unused".into())));
    let worker = IngestWorker::new(
        storage.clone(),
        fetcher.clone(),
        fetcher,
        Arc::new(BrowserRecords(vec![next])),
        IngestLimits::new(100, 2, 4, 1_000_000, 1_000_000).unwrap(),
        Duration::seconds(30),
    )
    .unwrap();
    worker
        .handle(&lease(WorkItem::CollectWebFeed { source_id }), now())
        .await
        .unwrap();
    let polls = storage.polls.lock().unwrap();
    assert_eq!(polls.len(), 1);
    assert_eq!(polls[0].records.len(), 1);
    let new = &polls[0].records[0].record;
    assert_eq!(new.id(), old.id());
    assert_eq!(new.key().description, old.key().description);
    assert_eq!(new.published_at(), old.published_at());
    assert_eq!(new.feed_content_html(), old.feed_content_html());
    assert_eq!(new.key().title, "After");
}
