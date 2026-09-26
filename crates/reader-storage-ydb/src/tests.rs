use super::*;
use std::time::Duration;

#[test]
fn production_client_limits_reject_unbounded_or_disabled_values() {
    assert!(YdbClientLimits::new(Duration::ZERO, 1, 1, Duration::from_millis(1)).is_err());
    assert!(YdbClientLimits::new(Duration::from_secs(1), 0, 1, Duration::from_millis(1)).is_err());
    assert!(YdbClientLimits::new(Duration::from_secs(1), 1, 0, Duration::from_millis(1)).is_err());
    assert!(YdbClientLimits::new(Duration::from_secs(1), 1, 1, Duration::ZERO).is_err());
    assert_eq!(
        YdbClientLimits::new(Duration::from_secs(15), 32, 5, Duration::from_millis(200)).unwrap(),
        YdbClientLimits {
            request_timeout: Duration::from_secs(15),
            max_concurrency: 32,
            retry_attempts: 5,
            retry_initial_backoff: Duration::from_millis(200),
        }
    );
}

#[test]
fn generic_imported_html_source_has_a_normalized_editable_recipe() {
    let workspace = WorkspaceId::new();
    let url = Url::parse("https://example.com/news").unwrap();
    let kind = imported_source_kind(&serde_json::json!({
        "id":"example",
        "name":"Example",
        "url":url,
        "link_selector":"article a",
        "card_selector":"article",
        "title_selector":"h2",
        "max_pages":4,
        "next_selector":"a.next"
    }))
    .unwrap();
    let document = editable_recipe_document(workspace, &url, &kind)
        .unwrap()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&document).unwrap();
    assert_eq!(value["workspaceId"], workspace.as_uuid().to_string());
    assert_eq!(value["selector"], "article a");
    assert_eq!(value["cardSelector"], "article");
    assert_eq!(value["titleSelector"], "h2");
    assert_eq!(value["maxPages"], 4);
    assert_eq!(value["nextPage"]["expression"], "a.next");
}
use async_trait::async_trait;
use chrono::Utc;
use reader_ingest::{ContentManifestPointer, ContentRevision};
use std::{collections::HashMap, sync::Mutex};
use url::Url;

