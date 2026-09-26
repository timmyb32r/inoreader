use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{header, Response, StatusCode, Uri},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::{Parser, Subcommand};
use inoreader::{format_external_request_completion, Config, LogFormat};
use reader_application::{Argon2idPolicy, AuthPolicy, AuthService, ReaderRepository, SeedSource};
use reader_core::{AccountId, ReasonPolicy, Subscription, SubscriptionId, WorkspaceId};
use reader_ingest::{
    run_until_shutdown, IngestLimits, IngestWorker, SecureWebFetcher, StaticWebFeedCollector,
};
use reader_ingest::{
    BrowserCapability, BrowserCollector, BrowserHttpClient, BuiltInAdapterCollector,
    CacheValidators, CdpBrowserCollector, FeedFetcher, FetchError, SelectorLanguage,
    SourceDefinition, SourceKind, SourceRecord, WebFeedActions, WebFeedRecipe, WebLoading,
    WebSelector, WebViewport,
};
use reader_server::FeedDiscovery;
use reader_server_contracts::{
    FeedPreviewArticle, FeedPreviewResponse, SelectorDraft,
    VisualCandidateGroup as VisualCandidateGroupView, VisualPreviewRequest, VisualPreviewResponse,
    VisualRect as VisualRectView, VisualSelectionRequest, VisualSelectionResponse,
    WebFeedRecipeDraft,
};
use reader_storage_postgres::{prepare_schema, PostgresIngestStore, PostgresRepository};
use reader_storage_ydb::{export_migration_snapshot, ProductionYdbTransport, YdbClientLimits};
use reader_web_runtime::{
    ExternalRequestCompletion, ExternalRequestObserver, OutboundHttpClient, OutboundLimits,
    OutboundPolicy, RawOutboundLimits, ReqwestPinnedTransport, TokioDnsResolver,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use url::Url;

mod migration;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = "inoreader.yaml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Serve,
    CheckConfig,
    PrepareSchema,
    PrioritizeJobs,
    BenchmarkLibrary {
        workspace_id: uuid::Uuid,
    },
    MigrateYdbToPostgres,
    BootstrapAdmin {
        username: String,
    },
    Seed {
        manifest: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    Health,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedManifest {
    schema_version: u64,
    owner_account_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    items: Vec<SeedItem>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedItem {
    source_inventory_id: String,
    idempotency_key: String,
    selected: bool,
    status: String,
    configuration: serde_json::Value,
    note: Option<String>,
}

type PreparedSeed = (String, Subscription, SeedSource, serde_json::Value);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::load(&cli.config)?;
    if matches!(cli.command, Command::CheckConfig) {
        require_database_credentials(&config)?;
        println!("configuration and credential reference are valid");
        return Ok(());
    }
    if let Command::Seed {
        manifest,
        apply: false,
    } = &cli.command
    {
        let seed = load_seed(manifest)?;
        println!("seed manifest is valid: {} selected sources for account {} workspace {}; no changes applied",seed.items.iter().filter(|v|v.selected).count(),seed.owner_account_id,seed.workspace_id);
        return Ok(());
    }
    let pool = postgres_pool(&config).await?;
    match cli.command {
        Command::CheckConfig | Command::Seed { apply: false, .. } => unreachable!(),
        Command::Health => {
            sqlx::query("SELECT 1").execute(&pool).await?;
            println!("PostgreSQL connection is healthy");
        }
        Command::PrepareSchema => {
            prepare_schema(&pool).await?;
            println!("PostgreSQL schema prepared");
        }
        Command::PrioritizeJobs => {
            let store = PostgresIngestStore::new(
                pool,
                chrono::Duration::seconds(config.scheduler.polling_interval_seconds as i64),
                config.scheduler.per_origin_concurrency,
                chrono::Duration::seconds(config.scheduler.max_retry_age_seconds as i64),
            )?;
            let count = store.reprioritize_ready_jobs().await?;
            println!("reprioritized {count} ready ingest jobs");
        }
        Command::BenchmarkLibrary { workspace_id } => {
            let repository = PostgresRepository::new(
                pool,
                ReasonPolicy::new(config.subscriptions.pause_reason_max_bytes)?,
                config.ingest.initial_feed_items,
            )?;
            let started = std::time::Instant::now();
            let workspace = WorkspaceId::from_uuid(workspace_id);
            let articles = repository.article_summaries_by_workspace(workspace).await?;
            println!(
                "loaded {} article summaries in {} ms",
                articles.len(),
                started.elapsed().as_millis()
            );
            if let Some(first) = articles.first() {
                let started = std::time::Instant::now();
                repository
                    .article_presentation(workspace, first.article.id)
                    .await?;
                println!(
                    "loaded one article presentation in {} ms",
                    started.elapsed().as_millis()
                );
            }
        }
        Command::BootstrapAdmin { username } => {
            let password = rpassword::prompt_password("Administrator password: ")?;
            let confirmation = rpassword::prompt_password("Confirm password: ")?;
            if password != confirmation {
                return Err("password confirmation does not match".into());
            }
            let repository = Arc::new(PostgresRepository::new(
                pool.clone(),
                ReasonPolicy::new(config.subscriptions.pause_reason_max_bytes)?,
                config.ingest.initial_feed_items,
            )?);
            let account = AuthService::new(repository, auth_policy(&config)?)
                .bootstrap_admin(username, &password)
                .await?;
            println!("administrator {} created", account.id.as_uuid());
        }
        Command::Seed {
            manifest,
            apply: true,
        } => {
            let seed = load_seed(&manifest)?;
            let policy = ReasonPolicy::new(config.subscriptions.pause_reason_max_bytes)?;
            let repository =
                PostgresRepository::new(pool.clone(), policy, config.ingest.initial_feed_items)?;
            repository
                .account(AccountId::from_uuid(seed.owner_account_id))
                .await?;
            let workspace = repository
                .workspace(WorkspaceId::from_uuid(seed.workspace_id))
                .await?;
            if workspace.owner() != AccountId::from_uuid(seed.owner_account_id) {
                return Err("seed manifest workspace is not owned by owner_account_id".into());
            }
            let values = seed_values(seed)?;
            let count = values.len();
            repository.apply_seed_atomic(workspace.id(), values).await?;
            println!("seed applied atomically: {count} selected sources");
        }
        Command::MigrateYdbToPostgres => {
            require_ydb_credentials(&config)?;
            let source = connect_ydb_migration_source(&config).await?;
            let snapshot = export_migration_snapshot(source.as_ref()).await?;
            migration::import_snapshot(&pool, &snapshot).await?;
            for table in &snapshot.tables {
                println!(
                    "migrated {} rows={} utf8_bytes={} fingerprint={:032x}",
                    table.table.name, table.row_count, table.utf8_bytes, table.fingerprint
                );
            }
            println!("YDB to PostgreSQL migration verified");
        }
        Command::Serve => {
            let policy = ReasonPolicy::new(config.subscriptions.pause_reason_max_bytes)?;
            serve(&config, pool, policy).await?;
        }
    }
    Ok(())
}

fn require_ydb_credentials(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let name = &config.database.migration_source_ydb.credentials_env;
    let value = std::env::var(name)
        .map_err(|_| format!("missing credentials environment variable {name}"))?;
    if value.trim().is_empty() {
        return Err(format!("credentials environment variable {name} is empty").into());
    }
    if name.ends_with("_FILE_CREDENTIALS") {
        let metadata = std::fs::metadata(&value).map_err(|error| {
            format!("credential file referenced by {name} is unavailable: {error}")
        })?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(format!(
                "credential file referenced by {name} must be a non-empty regular file"
            )
            .into());
        }
    }
    Ok(())
}

fn require_database_credentials(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let _ = postgres_password(config)?;
    Ok(())
}

fn postgres_password(config: &Config) -> Result<String, Box<dyn std::error::Error>> {
    let name = &config.database.postgres.password_file_env;
    let path = std::env::var(name)
        .map_err(|_| format!("missing PostgreSQL password-file environment variable {name}"))?;
    let raw = std::fs::read_to_string(&path).map_err(|error| {
        format!("PostgreSQL password file referenced by {name} is unavailable: {error}")
    })?;
    let password = raw
        .strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .unwrap_or(&raw);
    if password.is_empty() || password.contains(['\n', '\r', '\0']) {
        return Err("PostgreSQL password file must contain exactly one non-empty line".into());
    }
    Ok(password.to_owned())
}

async fn postgres_pool(config: &Config) -> Result<sqlx::PgPool, Box<dyn std::error::Error>> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    let password = postgres_password(config)?;
    let options = PgConnectOptions::new()
        .host(&config.database.postgres.host)
        .port(config.database.postgres.port)
        .database(&config.database.postgres.database)
        .username(&config.database.postgres.username)
        .password(&password);
    let pool = PgPoolOptions::new()
        .max_connections(config.database.postgres.max_connections)
        .acquire_timeout(Duration::from_secs(
            config.database.postgres.acquire_timeout_seconds,
        ))
        .connect_with(options)
        .await?;
    prepare_schema(&pool).await?;
    Ok(pool)
}

