mod startup;
use startup::*;
mod http;
use http::*;
mod discovery;
use discovery::*;
mod seed;
use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{header, Response, StatusCode, Uri},
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::{Parser, Subcommand};
use inoreader::{format_external_request_completion, Config, LogFormat};
use reader_application::{Argon2idPolicy, AuthPolicy, AuthService, SeedSource};
use reader_application::{
    ArticleRepository, IdentityRepository, OperationsRepository, SubscriptionRepository,
    WorkspaceRepository,
};
use reader_core::{AccountId, ReasonPolicy, Subscription, SubscriptionId, WorkspaceId};
use reader_ingest::{
    run_until_shutdown, IngestLimits, IngestWorker, SecureWebFetcher, StaticWebFeedCollector,
};
use reader_ingest::{
    BrowserCapability, BrowserCollector, BrowserHttpClient, BuiltInAdapterCollector,
    CacheValidators, CdpBrowserCollector, FeedFetcher, FetchError, SelectorLanguage,
    SourceDefinition, SourceKind, SourceRecord, WebLoading, WebSelector,
};
use reader_server::FeedDiscovery;
use reader_server_contracts::{
    FeedPreviewArticle, FeedPreviewResponse, SelectorDraft,
    VisualCandidateGroup as VisualCandidateGroupView, VisualPreviewRequest, VisualPreviewResponse,
    VisualRect as VisualRectView, VisualSelectionRequest, VisualSelectionResponse,
    WebFeedRecipeDraft,
};
use reader_storage_postgres::{prepare_schema, PostgresIngestStore, PostgresRepository};
use reader_web_runtime::{
    ExternalRequestCompletion, ExternalRequestObserver, OutboundHttpClient, OutboundLimits,
    OutboundPolicy, PublicFetchTransport, RawOutboundLimits, ReqwestPinnedTransport,
    TokioDnsResolver,
};
use seed::*;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use url::Url;