#[derive(Default)]
struct Transport {
    rows: Mutex<HashMap<(&'static str, String), Vec<u8>>>,
    scans: Mutex<HashMap<&'static str, Vec<Vec<u8>>>>,
    ddl: Mutex<Vec<&'static str>>,
    cas: Mutex<Vec<(&'static str, String, Option<u64>, u64, Vec<u8>)>>,
    jobs: Mutex<Vec<(String, Vec<u8>)>>,
    ddl_failure: Mutex<Option<&'static str>>,
    ingest: Mutex<HashMap<String, (String, String, Option<String>)>>,
    evaluations: Mutex<usize>,
    account_workspaces: Mutex<Vec<(String, String, Vec<u8>, String, Vec<u8>)>>,
}

fn no<T>() -> Result<T, String> {
    Err("unused transport operation".into())
}

#[tokio::test]
async fn administrator_creation_persists_account_and_initial_workspace_atomically() {
    let transport = Arc::new(Transport::default());
    let repository =
        YdbRepository::new(transport.clone(), ReasonPolicy::new(128).unwrap(), 100).unwrap();
    let account = reader_application::AccountRecord {
        id: AccountId::new(),
        username: "admin".into(),
        password_hash: "encoded".into(),
        admin: true,
        auth_revision: 0,
        revision: 0,
    };
    let workspace = Workspace::new(WorkspaceId::new(), account.id, "Personal".into());

    repository
        .create_account_and_workspace(account.clone(), workspace.clone())
        .await
        .unwrap();

    let writes = transport.account_workspaces.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, "admin");
    assert_eq!(writes[0].1, account.id.as_uuid().to_string());
    assert_eq!(writes[0].3, workspace.id().as_uuid().to_string());
    assert_eq!(
        serde_json::from_slice::<Workspace>(&writes[0].4).unwrap(),
        workspace
    );
}

#[async_trait]
impl YdbTransport for Transport {
    async fn read(&self, table: &'static str, key: String) -> Result<Option<Vec<u8>>, String> {
        Ok(self.rows.lock().unwrap().get(&(table, key)).cloned())
    }
    async fn scan_prefix(&self, table: &'static str, _: String) -> Result<Vec<Vec<u8>>, String> {
        Ok(self
            .scans
            .lock()
            .unwrap()
            .get(table)
            .cloned()
            .unwrap_or_default())
    }
    async fn compare_and_swap(
        &self,
        table: &'static str,
        key: String,
        expected: Option<u64>,
        revision: u64,
        document: Vec<u8>,
    ) -> Result<bool, String> {
        self.cas
            .lock()
            .unwrap()
            .push((table, key, expected, revision, document));
        Ok(true)
    }
    async fn delete(&self, _: &'static str, _: String, _: u64) -> Result<bool, String> {
        no()
    }
    async fn atomic_create_account(
        &self,
        _: String,
        _: String,
        _: Vec<u8>,
    ) -> Result<bool, String> {
        no()
    }
    async fn atomic_create_account_and_workspace(
        &self,
        username: String,
        account_key: String,
        account_document: Vec<u8>,
        workspace_key: String,
        workspace_document: Vec<u8>,
    ) -> Result<bool, String> {
        self.account_workspaces.lock().unwrap().push((
            username,
            account_key,
            account_document,
            workspace_key,
            workspace_document,
        ));
        Ok(true)
    }
    async fn atomic_accept_invite(
        &self,
        _: String,
        _: u64,
        _: u64,
        _: Vec<u8>,
        _: String,
        _: String,
        _: Vec<u8>,
        _: String,
        _: Vec<u8>,
    ) -> Result<bool, String> {
        no()
    }
    async fn atomic_reset_password(
        &self,
        _: String,
        _: u64,
        _: u64,
        _: Vec<u8>,
        _: String,
        _: u64,
        _: u64,
        _: Vec<u8>,
    ) -> Result<bool, String> {
        no()
    }
    async fn restore_workspace_with_refreshes(
        &self,
        _: String,
        _: u64,
        _: u64,
        _: Vec<u8>,
        _: Vec<(String, String, String, Vec<u8>)>,
        _: usize,
    ) -> Result<bool, String> {
        no()
    }
    async fn execute_ddl(&self, statement: &'static str) -> Result<(), String> {
        self.ddl.lock().unwrap().push(statement);
        if self.ddl_failure.lock().unwrap().as_ref() == Some(&statement) {
            Err("ddl failed".into())
        } else {
            Ok(())
        }
    }
    async fn provision_subscription(
        &self,
        _: String,
        _: Vec<u8>,
        _: String,
        _: String,
        _: Vec<u8>,
        _: String,
        _: Vec<u8>,
        _: usize,
        _: Option<String>,
        _: bool,
        _: Option<String>,
    ) -> Result<(), String> {
        no()
    }
    async fn update_web_feed_recipe(
        &self,
        _: String,
        _: u64,
        _: u64,
        _: String,
        _: Vec<u8>,
        _: String,
        _: Vec<u8>,
    ) -> Result<bool, String> {
        no()
    }
    async fn web_feed_recipe(&self, _: String) -> Result<Option<(u64, String)>, String> {
        Ok(None)
    }
    async fn atomic_cas_many(
        &self,
        _: &'static str,
        _: Vec<(String, u64, u64, Vec<u8>)>,
    ) -> Result<bool, String> {
        no()
    }
    async fn provision_subscriptions(
        &self,
        _: Vec<(String, Vec<u8>, String, String, Vec<u8>, String)>,
        _: usize,
    ) -> Result<(), String> {
        no()
    }
    async fn presentation_metadata(
        &self,
        _: String,
        _: String,
    ) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>), String> {
        no()
    }
    async fn enqueue_ingest_job(&self, id: String, item: Vec<u8>) -> Result<(), String> {
        self.jobs.lock().unwrap().push((id, item));
        Ok(())
    }
    async fn refresh_subscription(
        &self,
        _: String,
        _: String,
        id: String,
        item: Vec<u8>,
        _: usize,
    ) -> Result<(), String> {
        self.jobs.lock().unwrap().push((id, item));
        Ok(())
    }
    async fn activate_subscription_with_refresh(
        &self,
        _: String,
        _: u64,
        _: u64,
        _: Vec<u8>,
        _: String,
        id: String,
        item: Vec<u8>,
        _: usize,
    ) -> Result<bool, String> {
        self.jobs.lock().unwrap().push((id, item));
        Ok(true)
    }
    async fn source_id_for_subscription(&self, _: String) -> Result<Option<String>, String> {
        Ok(None)
    }
    async fn replace_subscription_source(
        &self,
        _: String,
        _: u64,
        _: u64,
        _: Vec<u8>,
        _: String,
        _: String,
        _: String,
        _: String,
        _: Vec<u8>,
        _: String,
        _: Vec<u8>,
    ) -> Result<bool, String> {
        no()
    }
    async fn subscription_stats(
        &self,
        _: String,
        _: Vec<String>,
    ) -> Result<
        Vec<(
            String,
            usize,
            usize,
            Option<i64>,
            Option<i64>,
            u32,
            bool,
            Option<String>,
            bool,
            String,
        )>,
        String,
    > {
        Ok(vec![])
    }
    async fn subscription_activity(&self, _: String, _: i64) -> Result<Vec<Vec<u8>>, String> {
        Ok(vec![])
    }
    async fn apply_seed(
        &self,
        _: Vec<(
            String,
            Vec<u8>,
            String,
            String,
            Vec<u8>,
            String,
            Vec<u8>,
            String,
            Vec<u8>,
            Option<Vec<u8>>,
        )>,
        _: usize,
    ) -> Result<(), String> {
        no()
    }
    async fn ingest_job(
        &self,
        id: String,
    ) -> Result<Option<(String, String, Option<String>)>, String> {
        Ok(self.ingest.lock().unwrap().get(&id).cloned())
    }
    async fn ingest_jobs(&self) -> Result<Vec<(String, String, Option<String>)>, String> {
        Ok(self.ingest.lock().unwrap().values().cloned().collect())
    }
    async fn rule_evaluation_count(&self, _: String, _: String, _: u64) -> Result<usize, String> {
        Ok(*self.evaluations.lock().unwrap())
    }
}