async fn connect_ydb_migration_source(
    config: &Config,
) -> Result<Arc<ProductionYdbTransport>, Box<dyn std::error::Error>> {
    let source = &config.database.migration_source_ydb;
    let connection = format!(
        "{}/{}",
        source.endpoint.trim_end_matches('/'),
        source.database_path.trim_start_matches('/')
    );
    let limits = YdbClientLimits::new(
        Duration::from_secs(source.request_timeout_seconds),
        source.max_concurrency,
        source.retry_attempts,
        Duration::from_millis(source.retry_initial_backoff_milliseconds),
    )?;
    Ok(Arc::new(
        ProductionYdbTransport::connect_from_environment(&connection, limits).await?,
    ))
}

fn load_seed(path: &std::path::Path) -> Result<SeedManifest, Box<dyn std::error::Error>> {
    let raw = std::fs::read(path)?;
    let value: SeedManifest = serde_json::from_slice(&raw)?;
    if value.schema_version != 1 {
        return Err("unsupported seed schema_version".into());
    }
    if value.items.is_empty() {
        return Err("seed manifest has no items".into());
    }
    let mut ids = std::collections::HashSet::new();
    let mut keys = std::collections::HashSet::new();
    for item in &value.items {
        if item.source_inventory_id.is_empty()
            || item.idempotency_key != format!("personal_feed:{}", item.source_inventory_id)
        {
            return Err("seed item has invalid identity or idempotency key".into());
        }
        if !ids.insert(&item.source_inventory_id) || !keys.insert(&item.idempotency_key) {
            return Err("seed manifest contains duplicate identities".into());
        }
        if item.selected && item.status != "reviewed" {
            return Err("selected seed item is not reviewed".into());
        }
        if contains_secret_field(&item.configuration) {
            return Err("seed configuration contains a forbidden secret-like field".into());
        }
        if !matches!(item.status.as_str(), "reviewed" | "unresolved" | "disabled") {
            return Err("seed item has invalid status".into());
        }
        let _ = &item.note;
    }
    Ok(value)
}