mod ai;
mod glossary;
mod icons;
mod zhihu;

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
    UpgradeSchema,
    ReindexTelegramGlossary {
        owner: uuid::Uuid,
        workspace: uuid::Uuid,
    },
    ImportTelegramArchive {
        archive: PathBuf,
        owner: uuid::Uuid,
        workspace: uuid::Uuid,
    },
    PrioritizeJobs,
    /// Audit every source; optionally enrich missing dates from retained HTML.
    BackfillPublicationDates {
        #[arg(long)]
        apply: bool,
    },
    BenchmarkLibrary {
        workspace_id: uuid::Uuid,
    },
    MigratePersonalFeedLinks {
        inventory: PathBuf,
        workspace_id: uuid::Uuid,
    },
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = Config::load(&cli.config)?;
    if let Ok(value) = std::env::var("CONTAINER_STOP_GRACE_SECONDS") {
        config.validate_container_stop_budget(
            value
                .parse()
                .map_err(|_| "invalid CONTAINER_STOP_GRACE_SECONDS")?,
        )?;
    }
    init_logging(config.observability.log_format);
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
    if !matches!(cli.command, Command::PrepareSchema | Command::UpgradeSchema) {
        reader_storage_postgres::verify_schema(&pool).await?;
    }
    match cli.command {
        Command::BackfillPublicationDates { apply } => {
            let report = reader_storage_postgres::backfill_publication_dates(
                &pool,
                std::num::NonZeroU32::new(u32::try_from(config.ingest.batch_items)?)
                    .ok_or("batch_items must be positive")?,
                config.ingest.max_input_bytes,
                apply,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::CheckConfig | Command::Seed { apply: false, .. } => unreachable!(),
        Command::ReindexTelegramGlossary { owner, workspace } => {
            let policy = reader_glossary::GlossaryPolicy::new(
                config
                    .glossary
                    .clone()
                    .ok_or("glossary configuration required")?,
            )?;
            let count = reader_storage_postgres::PostgresGlossaryStore::new(pool)
                .reindex(owner, workspace, policy.config().batch_size)
                .await?;
            println!("Reindexed {count} retained current posts; failed non-conflicting events queued for replay.");
        }
        Command::ImportTelegramArchive {
            archive,
            owner,
            workspace,
        } => {
            glossary::import(&config, pool, archive, owner, workspace).await?;
        }
        Command::Health => {
            sqlx::query("SELECT 1").execute(&pool).await?;
            println!("PostgreSQL connection is healthy");
        }
        Command::UpgradeSchema => {
            reader_storage_postgres::upgrade_schema(
                &pool,
                std::num::NonZeroU32::new(u32::try_from(config.ingest.batch_items)?)
                    .ok_or("upgrade batch must be positive")?,
            )
            .await?;
            println!("PostgreSQL schema upgraded and verified");
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
                config.subscriptions.attention_after_seconds,
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
                config.subscriptions.attention_after_seconds,
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
            let repository = PostgresRepository::new(
                pool.clone(),
                policy,
                config.ingest.initial_feed_items,
                config.subscriptions.attention_after_seconds,
            )?;
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
        Command::MigratePersonalFeedLinks {
            inventory,
            workspace_id,
        } => {
            let raw = std::fs::read(inventory)?;
            let inventory: SourceInventory = serde_json::from_slice(&raw)?;
            if inventory.schema_version != 1
                || inventory.sources.len() != inventory.expected_source_count
                || inventory.expected_source_count != 42
            {
                return Err("personal_feed inventory is incomplete or unsupported".into());
            }
            let _ = (&inventory.origin, &inventory.pdf_inventory);
            let mut ids = std::collections::HashSet::new();
            let values = inventory
                .sources
                .into_iter()
                .map(|source| {
                    if source.id.is_empty() || !ids.insert(source.id.clone()) {
                        return Err("personal_feed inventory has invalid identities");
                    }
                    let _ = (
                        source.adapter_kind,
                        source.historical_observations,
                        source.fixture_coverage,
                    );
                    Ok((source.id, source.configuration))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let policy = ReasonPolicy::new(config.subscriptions.pause_reason_max_bytes)?;
            let repository = PostgresRepository::new(
                pool,
                policy,
                config.ingest.initial_feed_items,
                config.subscriptions.attention_after_seconds,
            )?;
            repository
                .workspace(WorkspaceId::from_uuid(workspace_id))
                .await?;
            let migrated = repository
                .migrate_personal_feed_links_atomic(WorkspaceId::from_uuid(workspace_id), values)
                .await?;
            println!("migrated {migrated} legacy personal_feed subscriptions atomically");
        }
        Command::Serve => {
            let policy = ReasonPolicy::new(config.subscriptions.pause_reason_max_bytes)?;
            serve(&config, pool, policy).await?;
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
        .acquire_time_level(log::LevelFilter::Info)
        .max_connections(config.database.postgres.max_connections)
        .acquire_timeout(Duration::from_secs(
            config.database.postgres.acquire_timeout_seconds,
        ))
        .connect_with(reader_storage_postgres::instrument_postgres(options))
        .await?;
    Ok(pool)
}

fn init_logging(format: LogFormat) {
    use std::io::Write;

    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    builder.format_timestamp_millis();
    if format == LogFormat::Json {
        builder.format(|buffer, record| {
            writeln!(
                buffer,
                "{}",
                serde_json::json!({
                    "event": "log",
                    "context": reader_runtime::Context::current(),
                    "level": record.level().to_string(),
                    "target": record.target(),
                    "message": record.args().to_string(),
                })
            )
        });
    }
    if format != LogFormat::Json {
        builder.format(|buffer, record| {
            let context = reader_runtime::Context::current();
            writeln!(
                buffer,
                "{} {} {} context={:?} {}",
                buffer.timestamp_millis(),
                record.level(),
                record.target(),
                context,
                record.args()
            )
        });
    }
    let _ = builder.try_init();
}

#[cfg(test)]
mod main_tests;

#[cfg(test)]
#[path = "tests/cli.rs"]
mod cli_tests;