#[tokio::test]
async fn schema_marker_is_not_advanced_when_any_ddl_fails() {
    let transport = Transport::default();
    *transport.ddl_failure.lock().unwrap() = Some(SCHEMA_DDL[2]);
    assert!(prepare_schema(&transport).await.is_err());
    assert!(transport.cas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn schema_preparation_upgrades_an_existing_marker_after_ddl() {
    let transport = Transport::default();
    transport.rows.lock().unwrap().insert(
        ("schema_metadata", "schema".into()),
        serde_json::to_vec(&SchemaMarker { version: 0 }).unwrap(),
    );
    prepare_schema(&transport).await.unwrap();
    let cas = transport.cas.lock().unwrap();
    assert_eq!(cas[0].2, Some(0));
    assert_eq!(cas[0].3, SCHEMA_VERSION);
}

#[tokio::test]
async fn schema_v6_upgrade_executes_only_v7_ddl() {
    let transport = Transport::default();
    transport.rows.lock().unwrap().insert(
        ("schema_metadata", "schema".into()),
        serde_json::to_vec(&SchemaMarker { version: 6 }).unwrap(),
    );
    prepare_schema(&transport).await.unwrap();
    let ddl = transport.ddl.lock().unwrap();
    assert_eq!(ddl[0], SCHEMA_DDL[0]);
    assert_eq!(&ddl[1..], SCHEMA_V7_DDL);
}

#[tokio::test]
async fn schema_preparation_executes_every_statement_before_persisting_version_marker() {
    let transport = Transport::default();
    prepare_schema(&transport).await.unwrap();
    assert_eq!(transport.ddl.lock().unwrap().as_slice(), SCHEMA_DDL);
    let cas = transport.cas.lock().unwrap();
    assert_eq!(cas.len(), 1);
    assert_eq!(cas[0].0, "schema_metadata");
    assert_eq!(cas[0].1, "schema");
    assert_eq!(cas[0].2, None);
    assert_eq!(cas[0].3, SCHEMA_VERSION);
    let marker: SchemaMarker = serde_json::from_slice(&cas[0].4).unwrap();
    assert_eq!(marker.version, SCHEMA_VERSION);
}

#[tokio::test]
async fn rule_application_job_is_deterministic_and_freezes_current_article_boundary() {
    let transport = Arc::new(Transport::default());
    let workspace = WorkspaceId::new();
    let article_ids = [ArticleId::new(), ArticleId::new()];
    let articles: Vec<_> = article_ids
        .into_iter()
        .map(|id| Article {
            id,
            key: DedupKey {
                location: ArticleLocation::from(Url::parse("https://example.test/item").unwrap()),
                title: id.as_uuid().to_string(),
                description: None,
            },
            state: ArticleState::default(),
            first_arrived_at: Utc::now(),
            origins: vec![],
            revision: 0,
        })
        .collect();
    transport.scans.lock().unwrap().insert(
        "articles",
        articles
            .iter()
            .map(|v| serde_json::to_vec(v).unwrap())
            .collect(),
    );
    let repository =
        YdbRepository::new(transport.clone(), ReasonPolicy::new(128).unwrap(), 100).unwrap();
    let rule = Rule {
        id: RuleId::new(),
        subscription_id: SubscriptionId::new(),
        version: 4,
        enabled: true,
        field: RuleField::Both,
        needles: vec!["item".into()],
        action: RuleAction::MarkRead,
    };

    repository
        .enqueue_rule_application(workspace, rule.clone())
        .await
        .unwrap();
    repository
        .enqueue_rule_application(workspace, rule)
        .await
        .unwrap();

    let jobs = transport.jobs.lock().unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(
        jobs[0].0, jobs[1].0,
        "retries must address the same durable job"
    );
    let first: WorkItem = serde_json::from_slice(&jobs[0].1).unwrap();
    let second: WorkItem = serde_json::from_slice(&jobs[1].1).unwrap();
    assert_eq!(first, second);
    match first {
        WorkItem::ApplyRule {
            workspace_id,
            rule_version,
            after_article,
            through_article,
            ..
        } => {
            assert_eq!(workspace_id, workspace);
            assert_eq!(rule_version, 4);
            assert_eq!(after_article, None);
            assert_eq!(
                through_article,
                article_ids.into_iter().max_by_key(|v| v.as_uuid())
            );
        }
        other => panic!("unexpected job: {other:?}"),
    }
}

#[test]
fn schema_contains_durable_uniqueness_rate_limit_and_rule_provenance_tables() {
    let schema = SCHEMA_DDL.join("\n");
    for table in [
        "username_reservations",
        "login_attempts",
        "rule_evaluations",
        "ingest_jobs",
        "library_origins",
    ] {
        assert!(
            schema.contains(&format!("CREATE TABLE IF NOT EXISTS {table} ")),
            "missing {table}"
        );
    }
    assert!(SCHEMA_DDL
        .iter()
        .all(|statement| statement.starts_with("CREATE TABLE IF NOT EXISTS ")));
    assert!(schema.contains("PRIMARY KEY (workspace_id, article_id, rule_id, rule_version)"));
    assert!(schema.contains("first_attempt_ms Int64 NOT NULL"));
    assert!(schema.contains("origin_key Utf8 NOT NULL"));
    assert!(
        schema.contains("INDEX leased_by_origin GLOBAL ON (status, origin_key, lease_deadline_ms)")
    );
    assert!(schema.contains("document Utf8 NOT NULL"));
    assert!(schema.contains("CREATE TABLE IF NOT EXISTS subscription_activity"));
    assert!(schema.contains("INDEX expired_activity GLOBAL ON (occurred_at_ms)"));
    assert!(schema.contains("PRIMARY KEY (subscription_id, occurred_at_ms, id)"));
    assert!(schema.contains("CREATE TABLE IF NOT EXISTS workspace_feed_urls"));
    assert!(!schema.contains("document String"));
    assert!(!schema.contains("item String"));
    assert!(!schema.contains("dedup_key String"));
    assert!(!schema.contains("bytes String"));
}

#[test]
fn retry_age_and_origin_capacity_are_strict_at_the_boundary() {
    assert!(!crate::ingest_store::retry_age_exceeded(100, 100));
    assert!(crate::ingest_store::retry_age_exceeded(99, 100));
    assert!(crate::ingest_store::origin_has_capacity(1, 2));
    assert!(!crate::ingest_store::origin_has_capacity(2, 2));
}

#[test]
fn durable_job_identity_covers_revision_subscription_and_refresh_generation() {
    let record = SourceRecordId::new();
    assert_ne!(
        crate::ingest_store::record_job("fanout", record, 1),
        crate::ingest_store::record_job("fanout", record, 2),
    );
    assert_ne!(
        crate::ingest_store::record_job("fanout", record, 1),
        crate::ingest_store::record_job("fulltext", record, 1),
    );
    let article = ArticleId::new();
    let first = SubscriptionId::new();
    let second = SubscriptionId::new();
    assert_ne!(
        crate::ingest_store::article_rule_job(article, first, record, 1),
        crate::ingest_store::article_rule_job(article, second, record, 1),
        "each linked subscription must enqueue its own pinned rule evaluation",
    );
    assert_ne!(
        crate::ingest_store::cleanup_job(record, uuid::Uuid::new_v4()),
        crate::ingest_store::cleanup_job(record, uuid::Uuid::new_v4()),
    );
}

#[test]
fn feed_and_web_feed_collection_jobs_remain_scheduled_after_success() {
    let source = SourceId::new();
    assert!(crate::ingest_store::is_recurring(&WorkItem::PollSource {
        source_id: source
    }));
    assert!(crate::ingest_store::is_recurring(
        &WorkItem::CollectWebFeed { source_id: source }
    ));
    assert!(!crate::ingest_store::is_recurring(
        &WorkItem::RefreshSource { source_id: source }
    ));
}

#[test]
fn manual_content_refresh_can_replace_same_source_revision_but_not_a_newer_refresh() {
    let record = SourceRecordId::new();
    let current = ContentManifestPointer {
        record_id: record,
        source_revision: 7,
        refresh_id: uuid::Uuid::new_v4(),
        raw_chunks: 1,
        safe_html_chunks: 1,
        fetched_at: Utc::now(),
        final_url: Url::parse("https://example.test/article").unwrap(),
    };
    let newer = ContentRevision {
        record_id: record,
        source_revision: 7,
        refresh_id: uuid::Uuid::new_v4(),
        raw_chunks: vec![],
        safe_html_chunks: vec![],
        fetched_at: current.fetched_at + chrono::Duration::seconds(1),
        final_url: current.final_url.clone(),
    };
    let older = ContentRevision {
        fetched_at: current.fetched_at - chrono::Duration::seconds(1),
        ..newer.clone()
    };
    assert!(!crate::ingest_store::manifest_is_current_or_newer(
        &current, &newer
    ));
    assert!(crate::ingest_store::manifest_is_current_or_newer(
        &current, &older
    ));
}

#[test]
fn http_origin_preserves_scheme_host_and_effective_port() {
    assert_eq!(
        http_origin(&Url::parse("https://example.test:8443/path?q=secret").unwrap()).unwrap(),
        "https://example.test:8443"
    );
    assert!(http_origin(&Url::parse("file:///tmp/feed").unwrap()).is_err());
}

#[test]
fn every_known_source_configuration_has_a_validated_runtime_mapping() {
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../../../source-inventory/inventory.json")).unwrap();
    let sources = inventory["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 42);
    let mut feed = 0;
    let mut web = 0;
    let mut built_in = 0;
    for source in sources {
        match imported_source_kind(&source["configuration"]).unwrap() {
            SourceKind::Auto => feed += 1,
            SourceKind::WebPage(_) => web += 1,
            SourceKind::BuiltIn(_) => built_in += 1,
            other => panic!("unexpected imported source kind: {other:?}"),
        }
    }
    assert_eq!((feed, web, built_in), (2, 33, 7));
}

#[test]
fn built_in_adapter_is_not_reported_as_a_plain_feed() {
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../../../source-inventory/inventory.json")).unwrap();
    let built_in = inventory["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|source| {
            let kind = imported_source_kind(&source["configuration"]).ok()?;
            matches!(kind, SourceKind::BuiltIn(_)).then_some(kind)
        })
        .unwrap();
    assert_eq!(subscription_source_type(&built_in), "built_in");
}

#[test]
fn subscription_counts_unique_articles_and_only_actual_unread_state() {
    let articles = ["read".to_owned(), "unread".to_owned()]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let unread = ["unread".to_owned(), "other-subscription".to_owned()]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        subscription_article_counts(Some(&articles), &unread),
        (2, 1)
    );
    assert_eq!(subscription_article_counts(None, &unread), (0, 0));
}

#[tokio::test]
async fn rule_application_progress_is_scoped_and_reports_durable_evaluations() {
    let transport = Arc::new(Transport::default());
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let rule_id = RuleId::new();
    let operation = uuid::Uuid::new_v4();
    let item = WorkItem::ApplyRule {
        workspace_id: workspace,
        rule_id,
        rule_version: 3,
        after_article: None,
        through_article: Some(ArticleId::new()),
    };
    transport.ingest.lock().unwrap().insert(
        operation.to_string(),
        ("leased".into(), serde_json::to_string(&item).unwrap(), None),
    );
    *transport.evaluations.lock().unwrap() = 17;
    let rule = Rule {
        id: rule_id,
        subscription_id: SubscriptionId::new(),
        version: 3,
        enabled: true,
        field: RuleField::Both,
        needles: vec!["rust".into()],
        action: RuleAction::MarkRead,
    };
    transport.rows.lock().unwrap().insert(
        (
            "rules",
            format!("{}/{}", workspace.as_uuid(), rule_id.as_uuid()),
        ),
        serde_json::to_vec(&rule).unwrap(),
    );
    let repository = YdbRepository::new(transport, ReasonPolicy::new(128).unwrap(), 100).unwrap();
    let progress = repository
        .rule_application_progress(workspace, operation)
        .await
        .unwrap();
    assert_eq!(progress.status, "running");
    assert_eq!(progress.evaluated, 17);
    assert!(matches!(
        repository.rule_application_progress(other, operation).await,
        Err(RepositoryError::NotFound)
    ));
}

#[tokio::test]
async fn repository_fails_closed_when_persisted_reason_exceeds_current_policy() {
    let transport = Arc::new(Transport::default());
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Reading".into());
    workspace.archive(WorkspaceStateEvent {
        reason: ReasonPolicy::new(128)
            .unwrap()
            .validate("reason longer than limit".into())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc::now(),
    });
    transport.rows.lock().unwrap().insert(
        ("workspaces", workspace.id().as_uuid().to_string()),
        serde_json::to_vec(&workspace).unwrap(),
    );
    let repository = YdbRepository::new(transport, ReasonPolicy::new(8).unwrap(), 100).unwrap();
    assert!(matches!(
        repository.workspace(workspace.id()).await,
        Err(RepositoryError::Storage(_))
    ));
}

#[test]
fn activity_retention_continues_past_more_than_one_hundred_inactive_rows() {
    let mut remaining = 205usize;
    let mut rounds = 0;
    loop {
        let deleted = remaining.min(100);
        remaining -= deleted;
        rounds += 1;
        if super::activity_retention_complete(deleted) {
            break;
        }
    }
    assert_eq!(rounds, 4);
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn subscription_activity_rejects_a_different_workspace_owner() {
    let transport = Arc::new(Transport::default());
    let owner = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "Private".into());
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        Url::parse("https://example.test/shared-feed").unwrap(),
        "Private subscription".into(),
    );
    transport.rows.lock().unwrap().insert(
        ("workspaces", workspace.id().as_uuid().to_string()),
        serde_json::to_vec(&workspace).unwrap(),
    );
    transport.rows.lock().unwrap().insert(
        ("subscriptions", subscription.id().as_uuid().to_string()),
        serde_json::to_vec(&subscription).unwrap(),
    );
    let repository = YdbRepository::new(transport, ReasonPolicy::new(128).unwrap(), 100).unwrap();
    assert!(matches!(
        repository
            .subscription_activity(
                AccountId::new(),
                subscription.id(),
                chrono::Utc::now() - chrono::Duration::days(30),
            )
            .await,
        Err(RepositoryError::NotFound)
    ));
}

#[tokio::test]
async fn import_and_seed_reject_cross_workspace_values_before_transport() {
    let transport = Arc::new(Transport::default());
    let repository = YdbRepository::new(transport, ReasonPolicy::new(128).unwrap(), 100).unwrap();
    let target = WorkspaceId::new();
    let foreign = Subscription::new(
        SubscriptionId::new(),
        WorkspaceId::new(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    assert!(matches!(
        repository
            .import_subscriptions_atomic(target, vec![foreign.clone()])
            .await,
        Err(RepositoryError::Storage(_))
    ));
    assert!(matches!(
        repository
            .apply_seed_atomic(
                target,
                vec![(
                    "key".into(),
                    foreign,
                    reader_application::SeedSource::Feed,
                    serde_json::json!({})
                )]
            )
            .await,
        Err(RepositoryError::Storage(_))
    ));
}