fn contains_secret_field(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            ["password", "secret", "token", "credential", "private_key"]
                .iter()
                .any(|part| key.to_lowercase().contains(part))
                || contains_secret_field(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_secret_field),
        _ => false,
    }
}

fn seed_values(seed: SeedManifest) -> Result<Vec<PreparedSeed>, Box<dyn std::error::Error>> {
    let mut values = Vec::new();
    for item in seed.items.into_iter().filter(|v| v.selected) {
        let config = item
            .configuration
            .as_object()
            .ok_or("seed configuration must be an object")?;
        let name = config
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("seed configuration requires name")?;
        let raw_url = config
            .get("feed_url")
            .or_else(|| config.get("url"))
            .and_then(|v| v.as_str())
            .ok_or("seed configuration requires url or feed_url")?;
        let url = Url::parse(raw_url)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("seed URL must use HTTP or HTTPS".into());
        }
        let kind = if config.get("feed_url").is_some() {
            SeedSource::Feed
        } else {
            SeedSource::Imported
        };
        let identity = format!(
            "seed/{}/{}/{}",
            seed.owner_account_id, seed.workspace_id, item.idempotency_key
        );
        let id = SubscriptionId::from_uuid(uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            identity.as_bytes(),
        ));
        values.push((
            item.idempotency_key,
            Subscription::new(
                id,
                WorkspaceId::from_uuid(seed.workspace_id),
                url,
                name.to_owned(),
            ),
            kind,
            serde_json::Value::Object(config.clone()),
        ));
    }
    Ok(values)
}

