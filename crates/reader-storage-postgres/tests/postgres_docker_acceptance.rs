use chrono::{Duration as ChronoDuration, Utc};
use reader_application::{AccountRecord, ReaderRepository, RepositoryError};
use reader_core::{
    AccountId, Article, ArticleId, ArticleLocation, ArticleState, DedupKey, ReasonPolicy, SourceId,
    Subscription, SubscriptionId, Workspace, WorkspaceId,
};
use reader_ingest::{
    IngestStore, JobId, LeaseToken, SourceDefinition, SourceKind, StoreError, WorkItem,
};
use reader_storage_postgres::{prepare_schema, PostgresIngestStore, PostgresRepository};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::{
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const POSTGRES_IMAGE: &str =
    "postgres:17-bookworm@sha256:91eb910c44c7ed13f7f1a4ccadaa9ca72ef14cddc04cacb6e070e48eb44731a3";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(90);

struct PostgresContainer {
    name: String,
}

impl PostgresContainer {
    fn start() -> Self {
        require_docker();
        let name = format!("inoreader-postgres-acceptance-{}", Uuid::new_v4());
        let output = command(
            "docker",
            &[
                "run",
                "--detach",
                "--name",
                &name,
                "--security-opt",
                "no-new-privileges:true",
                "--publish",
                "127.0.0.1::5432",
                "--env",
                "POSTGRES_DB=inoreader",
                "--env",
                "POSTGRES_USER=inoreader",
                "--env",
                "POSTGRES_PASSWORD=acceptance-only-password",
                POSTGRES_IMAGE,
            ],
        );
        assert_success("start pinned PostgreSQL container", &output);
        Self { name }
    }

    fn connection_string(&self) -> String {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            let output = command("docker", &["port", &self.name, "5432/tcp"]);
            if output.status.success() {
                let mapping = String::from_utf8(output.stdout)
                    .expect("docker port output must be UTF-8")
                    .trim()
                    .to_owned();
                if let Some(port) = mapping.rsplit(':').next().filter(|port| !port.is_empty()) {
                    return format!(
                        "postgres://inoreader:acceptance-only-password@127.0.0.1:{port}/inoreader"
                    );
                }
            }

            let running = command(
                "docker",
                &["inspect", "--format", "{{.State.Running}}", &self.name],
            );
            assert!(
                running.status.success()
                    && String::from_utf8_lossy(&running.stdout).trim() == "true",
                "PostgreSQL container exited during startup: {}",
                self.diagnostics()
            );
            assert!(
                Instant::now() < deadline,
                "PostgreSQL container did not publish its port within {STARTUP_TIMEOUT:?}: {}",
                self.diagnostics()
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    fn diagnostics(&self) -> String {
        let output = command("docker", &["logs", &self.name]);
        format!(
            "status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

impl Drop for PostgresContainer {
    fn drop(&mut self) {
        let output = command("docker", &["rm", "--force", &self.name]);
        if !output.status.success() {
            eprintln!(
                "failed to remove PostgreSQL acceptance container {}: {}",
                self.name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[tokio::test]
async fn real_postgres_creates_the_complete_idempotent_schema() {
    let container = PostgresContainer::start();
    let connection = container.connection_string();
    let pool = connect_when_ready(&connection)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "PostgreSQL Docker acceptance failed: {error}\ncontainer diagnostics: {}",
                container.diagnostics()
            )
        });

    prepare_schema(&pool)
        .await
        .expect("prepare PostgreSQL schema");
    prepare_schema(&pool)
        .await
        .expect("prepare PostgreSQL schema a second time");

    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE'",
    )
    .fetch_one(&pool)
    .await
    .expect("count schema tables");
    assert_eq!(table_count, 33);

    let index_names: Vec<String> = sqlx::query_scalar(
        "SELECT indexname FROM pg_indexes WHERE schemaname = 'public' AND indexname = ANY($1)",
    )
    .bind(
        &[
            "ready_jobs",
            "leased_by_origin",
            "recent_by_source",
            "by_subscription",
            "expired_activity",
        ][..],
    )
    .fetch_all(&pool)
    .await
    .expect("read operational indexes");
    assert_eq!(index_names.len(), 5);

    sqlx::query("INSERT INTO workspaces (id, revision, document) VALUES ($1, $2, $3)")
        .bind("workspace-1")
        .bind(0_i64)
        .bind("{\"preserved\": \"exactly\"}")
        .execute(&pool)
        .await
        .expect("insert workspace row");
    let row = sqlx::query("SELECT revision, document FROM workspaces WHERE id = $1")
        .bind("workspace-1")
        .fetch_one(&pool)
        .await
        .expect("read workspace row");
    assert_eq!(row.get::<i64, _>("revision"), 0);
    assert_eq!(
        row.get::<String, _>("document"),
        "{\"preserved\": \"exactly\"}"
    );

    let duplicate =
        sqlx::query("INSERT INTO workspaces (id, revision, document) VALUES ($1, $2, $3)")
            .bind("workspace-1")
            .bind(1_i64)
            .bind("{}")
            .execute(&pool)
            .await;
    assert!(
        duplicate.is_err(),
        "primary key must reject duplicate identity"
    );

    let negative_revision =
        sqlx::query("INSERT INTO workspaces (id, revision, document) VALUES ($1, $2, $3)")
            .bind("workspace-2")
            .bind(-1_i64)
            .bind("{}")
            .execute(&pool)
            .await;
    assert!(
        negative_revision.is_err(),
        "revision constraint must reject negative values"
    );

    verify_lease_fencing(&pool).await;
    verify_repository_isolation(&pool).await;
}

async fn verify_repository_isolation(pool: &PgPool) {
    let repository = PostgresRepository::new(
        pool.clone(),
        ReasonPolicy::new(4096).expect("valid reason policy"),
        100,
    )
    .expect("valid repository limits");
    let account_a = AccountRecord {
        id: AccountId::new(),
        username: format!("acceptance-a-{}", Uuid::new_v4()),
        password_hash: "acceptance-hash-a".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let account_b = AccountRecord {
        id: AccountId::new(),
        username: format!("acceptance-b-{}", Uuid::new_v4()),
        password_hash: "acceptance-hash-b".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let workspace_a = Workspace::new(WorkspaceId::new(), account_a.id, "Workspace A".into());
    let workspace_b = Workspace::new(WorkspaceId::new(), account_b.id, "Workspace B".into());
    repository
        .create_account_and_workspace(account_a.clone(), workspace_a.clone())
        .await
        .expect("create first account and workspace");
    repository
        .create_account_and_workspace(account_b.clone(), workspace_b.clone())
        .await
        .expect("create second account and workspace");

    assert_eq!(
        repository
            .workspaces_by_owner(account_a.id)
            .await
            .expect("list first owner's workspaces"),
        vec![workspace_a.clone()]
    );
    assert_eq!(
        repository
            .workspaces_by_owner(account_b.id)
            .await
            .expect("list second owner's workspaces"),
        vec![workspace_b.clone()]
    );

    let feed_url = url::Url::parse("https://shared.acceptance.example/feed.xml")
        .expect("valid shared feed URL");
    let mut subscription_a = Subscription::new(
        SubscriptionId::new(),
        workspace_a.id(),
        feed_url.clone(),
        "Feed A".into(),
    );
    let subscription_b = Subscription::new(
        SubscriptionId::new(),
        workspace_b.id(),
        feed_url,
        "Feed B".into(),
    );
    repository
        .save_subscription(None, subscription_a.clone())
        .await
        .expect("create first subscription");
    repository
        .save_subscription(None, subscription_b.clone())
        .await
        .expect("create second subscription to shared source");

    subscription_a.rename("Private rename".into());
    repository
        .save_subscription(Some(0), subscription_a.clone())
        .await
        .expect("rename first subscription");
    assert_eq!(
        repository
            .subscription(subscription_b.id())
            .await
            .expect("read second subscription")
            .title(),
        "Feed B"
    );
    assert_eq!(
        repository
            .subscriptions_by_workspace(workspace_a.id())
            .await
            .expect("list first workspace subscriptions"),
        vec![subscription_a.clone()]
    );
    assert_eq!(
        repository
            .subscriptions_by_workspace(workspace_b.id())
            .await
            .expect("list second workspace subscriptions"),
        vec![subscription_b.clone()]
    );

    let (shared_source, shared_source_count): (String, i64) = sqlx::query_as(
        "SELECT min(source_id), count(DISTINCT source_id) FROM subscription_sources
         WHERE subscription_id = ANY($1)",
    )
    .bind(
        &[
            subscription_a.id().as_uuid().to_string(),
            subscription_b.id().as_uuid().to_string(),
        ][..],
    )
    .fetch_one(pool)
    .await
    .expect("count shared source identities");
    assert_eq!(
        shared_source_count, 1,
        "public fetch identity may be shared"
    );

    let shared_source = SourceId::from_uuid(
        Uuid::parse_str(&shared_source).expect("stored source identity must be a UUID"),
    );
    let activity_job = JobId::new();
    let activity_item = WorkItem::RefreshSource {
        source_id: shared_source,
    };
    let activity_at = Utc::now();
    sqlx::query(
        "INSERT INTO ingest_jobs
         (id, status, run_at_ms, first_attempt_ms, origin_key, attempt, item, revision)
         VALUES ($1, 'ready', -1000, $2, $3, 0, $4, 0)",
    )
    .bind(activity_job.as_uuid().to_string())
    .bind(activity_at.timestamp_millis())
    .bind("https://shared.acceptance.example")
    .bind(serde_json::to_string(&activity_item).expect("serialize activity work item"))
    .execute(pool)
    .await
    .expect("insert activity job");
    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        4,
        ChronoDuration::days(1),
    )
    .expect("valid scheduler limits");
    let lease = store
        .claim(
            "activity-worker",
            activity_at,
            activity_at + ChronoDuration::seconds(30),
        )
        .await
        .expect("claim activity job")
        .expect("activity job must be claimable");
    assert_eq!(lease.job_id, activity_job);
    store
        .record_source_success(&lease, shared_source, activity_at, false, 17)
        .await
        .expect("record shared source success");
    let activity = repository
        .subscription_activity(
            account_a.id,
            subscription_a.id(),
            activity_at - ChronoDuration::seconds(1),
        )
        .await
        .expect("read owned activity");
    assert_eq!(activity.len(), 1);
    assert!(activity[0].successful);
    assert_eq!(activity[0].duration_ms, Some(17));

    assert!(matches!(
        repository
            .subscription_activity(
                account_b.id,
                subscription_a.id(),
                Utc::now() - ChronoDuration::days(1),
            )
            .await,
        Err(RepositoryError::NotFound)
    ));

    let article_id = ArticleId::new();
    let article = Article {
        id: article_id,
        key: DedupKey {
            location: ArticleLocation::url(
                "https://shared.acceptance.example/article".into(),
                url::Url::parse("https://shared.acceptance.example/article")
                    .expect("valid article URL"),
            ),
            title: "Private article".into(),
            description: None,
        },
        state: ArticleState::default(),
        first_arrived_at: Utc::now(),
        origins: vec![],
        revision: 0,
    };
    repository
        .save_article(workspace_a.id(), None, article.clone())
        .await
        .expect("save first workspace article");
    assert_eq!(
        repository
            .article(workspace_a.id(), article_id)
            .await
            .expect("read owned article"),
        article
    );
    assert!(matches!(
        repository.article(workspace_b.id(), article_id).await,
        Err(RepositoryError::NotFound)
    ));
    assert!(repository
        .articles_by_workspace(workspace_b.id())
        .await
        .expect("list second workspace articles")
        .is_empty());

    let mut legacy = Subscription::new(
        SubscriptionId::new(),
        workspace_a.id(),
        url::Url::parse("http://china-radio-international.duckdns.org:8080/example.xml")
            .expect("valid legacy URL"),
        "Legacy title".into(),
    );
    legacy.set_personal_note("private retained note".into());
    repository
        .save_subscription(None, legacy.clone())
        .await
        .expect("create legacy personal_feed subscription");
    let canonical = url::Url::parse("https://publisher.example/blog").expect("valid publisher URL");
    let preexisting_id = SourceId::from_uuid(Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        canonical.as_str().as_bytes(),
    ));
    let preexisting = SourceDefinition::new(preexisting_id, canonical, SourceKind::Auto)
        .expect("valid preexisting automatic source");
    sqlx::query("INSERT INTO sources(id,revision,document) VALUES($1,0,$2)")
        .bind(preexisting_id.as_uuid().to_string())
        .bind(serde_json::to_string(&preexisting).expect("serialize preexisting source"))
        .execute(pool)
        .await
        .expect("insert preexisting source without URL index");
    let migrated = repository
        .migrate_personal_feed_links_atomic(
            workspace_a.id(),
            vec![(
                "example".into(),
                serde_json::json!({
                    "id":"example",
                    "name":"Original publisher",
                    "url":"https://publisher.example/blog",
                    "link_selector":"article a[href]",
                    "url_pattern":"^https://publisher\\.example/posts/"
                }),
            )],
        )
        .await
        .expect("migrate legacy subscription atomically");
    assert_eq!(migrated, 1);
    let migrated = repository
        .subscription(legacy.id())
        .await
        .expect("read migrated subscription");
    assert_eq!(migrated.personal_note(), "private retained note");
    assert_eq!(
        migrated.source_url_exact(),
        "https://publisher.example/blog"
    );
    let source_document: String = sqlx::query_scalar(
        "SELECT s.document FROM sources s JOIN subscription_sources x ON x.source_id=s.id WHERE x.subscription_id=$1",
    )
    .bind(legacy.id().as_uuid().to_string())
    .fetch_one(pool)
    .await
    .expect("read migrated collector");
    let source: SourceDefinition =
        serde_json::from_str(&source_document).expect("deserialize migrated collector");
    assert!(matches!(source.kind(), SourceKind::WebPage(_)));
}

async fn verify_lease_fencing(pool: &PgPool) {
    let now = Utc::now();
    let source_id = SourceId::new();
    let source = SourceDefinition::new(
        source_id,
        url::Url::parse("https://acceptance.example/feed.xml").expect("valid source URL"),
        SourceKind::XmlFeed,
    )
    .expect("valid source");
    sqlx::query("INSERT INTO sources (id, revision, document) VALUES ($1, 0, $2)")
        .bind(source_id.as_uuid().to_string())
        .bind(serde_json::to_string(&source).expect("serialize source"))
        .execute(pool)
        .await
        .expect("insert source");

    let job_id = JobId::new();
    let item = WorkItem::PollSource { source_id };
    sqlx::query(
        "INSERT INTO ingest_jobs
         (id, status, run_at_ms, first_attempt_ms, origin_key, attempt, item, revision)
         VALUES ($1, 'ready', $2, $2, $3, 0, $4, 0)",
    )
    .bind(job_id.as_uuid().to_string())
    .bind(now.timestamp_millis())
    .bind("https://acceptance.example")
    .bind(serde_json::to_string(&item).expect("serialize work item"))
    .execute(pool)
    .await
    .expect("insert ready job");

    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        1,
        ChronoDuration::days(1),
    )
    .expect("valid scheduler limits");
    let first = store
        .claim("first-worker", now, now + ChronoDuration::seconds(30))
        .await
        .expect("claim first lease")
        .expect("ready job must be claimable");
    assert_eq!(first.job_id, job_id);

    assert_eq!(
        store
            .renew(job_id, LeaseToken::new(), now + ChronoDuration::seconds(60))
            .await,
        Err(StoreError::StaleLease),
        "a foreign fencing token must never renew the lease"
    );

    sqlx::query("UPDATE ingest_jobs SET lease_deadline_ms = $1 WHERE id = $2")
        .bind((now - ChronoDuration::seconds(1)).timestamp_millis())
        .bind(job_id.as_uuid().to_string())
        .execute(pool)
        .await
        .expect("expire first lease");
    let second = store
        .claim("second-worker", now, now + ChronoDuration::seconds(30))
        .await
        .expect("reclaim expired lease")
        .expect("expired job must be reclaimable");
    assert_ne!(first.token, second.token);
    assert_eq!(
        store.complete(job_id, first.token).await,
        Err(StoreError::StaleLease),
        "the superseded worker must be fenced from completion"
    );
    store
        .complete(job_id, second.token)
        .await
        .expect("current lease holder completes the job");
}

async fn connect_when_ready(connection: &str) -> Result<PgPool, String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut last_error = "PostgreSQL did not accept a connection".to_owned();
    while Instant::now() < deadline {
        match PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .connect(connection)
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(error) => last_error = error.to_string(),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!(
        "PostgreSQL was not healthy within {STARTUP_TIMEOUT:?}: {last_error}"
    ))
}

fn require_docker() {
    let output = command("docker", &["version", "--format", "{{.Server.Version}}"]);
    assert_success(
        "connect to Docker daemon; Docker is mandatory for PostgreSQL acceptance",
        &output,
    );
}

fn command(program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to execute {program}: {error}"))
}

fn assert_success(operation: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{operation} failed (status {}): stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
