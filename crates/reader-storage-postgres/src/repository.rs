use chrono::{DateTime, Utc};
use reader_application::{
    AccountRecord, ArticlePage, ArticlePageDirection, ArticlePageRequest, ArticlePresentation,
    InviteRecord, PasswordResetRecord, RepositoryError, RuleApplicationProgress, SeedSource,
    SessionRecord, SourceUrlPreviewRecord, SubscriptionActivity, SubscriptionStats,
};
use reader_application::{ArticleRepository, WorkspaceRepository};
use reader_core::{
    AccountId, Article, ArticleId, DurableJob, ReasonPolicy, Rule, RuleId, SourceId, Subscription,
    SubscriptionId, SubscriptionStatus, Workspace, WorkspaceId,
};
use reader_ingest::{
    BuiltInAdapter, JobId, SelectorLanguage, SourceDefinition, SourceKind, SourceRecord,
    WebExtraction, WebFeedActions, WebFeedRecipe, WebLoading, WebSelector, WebViewport, WorkItem,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::{postgres::PgConnectOptions, PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

type PresentationRow = (
    bool,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);
type PresentationOrigin = (
    bool,
    String,
    Subscription,
    Option<reader_ingest::ContentManifestPointer>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(Clone)]
pub struct PostgresRepository {
    pool: PgPool,
    reason_policy: ReasonPolicy,
    initial_scope: usize,
    attention_after_seconds: u64,
}
impl PostgresRepository {
    pub async fn connect(
        options: PgConnectOptions,
        reason_policy: ReasonPolicy,
        initial_scope: usize,
        attention_after_seconds: u64,
    ) -> Result<Self, RepositoryError> {
        validate_repository_limits(initial_scope, attention_after_seconds)?;
        let pool = PgPool::connect_with(crate::instrument_postgres(options))
            .await
            .map_err(storage)?;
        crate::verify_schema(&pool).await.map_err(storage)?;
        Self::new(pool, reason_policy, initial_scope, attention_after_seconds)
    }
    pub fn new(
        pool: PgPool,
        reason_policy: ReasonPolicy,
        initial_scope: usize,
        attention_after_seconds: u64,
    ) -> Result<Self, RepositoryError> {
        validate_repository_limits(initial_scope, attention_after_seconds)?;
        Ok(Self {
            pool,
            reason_policy,
            initial_scope,
            attention_after_seconds,
        })
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
    pub const fn backend_name() -> &'static str {
        "postgres"
    }
    async fn read<T: DeserializeOwned>(
        &self,
        kind: &str,
        id: String,
    ) -> Result<T, RepositoryError> {
        let query = format!("SELECT document FROM {} WHERE id = $1", table(kind)?);
        let document: String = sqlx::query_scalar(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
        serde_json::from_str(&document).map_err(storage)
    }
    async fn list<T: DeserializeOwned>(
        &self,
        kind: &str,
        scope: String,
    ) -> Result<Vec<T>, RepositoryError> {
        let table = table(kind)?;
        let query = match kind {
            "workspaces" => format!("SELECT document FROM {table} WHERE document::jsonb ->> 'owner' = $1 ORDER BY id"),
            "subscriptions" => format!("SELECT document FROM {table} WHERE document::jsonb ->> 'workspace_id' = $1 ORDER BY id"),
            "articles" | "rules" => format!("SELECT document FROM {table} WHERE id LIKE $1 || '/%' ORDER BY id"),
            _ => return Err(RepositoryError::Storage(format!("unsupported scoped list {kind}"))),
        };
        sqlx::query_scalar::<_, String>(&query)
            .bind(scope)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .into_iter()
            .map(|v| serde_json::from_str(&v).map_err(storage))
            .collect()
    }
    async fn cas<T: Serialize>(
        &self,
        kind: &str,
        id: String,
        _scope: String,
        expected: Option<u64>,
        revision: u64,
        value: &T,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let affected = cas_tx(&mut tx, kind, id, expected, revision, value).await?;
        if affected == 1 {
            tx.commit().await.map_err(storage)
        } else {
            Err(RepositoryError::Conflict)
        }
    }
}

fn validate_repository_limits(
    initial_scope: usize,
    attention_after_seconds: u64,
) -> Result<(), RepositoryError> {
    if attention_after_seconds == 0 || attention_after_seconds > i64::MAX as u64 / 1000 {
        return Err(storage(
            "attention duration must be positive and fit milliseconds",
        ));
    }
    if initial_scope == 0 {
        return Err(RepositoryError::Storage(
            "initial feed scope must be positive".into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct SourceHealth {
    last_success_ms: Option<i64>,
    incomplete: bool,
    error: Option<String>,
    #[serde(default)]
    last_error_ms: Option<i64>,
    #[serde(default)]
    consecutive_failures: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedSeedConfig {
    id: String,
    name: String,
    url: String,
    #[serde(default)]
    feed_url: Option<String>,
    #[serde(default)]
    adapter: Option<String>,
    #[serde(default)]
    listing_url: Option<String>,
    #[serde(default)]
    link_selector: Option<String>,
    #[serde(default)]
    card_selector: Option<String>,
    #[serde(default)]
    title_selector: Option<String>,
    #[serde(default)]
    date_selector: Option<String>,
    #[serde(default)]
    content_selector: Option<String>,
    #[serde(default)]
    url_pattern: Option<String>,
    #[serde(default)]
    next_selector: Option<String>,
    #[serde(default)]
    extra_listing_urls: Vec<String>,
    #[serde(default)]
    max_pages: Option<usize>,
    #[serde(default)]
    browser: bool,
    #[serde(default)]
    browser_fallback: bool,
    #[serde(default)]
    wait_selector: Option<String>,
    #[serde(default)]
    load_more_selector: Option<String>,
    #[serde(default)]
    load_more_clicks: usize,
    #[serde(default)]
    scroll_steps: usize,
}

fn imported_source_kind(configuration: &serde_json::Value) -> Result<SourceKind, RepositoryError> {
    let raw: ImportedSeedConfig = serde_json::from_value(configuration.clone())
        .map_err(|e| storage(format!("invalid imported source recipe: {e}")))?;
    if raw.id.trim().is_empty() || raw.name.trim().is_empty() {
        return Err(storage("imported source id/name is empty"));
    }
    let canonical =
        url::Url::parse(&raw.url).map_err(|_| storage("invalid imported source URL"))?;
    if !matches!(canonical.scheme(), "http" | "https") {
        return Err(storage("invalid imported source URL scheme"));
    }
    if raw.feed_url.is_some() {
        return Ok(SourceKind::Auto);
    }
    let parse_url = |value: Option<String>, fallback: &url::Url| match value {
        Some(value) => url::Url::parse(&value).map_err(|_| storage("invalid imported listing URL")),
        None => Ok(fallback.clone()),
    };
    if let Some(adapter) = raw.adapter.as_deref() {
        let adapter = match adapter {
            "cloudera" => BuiltInAdapter::Cloudera {
                listing_url: parse_url(raw.listing_url, &canonical)?,
            },
            "digoal" => BuiltInAdapter::Digoal {
                listing_url: parse_url(raw.listing_url, &canonical)?,
            },
            "mirrorship" => BuiltInAdapter::Mirrorship,
            "pingkai" => BuiltInAdapter::Pingkai {
                listing_url: parse_url(raw.listing_url, &canonical)?,
            },
            "modb-news" => BuiltInAdapter::ModbNews,
            "infoq-bigdata" => BuiltInAdapter::InfoqBigdata,
            "highgo" => BuiltInAdapter::Highgo {
                max_pages: raw.max_pages.unwrap_or(5),
            },
            _ => return Err(storage(format!("unsupported imported adapter {adapter}"))),
        };
        return Ok(SourceKind::BuiltIn(adapter));
    }
    let primary = WebSelector::new(
        SelectorLanguage::Css,
        raw.link_selector
            .ok_or_else(|| storage("generic imported source requires link_selector"))?,
    )
    .map_err(storage)?;
    let loading = if raw.browser {
        WebLoading::Browser
    } else if raw.browser_fallback {
        WebLoading::Automatic
    } else {
        WebLoading::Static
    };
    let pages = raw
        .extra_listing_urls
        .into_iter()
        .map(|v| url::Url::parse(&v).map_err(|_| storage("invalid extra listing URL")))
        .collect::<Result<Vec<_>, _>>()?;
    let next = raw
        .next_selector
        .map(|v| WebSelector::new(SelectorLanguage::Css, v).map_err(storage))
        .transpose()?;
    let load_more = raw
        .load_more_selector
        .map(|v| WebSelector::new(SelectorLanguage::Css, v).map_err(storage))
        .transpose()?;
    let actions = WebFeedActions::new(
        WebViewport::Desktop,
        vec![],
        pages,
        next,
        load_more,
        raw.load_more_clicks,
        raw.scroll_steps,
        usize::MAX,
        usize::MAX,
    )
    .map_err(storage)?;
    let listing = raw
        .listing_url
        .map(|v| url::Url::parse(&v).map_err(|_| storage("invalid imported listing URL")))
        .transpose()?;
    let extraction = WebExtraction::new(
        listing,
        raw.card_selector,
        raw.title_selector,
        raw.date_selector,
        raw.content_selector,
        raw.wait_selector,
        raw.url_pattern,
    )
    .map_err(storage)?;
    Ok(SourceKind::WebPage(
        WebFeedRecipe::configured(
            primary,
            loading,
            actions,
            extraction,
            raw.max_pages.unwrap_or(3),
        )
        .map_err(storage)?,
    ))
}

fn editable_recipe_document(
    workspace: WorkspaceId,
    url: &url::Url,
    kind: &SourceKind,
) -> Result<Option<String>, RepositoryError> {
    let SourceKind::WebPage(recipe) = kind else {
        return Ok(None);
    };
    let language = |v: &WebSelector| match v.language() {
        SelectorLanguage::Css => "css",
        SelectorLanguage::XPath => "xpath",
    };
    serde_json::to_string(&serde_json::json!({"workspaceId":workspace.as_uuid(),"url":url.as_str(),"selector":recipe.selector(),"loading":match recipe.loading(){WebLoading::Automatic=>"automatic",WebLoading::Static=>"static",WebLoading::Browser=>"browser"},"preview":false,"selectorLanguage":match recipe.selector_kind(){SelectorLanguage::Css=>"css",SelectorLanguage::XPath=>"xpath"},"viewport":match recipe.actions().viewport(){WebViewport::Desktop=>"desktop",WebViewport::Mobile=>"mobile"},"listingUrl":recipe.extraction().listing_url().map(url::Url::as_str),"cardSelector":recipe.extraction().card_selector().map(WebSelector::expression),"titleSelector":recipe.extraction().title_selector().map(WebSelector::expression),"dateSelector":recipe.extraction().date_selector().map(WebSelector::expression),"contentSelector":recipe.extraction().content_selector().map(WebSelector::expression),"waitSelector":recipe.extraction().wait_selector().map(WebSelector::expression),"urlPattern":recipe.extraction().url_pattern(),"maxPages":recipe.max_pages(),"hideOverlays":recipe.actions().hide_overlays().iter().map(|v|serde_json::json!({"language":language(v),"expression":v.expression()})).collect::<Vec<_>>(),"startPages":recipe.actions().start_pages().iter().map(url::Url::as_str).collect::<Vec<_>>(),"nextPage":recipe.actions().next_page().map(|v|serde_json::json!({"language":language(v),"expression":v.expression()})),"loadMore":recipe.actions().load_more().map(|v|serde_json::json!({"language":language(v),"expression":v.expression()})),"loadMoreClicks":recipe.actions().load_more_clicks(),"scrolls":recipe.actions().scrolls()})).map(Some).map_err(storage)
}

fn article_key(workspace: WorkspaceId, article: ArticleId) -> String {
    format!("{}/{}", workspace.as_uuid(), article.as_uuid())
}

fn rule_key(workspace: WorkspaceId, rule: RuleId) -> String {
    format!("{}/{}", workspace.as_uuid(), rule.as_uuid())
}

async fn source_for_subscription(
    tx: &mut Transaction<'_, Postgres>,
    subscription: SubscriptionId,
) -> Result<SourceId, RepositoryError> {
    let value: String =
        sqlx::query_scalar("SELECT source_id FROM subscription_sources WHERE subscription_id = $1")
            .bind(subscription.as_uuid().to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
    Ok(SourceId::from_uuid(
        Uuid::parse_str(&value).map_err(storage)?,
    ))
}

pub(crate) async fn enqueue_work_tx(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    item: &WorkItem,
) -> Result<(), RepositoryError> {
    let document = serde_json::to_string(item).map_err(storage)?;
    if let Some(existing) =
        sqlx::query_scalar::<_, String>("SELECT item FROM ingest_jobs WHERE id = $1 FOR UPDATE")
            .bind(id.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?
    {
        return if existing == document {
            Ok(())
        } else {
            Err(storage("durable job identity collision"))
        };
    }
    let origin = match item {
        WorkItem::PollSource { source_id }
        | WorkItem::RefreshSource { source_id }
        | WorkItem::CollectWebFeed { source_id }
        | WorkItem::FanOut { source_id, .. } => {
            let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id = $1")
                .bind(source_id.as_uuid().to_string())
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage)?
                .ok_or(RepositoryError::NotFound)?;
            let source: SourceDefinition = serde_json::from_str(&document).map_err(storage)?;
            source.url().origin().ascii_serialization()
        }
        WorkItem::ExtractFullText { url, .. } => url.origin().ascii_serialization(),
        WorkItem::CleanupContent { record_id, .. } => {
            let document: String =
                sqlx::query_scalar("SELECT document FROM source_records WHERE id = $1")
                    .bind(record_id.as_uuid().to_string())
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(storage)?
                    .ok_or(RepositoryError::NotFound)?;
            let record: SourceRecord = serde_json::from_str(&document).map_err(storage)?;
            let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id = $1")
                .bind(record.source_id().as_uuid().to_string())
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage)?
                .ok_or(RepositoryError::NotFound)?;
            let source: SourceDefinition = serde_json::from_str(&document).map_err(storage)?;
            source.url().origin().ascii_serialization()
        }
        WorkItem::EvaluateArticleRules { workspace_id, .. }
        | WorkItem::ApplyRule { workspace_id, .. } => {
            format!("internal:workspace:{}", workspace_id.as_uuid())
        }
    };
    let now = Utc::now().timestamp_millis();
    sqlx::query("INSERT INTO ingest_jobs (id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision) VALUES ($1,'ready',$2,$3,$4,0,$5,0)")
        .bind(id.to_string()).bind(crate::ingest_store::initial_job_run_at(item)).bind(now).bind(origin).bind(document).execute(&mut **tx).await.map_err(storage)?;
    Ok(())
}

async fn enqueue_backfill_tx(
    tx: &mut Transaction<'_, Postgres>,
    source: SourceId,
    subscription: SubscriptionId,
    limit: usize,
) -> Result<(), RepositoryError> {
    let documents = sqlx::query_scalar::<_, String>("SELECT document FROM source_record_identity WHERE source_id = $1 ORDER BY observed_at_ms DESC LIMIT $2")
        .bind(source.as_uuid().to_string()).bind(i64::try_from(limit).map_err(storage)?).fetch_all(&mut **tx).await.map_err(storage)?;
    for document in documents {
        let record: SourceRecord = serde_json::from_str(&document).map_err(storage)?;
        let item = WorkItem::FanOut {
            source_id: source,
            record_id: record.id(),
            after_subscription: None,
        };
        let identity = format!(
            "subscription-backfill/{}/{}",
            subscription.as_uuid(),
            record.id().as_uuid()
        );
        enqueue_work_tx(
            tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &item,
        )
        .await?;
    }
    Ok(())
}

fn set_work_source(item: &mut WorkItem, source: SourceId) -> Result<(), RepositoryError> {
    match item {
        WorkItem::PollSource { source_id }
        | WorkItem::RefreshSource { source_id }
        | WorkItem::CollectWebFeed { source_id } => {
            *source_id = source;
            Ok(())
        }
        _ => Err(storage("invalid subscription work item")),
    }
}

#[allow(clippy::too_many_arguments)]
async fn provision_subscription_tx(
    tx: &mut Transaction<'_, Postgres>,
    value: &Subscription,
    proposed: &SourceDefinition,
    mut item: WorkItem,
    job: Uuid,
    initial_scope: usize,
    reserve_workspace_url: bool,
    isolated_source: bool,
    recipe: Option<&str>,
) -> Result<(), RepositoryError> {
    if reserve_workspace_url {
        let key = workspace_feed_url_key(value.workspace_id(), value.source_url_exact());
        let result = sqlx::query("INSERT INTO workspace_feed_urls (id,subscription_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
            .bind(key).bind(value.id().as_uuid().to_string()).execute(&mut **tx).await.map_err(storage)?;
        if result.rows_affected() != 1 {
            return Err(RepositoryError::Conflict);
        }
    }
    let source = if isolated_source {
        cas_tx(
            tx,
            "sources",
            proposed.id().as_uuid().to_string(),
            None,
            proposed.revision(),
            proposed,
        )
        .await?;
        proposed.id()
    } else {
        let inserted = sqlx::query(
            "INSERT INTO source_urls (url,source_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
        )
        .bind(value.source_url().as_str())
        .bind(proposed.id().as_uuid().to_string())
        .execute(&mut **tx)
        .await
        .map_err(storage)?
        .rows_affected()
            == 1;
        if inserted {
            if cas_tx(
                tx,
                "sources",
                proposed.id().as_uuid().to_string(),
                None,
                proposed.revision(),
                proposed,
            )
            .await?
                != 1
            {
                return Err(RepositoryError::Conflict);
            }
            proposed.id()
        } else {
            let id: String = sqlx::query_scalar("SELECT source_id FROM source_urls WHERE url = $1")
                .bind(value.source_url().as_str())
                .fetch_one(&mut **tx)
                .await
                .map_err(storage)?;
            SourceId::from_uuid(Uuid::parse_str(&id).map_err(storage)?)
        }
    };
    if cas_tx(
        tx,
        "subscriptions",
        value.id().as_uuid().to_string(),
        None,
        value.revision(),
        value,
    )
    .await?
        != 1
    {
        return Err(RepositoryError::Conflict);
    }
    sqlx::query("INSERT INTO subscription_sources (subscription_id,source_id) VALUES ($1,$2)")
        .bind(value.id().as_uuid().to_string())
        .bind(source.as_uuid().to_string())
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    if let Some(recipe) = recipe {
        sqlx::query("INSERT INTO web_feed_recipes (id,revision,document) VALUES ($1,0,$2)")
            .bind(value.id().as_uuid().to_string())
            .bind(recipe)
            .execute(&mut **tx)
            .await
            .map_err(storage)?;
    }
    set_work_source(&mut item, source)?;
    enqueue_work_tx(tx, job, &item).await?;
    enqueue_backfill_tx(tx, source, value.id(), initial_scope).await
}
fn storage(e: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Storage(e.to_string())
}

pub(crate) fn workspace_feed_url_key(workspace: WorkspaceId, source_url_exact: &str) -> String {
    // Workspace UUIDs have a fixed textual width, so this delimiter is
    // unambiguous while remaining valid PostgreSQL UTF-8 text.
    format!("{}/{}", workspace.as_uuid(), source_url_exact)
}

fn table(kind: &str) -> Result<&'static str, RepositoryError> {
    match kind {
        "schema_metadata" => Ok("schema_metadata"),
        "workspaces" => Ok("workspaces"),
        "subscriptions" => Ok("subscriptions"),
        "articles" => Ok("articles"),
        "jobs" => Ok("jobs"),
        "accounts" => Ok("accounts"),
        "invites" => Ok("invites"),
        "sessions" => Ok("sessions"),
        "password_resets" => Ok("password_resets"),
        "login_attempts" => Ok("login_attempts"),
        "rules" => Ok("rules"),
        "sources" => Ok("sources"),
        "web_feed_recipes" => Ok("web_feed_recipes"),
        "seed_items" => Ok("seed_items"),
        "source_url_previews" => Ok("source_url_previews"),
        "source_records" => Ok("source_records"),
        _ => Err(RepositoryError::Storage(format!(
            "unsupported table {kind}"
        ))),
    }
}

impl PostgresRepository {
    /// Replaces legacy `personal_feed` proxy URLs with their original validated
    /// recipes in one transaction. Subscription identity and all user-owned
    /// state remain unchanged; the obsolete source rows are retained so prior
    /// article provenance stays queryable.
    pub async fn migrate_personal_feed_links_atomic(
        &self,
        workspace: WorkspaceId,
        configurations: Vec<(String, serde_json::Value)>,
    ) -> Result<usize, RepositoryError> {
        let configurations = configurations.into_iter().collect::<HashMap<_, _>>();
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let documents: Vec<String> = sqlx::query_scalar(
            "SELECT document FROM subscriptions WHERE document::jsonb ->> 'workspace_id'=$1 FOR UPDATE",
        )
        .bind(workspace.as_uuid().to_string())
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut migrated = 0usize;
        for document in documents {
            let mut subscription: Subscription =
                serde_json::from_str(&document).map_err(storage)?;
            let legacy = subscription.source_url();
            if legacy.host_str() != Some("china-radio-international.duckdns.org") {
                continue;
            }
            let Some(source_id) = legacy
                .path()
                .strip_prefix('/')
                .and_then(|value| value.strip_suffix(".xml"))
                .map(str::to_owned)
            else {
                return Err(storage("legacy personal_feed URL has an unsupported path"));
            };
            let configuration = configurations
                .get(&source_id)
                .ok_or_else(|| storage(format!("missing personal_feed recipe for {source_id}")))?;
            let raw: ImportedSeedConfig = serde_json::from_value(configuration.clone())
                .map_err(|e| storage(format!("invalid imported source recipe: {e}")))?;
            if raw.id != source_id {
                return Err(storage("personal_feed URL and recipe identity differ"));
            }
            let exact_url = raw.feed_url.as_deref().unwrap_or(&raw.url).to_owned();
            let canonical = url::Url::parse(&exact_url)
                .map_err(|_| storage("invalid personal_feed canonical URL"))?;
            let kind = imported_source_kind(configuration)?;
            let proposed_id = SourceId::from_uuid(Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                canonical.as_str().as_bytes(),
            ));
            let proposed = SourceDefinition::new(proposed_id, canonical.clone(), kind.clone())
                .map_err(storage)?;
            let actual_source = if let Some(existing) =
                sqlx::query_scalar::<_, String>("SELECT source_id FROM source_urls WHERE url=$1")
                    .bind(canonical.as_str())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
            {
                let existing_id = SourceId::from_uuid(Uuid::parse_str(&existing).map_err(storage)?);
                let (revision, existing_document): (i64, String) =
                    sqlx::query_as("SELECT revision,document FROM sources WHERE id=$1 FOR UPDATE")
                        .bind(&existing)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(storage)?;
                let existing_source: SourceDefinition =
                    serde_json::from_str(&existing_document).map_err(storage)?;
                if existing_source.kind() != &kind {
                    if !matches!(existing_source.kind(), SourceKind::Auto)
                        || existing_source.url() != &canonical
                    {
                        return Err(storage(format!(
                            "canonical source {} already exists with a different collector",
                            canonical
                        )));
                    }
                    let upgraded =
                        SourceDefinition::new(existing_id, canonical.clone(), kind.clone())
                            .map_err(storage)?;
                    sqlx::query(
                        "UPDATE sources SET revision=$1,document=$2 WHERE id=$3 AND revision=$4",
                    )
                    .bind(revision + 1)
                    .bind(serde_json::to_string(&upgraded).map_err(storage)?)
                    .bind(&existing)
                    .bind(revision)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                }
                existing_id
            } else {
                let proposed_key = proposed_id.as_uuid().to_string();
                if let Some(document) = sqlx::query_scalar::<_, String>(
                    "SELECT document FROM sources WHERE id=$1 FOR UPDATE",
                )
                .bind(&proposed_key)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                {
                    let existing: SourceDefinition =
                        serde_json::from_str(&document).map_err(storage)?;
                    if existing != proposed {
                        if !matches!(existing.kind(), SourceKind::Auto)
                            || existing.url() != &canonical
                        {
                            return Err(storage("personal_feed source identity collision"));
                        }
                        sqlx::query(
                            "UPDATE sources SET revision=revision+1,document=$1 WHERE id=$2",
                        )
                        .bind(serde_json::to_string(&proposed).map_err(storage)?)
                        .bind(&proposed_key)
                        .execute(&mut *tx)
                        .await
                        .map_err(storage)?;
                    }
                } else if cas_tx(&mut tx, "sources", proposed_key.clone(), None, 0, &proposed)
                    .await?
                    != 1
                {
                    return Err(storage("personal_feed source identity collision"));
                }
                sqlx::query("INSERT INTO source_urls(url,source_id) VALUES($1,$2)")
                    .bind(canonical.as_str())
                    .bind(proposed_key)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                proposed_id
            };
            let old_revision = subscription.revision();
            let old_key = workspace_feed_url_key(workspace, subscription.source_url_exact());
            subscription
                .replace_source(canonical.clone(), exact_url.clone(), raw.name.clone())
                .map_err(storage)?;
            let new_key = workspace_feed_url_key(workspace, subscription.source_url_exact());
            if let Some(owner) = sqlx::query_scalar::<_, String>(
                "SELECT subscription_id FROM workspace_feed_urls WHERE id=$1 FOR UPDATE",
            )
            .bind(&new_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            {
                if owner != subscription.id().as_uuid().to_string() {
                    return Err(storage(format!(
                        "canonical source {} is already subscribed in this workspace",
                        canonical
                    )));
                }
            }
            if cas_tx(
                &mut tx,
                "subscriptions",
                subscription.id().as_uuid().to_string(),
                Some(old_revision),
                subscription.revision(),
                &subscription,
            )
            .await?
                != 1
            {
                return Err(RepositoryError::Conflict);
            }
            sqlx::query("DELETE FROM workspace_feed_urls WHERE id=$1")
                .bind(old_key)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            sqlx::query("INSERT INTO workspace_feed_urls(id,subscription_id) VALUES($1,$2) ON CONFLICT(id) DO UPDATE SET subscription_id=EXCLUDED.subscription_id")
                .bind(new_key).bind(subscription.id().as_uuid().to_string()).execute(&mut *tx).await.map_err(storage)?;
            sqlx::query("UPDATE subscription_sources SET source_id=$1 WHERE subscription_id=$2")
                .bind(actual_source.as_uuid().to_string())
                .bind(subscription.id().as_uuid().to_string())
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            if let Some(recipe) = editable_recipe_document(workspace, &canonical, &kind)? {
                sqlx::query("INSERT INTO web_feed_recipes(id,revision,document) VALUES($1,0,$2) ON CONFLICT(id) DO UPDATE SET revision=web_feed_recipes.revision+1,document=EXCLUDED.document")
                    .bind(subscription.id().as_uuid().to_string()).bind(recipe).execute(&mut *tx).await.map_err(storage)?;
            } else {
                sqlx::query("DELETE FROM web_feed_recipes WHERE id=$1")
                    .bind(subscription.id().as_uuid().to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            }
            let item = if matches!(kind, SourceKind::WebPage(_) | SourceKind::BuiltIn(_)) {
                WorkItem::CollectWebFeed {
                    source_id: actual_source,
                }
            } else {
                WorkItem::PollSource {
                    source_id: actual_source,
                }
            };
            let identity = format!("personal-feed-migration/{source_id}");
            enqueue_work_tx(
                &mut tx,
                Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
                &item,
            )
            .await?;
            migrated += 1;
        }
        tx.commit().await.map_err(storage)?;
        Ok(migrated)
    }
}

async fn cas_tx<T: Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    kind: &str,
    id: String,
    expected: Option<u64>,
    revision: u64,
    value: &T,
) -> Result<u64, RepositoryError> {
    let table = table(kind)?;
    let document = serde_json::to_string(value).map_err(storage)?;
    let revision = i64::try_from(revision).map_err(storage)?;
    let result = if let Some(expected) = expected {
        let query = format!(
            "UPDATE {table} SET revision = $1, document = $2 WHERE id = $3 AND revision = $4"
        );
        sqlx::query(&query)
            .bind(revision)
            .bind(document)
            .bind(id)
            .bind(i64::try_from(expected).map_err(storage)?)
            .execute(&mut **tx)
            .await
            .map_err(storage)?
    } else {
        let query = format!("INSERT INTO {table} (id, revision, document) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING");
        sqlx::query(&query)
            .bind(id)
            .bind(revision)
            .bind(document)
            .execute(&mut **tx)
            .await
            .map_err(storage)?
    };
    Ok(result.rows_affected())
}

mod article;
mod identity;
mod operations;
mod rule;
mod subscription;
mod workspace;

mod presentation;