async fn serve(
    config: &Config,
    pool: sqlx::PgPool,
    reason_policy: ReasonPolicy,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "archive_budget_bytes={} policy=telemetry_only",
        config.observability.archive_budget_bytes
    );
    let repository = Arc::new(PostgresRepository::new(
        pool.clone(),
        reason_policy,
        config.ingest.initial_feed_items,
    )?);
    repository.readiness().await?;
    // The transport exposes decoded response bytes, so both the wire/body
    // budget and decompressed budget constrain the same pre-parser boundary.
    let limits = OutboundLimits::try_from(RawOutboundLimits {
        connect_timeout_ms: config.http.connect_timeout_seconds * 1000,
        request_deadline_ms: config.http.request_timeout_seconds * 1000,
        max_redirect_hops: config.http.redirect_hops as usize,
        max_response_body_bytes: config
            .http
            .max_body_bytes
            .min(config.http.max_decompressed_bytes),
    })?;
    let outbound_policy = OutboundPolicy::for_plain_http_hosts(
        config.http.allowed_plain_http_hosts.iter().cloned(),
        limits,
    );
    let observer = RequestObserver {
        format: config.observability.log_format,
    };
    let fetcher = Arc::new(
        SecureWebFetcher::new(OutboundHttpClient::new(
            outbound_policy.clone(),
            TokioDnsResolver,
            ReqwestPinnedTransport,
            observer,
        ))
        .with_user_agent(&config.http.user_agent)?,
    );
    let browser_http: Arc<dyn BrowserHttpClient> = Arc::new(OutboundHttpClient::new(
        outbound_policy,
        TokioDnsResolver,
        ReqwestPinnedTransport,
        observer,
    ));
    let cdp = CdpBrowserCollector::configured(
        config.browser.cdp_endpoint.clone(),
        browser_http.clone(),
        config.browser.max_contexts,
        Duration::from_secs(config.browser.navigation_timeout_seconds),
        config.ingest.max_web_feed_pages,
    )?;
    if let Err(error) = cdp.health_check().await {
        eprintln!("browser capability degraded: {error}")
    }
    let web_feeds = Arc::new(ProductionWebCollector {
        static_feeds: StaticWebFeedCollector::new(fetcher.clone()),
        adapters: BuiltInAdapterCollector::new(browser_http),
        cdp,
        max_pages: config.ingest.max_web_feed_pages,
        max_actions: config.browser.max_actions,
    });
    let discovery = Arc::new(ProductionDiscovery {
        fetcher: fetcher.clone(),
        web_feeds: web_feeds.clone(),
        preview_timeout: Duration::from_secs(config.browser.preview_timeout_seconds),
        initial_items: config.ingest.initial_feed_items,
        visual_snapshots: tokio::sync::Mutex::new(HashMap::new()),
    });
    let app = reader_server::router(reader_server::AppState::new(
        repository,
        discovery,
        reason_policy,
        auth_policy(config)?,
        config.server.external_origin.clone(),
        config.auth.login_attempts_per_minute,
        config.ingest.batch_items,
    ))
    .fallback(serve_ui)
    .layer(DefaultBodyLimit::max(config.server.max_request_body_bytes))
    .layer(tower_http::timeout::TimeoutLayer::with_status_code(
        StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(config.server.request_timeout_seconds),
    ));
    let ingest_store = Arc::new(PostgresIngestStore::new(
        pool,
        chrono::Duration::seconds(config.scheduler.polling_interval_seconds as i64),
        config.scheduler.per_origin_concurrency,
        chrono::Duration::seconds(config.scheduler.max_retry_age_seconds as i64),
    )?);
    let ingest_limits = IngestLimits::new(
        config.ingest.initial_feed_items,
        config.ingest.batch_items,
        config.content.chunk_bytes,
    )?
    .with_payload_limits(
        config.ingest.max_input_bytes,
        config.content.max_extracted_bytes,
    )?;
    let worker = Arc::new(
        IngestWorker::new(
            ingest_store,
            fetcher.clone(),
            fetcher,
            web_feeds,
            ingest_limits,
            chrono::Duration::seconds(config.scheduler.lease_seconds as i64),
        )?
        .with_runtime_policy(
            chrono::Duration::seconds(config.scheduler.renew_seconds as i64),
            config.scheduler.retry_attempts,
        )?,
    );
    let listener = tokio::net::TcpListener::bind(&config.server.bind).await?;
    let (server_stop_tx, server_stop_rx) = tokio::sync::oneshot::channel();
    let (worker_stop_tx, worker_stop_rx) = tokio::sync::oneshot::channel();
    let scheduler = tokio::spawn(run_until_shutdown(
        worker,
        config.scheduler.workers,
        Duration::from_secs(config.scheduler.queue_poll_interval_seconds),
        async {
            let _ = worker_stop_rx.await;
        },
    ));
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = server_stop_rx.await;
            })
            .await
    });
    tokio::signal::ctrl_c().await?;
    let _ = server_stop_tx.send(());
    let _ = worker_stop_tx.send(());
    let grace = Duration::from_secs(config.server.graceful_shutdown_seconds);
    tokio::time::timeout(grace, async {
        server
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        scheduler.await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| {
        format!(
            "graceful shutdown exceeded {} seconds",
            config.server.graceful_shutdown_seconds
        )
    })??;
    Ok(())
}

#[derive(Clone, Copy)]
struct RequestObserver {
    format: LogFormat,
}
impl ExternalRequestObserver for RequestObserver {
    fn completed(&self, value: ExternalRequestCompletion) {
        eprintln!(
            "{}",
            format_external_request_completion(self.format, &value)
        )
    }
}

type ProductionFetcher =
    SecureWebFetcher<TokioDnsResolver, ReqwestPinnedTransport, RequestObserver>;
struct ProductionWebCollector {
    static_feeds: StaticWebFeedCollector<ProductionFetcher>,
    adapters: BuiltInAdapterCollector,
    cdp: CdpBrowserCollector,
    max_pages: usize,
    max_actions: usize,
}
#[async_trait::async_trait]
impl BrowserCollector for ProductionWebCollector {
    fn capability(&self) -> BrowserCapability {
        self.cdp.capability()
    }
    async fn collect(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        if let SourceKind::WebPage(recipe) = source.kind() {
            if recipe.actions().page_count() > self.max_pages || recipe.max_pages() > self.max_pages
            {
                return Err(FetchError::Rejected("web_feed_page_limit".into()));
            }
            if recipe.actions().action_count() > self.max_actions {
                return Err(FetchError::Rejected("web_feed_action_limit".into()));
            }
        }
        match source.kind() {
            SourceKind::BuiltIn(_) => self.adapters.collect(source).await,
            SourceKind::WebPage(recipe) if recipe.loading() == WebLoading::Browser => {
                self.cdp.collect(source).await
            }
            SourceKind::WebPage(recipe) if recipe.loading() == WebLoading::Automatic => {
                match self.static_feeds.collect(source).await {
                    Ok(records) if !records.is_empty() => return Ok(records),
                    Ok(_) => {}
                    Err(FetchError::Rejected(value)) if value == "selector_requires_browser" => {}
                    Err(error) => return Err(error),
                }
                self.cdp.collect(source).await
            }
            _ => self.static_feeds.collect(source).await,
        }
    }
}
struct VisualSnapshotState {
    session_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
    width: u32,
    height: u32,
    groups: Vec<reader_ingest::VisualCandidateGroup>,
}
struct ProductionDiscovery {
    fetcher: Arc<ProductionFetcher>,
    web_feeds: Arc<ProductionWebCollector>,
    preview_timeout: Duration,
    initial_items: usize,
    visual_snapshots: tokio::sync::Mutex<HashMap<uuid::Uuid, VisualSnapshotState>>,
}
#[async_trait::async_trait]
impl FeedDiscovery for ProductionDiscovery {
    async fn discover(&self, url: Url) -> Result<FeedPreviewResponse, String> {
        let page = self
            .fetcher
            .fetch(&url, &CacheValidators::default())
            .await
            .map_err(|e| e.to_string())?;
        let (is_json, records) = match reader_collectors::parse_json(&page.body, &page.final_url) {
            Ok(v) => (true, v),
            Err(_) => (
                false,
                reader_collectors::parse_xml(&page.body, &page.final_url)
                    .map_err(|e| e.to_string())?,
            ),
        };
        let title = page.final_url.host_str().unwrap_or("Feed").to_owned();
        let available_items = records.len();
        let initial_items = available_items.min(self.initial_items);
        Ok(FeedPreviewResponse {
            title,
            kind: if is_json { "json_feed" } else { "rss" }.to_owned(),
            url: page.final_url.to_string(),
            available_items,
            initial_items,
            incomplete: available_items > initial_items,
            articles: records
                .into_iter()
                .take(initial_items)
                .map(|v| FeedPreviewArticle {
                    title: v.title,
                    published_at: v.published_at.map(|d| d.to_rfc3339()),
                })
                .collect(),
        })
    }
    async fn preview_web_feed(
        &self,
        draft: &WebFeedRecipeDraft,
    ) -> Result<FeedPreviewResponse, String> {
        let url = Url::parse(&draft.url).map_err(|_| "invalid web feed URL".to_owned())?;
        let recipe =
            recipe_from_draft(draft, self.web_feeds.max_pages, self.web_feeds.max_actions)?;
        let source = SourceDefinition::new(
            reader_core::SourceId::new(),
            url.clone(),
            SourceKind::WebPage(recipe),
        )
        .map_err(|e| e.to_string())?;
        let records = tokio::time::timeout(self.preview_timeout, self.web_feeds.collect(&source))
            .await
            .map_err(|_| "web feed preview timed out".to_owned())?
            .map_err(|e| e.to_string())?;
        let title = url.host_str().unwrap_or("Web feed").to_owned();
        let available_items = records.len();
        let initial_items = available_items.min(self.initial_items);
        Ok(FeedPreviewResponse {
            title,
            kind: "web_feed".to_owned(),
            url: url.to_string(),
            available_items,
            initial_items,
            incomplete: available_items > initial_items,
            articles: records
                .into_iter()
                .take(initial_items)
                .map(|v| FeedPreviewArticle {
                    title: v.key().title.clone(),
                    published_at: v.published_at().map(|d| d.to_rfc3339()),
                })
                .collect(),
        })
    }
    async fn visual_preview(
        &self,
        session_id: uuid::Uuid,
        request: &VisualPreviewRequest,
    ) -> Result<VisualPreviewResponse, String> {
        let url = Url::parse(&request.url).map_err(|_| "invalid visual preview URL".to_owned())?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("visual preview URL must use HTTP or HTTPS".into());
        }
        let mobile = match request.viewport.as_str() {
            "desktop" => false,
            "mobile" => true,
            _ => return Err("invalid visual preview viewport".into()),
        };
        let capture = tokio::time::timeout(
            self.preview_timeout,
            self.web_feeds.cdp.visual_snapshot(&url, mobile),
        )
        .await
        .map_err(|_| "visual preview timed out".to_owned())?
        .map_err(|e| e.to_string())?;
        let token = uuid::Uuid::new_v4();
        let expires_at = chrono::Utc::now()
            + chrono::Duration::from_std(self.preview_timeout)
                .map_err(|_| "invalid visual preview deadline".to_owned())?;
        let groups = capture
            .groups
            .iter()
            .enumerate()
            .map(|(index, group)| VisualCandidateGroupView {
                id: format!("group-{index}"),
                selector: SelectorDraft {
                    language: "css".into(),
                    expression: group.selector.clone(),
                },
                count: group.boxes.len(),
                boxes: group
                    .boxes
                    .iter()
                    .map(|rect| VisualRectView {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                    })
                    .collect(),
            })
            .collect();
        let state = VisualSnapshotState {
            session_id,
            workspace_id: request.workspace_id,
            expires_at,
            width: capture.width,
            height: capture.height,
            groups: capture.groups,
        };
        let mut snapshots = self.visual_snapshots.lock().await;
        let now = chrono::Utc::now();
        snapshots.retain(|_, value| {
            value.expires_at > now
                && (value.session_id != session_id || value.workspace_id != request.workspace_id)
        });
        snapshots.insert(token, state);
        Ok(VisualPreviewResponse {
            snapshot_token: token,
            expires_at,
            image_data_url: format!("data:image/png;base64,{}", BASE64.encode(capture.png)),
            width: capture.width,
            height: capture.height,
            groups,
        })
    }
    async fn visual_select(
        &self,
        session_id: uuid::Uuid,
        request: &VisualSelectionRequest,
    ) -> Result<VisualSelectionResponse, String> {
        if !request.x.is_finite() || !request.y.is_finite() || request.x < 0.0 || request.y < 0.0 {
            return Err("invalid visual selection coordinates".into());
        }
        let mut snapshots = self.visual_snapshots.lock().await;
        snapshots.retain(|_, value| value.expires_at > chrono::Utc::now());
        let snapshot = snapshots
            .get(&request.snapshot_token)
            .ok_or_else(|| "visual_snapshot_stale".to_owned())?;
        if snapshot.session_id != session_id || snapshot.workspace_id != request.workspace_id {
            return Err("visual_snapshot_stale".into());
        }
        if request.x > f64::from(snapshot.width) || request.y > f64::from(snapshot.height) {
            return Err("visual selection is outside the snapshot".into());
        }
        let selected = select_visual_group(snapshot, request.x, request.y)?;
        snapshots.remove(&request.snapshot_token);
        Ok(selected)
    }
}

fn select_visual_group(
    snapshot: &VisualSnapshotState,
    x: f64,
    y: f64,
) -> Result<VisualSelectionResponse, String> {
    let group = snapshot
        .groups
        .iter()
        .filter(|group| {
            group.boxes.iter().any(|rect| {
                x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
            })
        })
        .min_by(|left, right| {
            let left_area = left
                .boxes
                .iter()
                .filter(|r| x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height)
                .map(|r| r.width * r.height)
                .fold(f64::INFINITY, f64::min);
            let right_area = right
                .boxes
                .iter()
                .filter(|r| x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height)
                .map(|r| r.width * r.height)
                .fold(f64::INFINITY, f64::min);
            left_area.total_cmp(&right_area)
        })
        .ok_or_else(|| "no repeated item group at the selected coordinates".to_owned())?;
    WebSelector::new(SelectorLanguage::Css, group.selector.clone()).map_err(|e| e.to_string())?;
    Ok(VisualSelectionResponse {
        selector: SelectorDraft {
            language: "css".into(),
            expression: group.selector.clone(),
        },
        count: group.boxes.len(),
        similar_items: group
            .boxes
            .iter()
            .map(|rect| VisualRectView {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            })
            .collect(),
    })
}

fn recipe_from_draft(
    draft: &WebFeedRecipeDraft,
    max_pages: usize,
    max_actions: usize,
) -> Result<WebFeedRecipe, String> {
    let loading = match draft.loading.as_str() {
        "automatic" => WebLoading::Automatic,
        "static" => WebLoading::Static,
        "browser" => WebLoading::Browser,
        _ => return Err("invalid web feed loading mode".into()),
    };
    let language = match draft.selector_language.as_deref().unwrap_or("css") {
        "css" => SelectorLanguage::Css,
        "xpath" => SelectorLanguage::XPath,
        _ => return Err("invalid selector language".into()),
    };
    let primary = WebSelector::new(language, draft.selector.clone()).map_err(|e| e.to_string())?;
    let viewport = match draft.viewport.as_deref().unwrap_or("desktop") {
        "desktop" => WebViewport::Desktop,
        "mobile" => WebViewport::Mobile,
        _ => return Err("invalid viewport".into()),
    };
    let parse = |value: &reader_server_contracts::SelectorDraft| {
        let language = match value.language.as_str() {
            "css" => SelectorLanguage::Css,
            "xpath" => SelectorLanguage::XPath,
            _ => return Err("invalid selector language".to_owned()),
        };
        WebSelector::new(language, value.expression.clone()).map_err(|e| e.to_string())
    };
    let overlays = draft
        .hide_overlays
        .iter()
        .map(&parse)
        .collect::<Result<Vec<_>, _>>()?;
    let pages = draft
        .start_pages
        .iter()
        .map(|value| Url::parse(value).map_err(|_| "invalid start page URL".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let next = draft.next_page.as_ref().map(&parse).transpose()?;
    let load_more = draft.load_more.as_ref().map(&parse).transpose()?;
    let requested_pages = draft.max_pages.unwrap_or(1);
    if requested_pages == 0 || requested_pages > max_pages {
        return Err("web feed maxPages exceeds configured limit".into());
    }
    let actions = WebFeedActions::new(
        viewport,
        overlays,
        pages,
        next,
        load_more,
        draft.load_more_clicks,
        draft.scrolls,
        max_pages,
        max_actions,
    )
    .map_err(|e| e.to_string())?;
    let listing = draft
        .listing_url
        .as_deref()
        .map(Url::parse)
        .transpose()
        .map_err(|_| "invalid listing URL".to_owned())?;
    let extraction = reader_ingest::WebExtraction::new(
        listing,
        draft.card_selector.clone(),
        draft.title_selector.clone(),
        draft.date_selector.clone(),
        draft.content_selector.clone(),
        draft.wait_selector.clone(),
        draft.url_pattern.clone(),
    )
    .map_err(|e| e.to_string())?;
    WebFeedRecipe::legacy(primary, loading, actions, extraction, requested_pages)
        .map_err(|e| e.to_string())
}

fn auth_policy(config: &Config) -> Result<AuthPolicy, reader_application::AuthError> {
    Ok(AuthPolicy {
        session_lifetime_seconds: config.auth.session_lifetime_seconds,
        invite_lifetime_seconds: config.auth.invite_lifetime_seconds,
        reset_lifetime_seconds: config.auth.reset_lifetime_seconds,
        argon2id: Argon2idPolicy::new(
            config.auth.argon2id_memory_kib,
            config.auth.argon2id_time_cost,
            config.auth.argon2id_parallelism,
        )?,
    })
}

async fn serve_ui(uri: Uri) -> Response<Body> {
    match ui_asset(uri.path()) {
        Some(asset) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, asset.content_type)
            .header(header::CACHE_CONTROL, reader_server_ui::cache_control(asset))
            .header("content-security-policy", "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'self'; connect-src 'self'; img-src 'self' https: data:; style-src 'self'; script-src 'self'")
            .header("x-content-type-options", "nosniff")
            .header("referrer-policy", "strict-origin-when-cross-origin")
            .body(Body::from(asset.bytes))
            .expect("static response headers are valid"),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"code":"not_found","message":"resource not found"}"#))
            .expect("static not-found response is valid"),
    }
}

fn ui_asset(path: &str) -> Option<reader_server_ui::Asset> {
    reader_server_ui::asset(path).or_else(|| {
        (!path.starts_with("/api/") && !path.rsplit('/').next().unwrap_or_default().contains('.'))
            .then(|| reader_server_ui::asset("/index.html"))
            .flatten()
    })
}

#[cfg(test)]
mod main_tests;
