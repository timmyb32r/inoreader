use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_application::{
    AccountRecord, ArticlePresentation, InviteRecord, PasswordResetRecord, ReaderRepository,
    RepositoryError, RuleApplicationProgress, SeedSource, SessionRecord, SourceUrlPreviewRecord,
    SubscriptionActivity, SubscriptionStats,
};
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
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresRepository {
    pool: PgPool,
    reason_policy: ReasonPolicy,
    initial_scope: usize,
}
impl PostgresRepository {
    pub async fn connect(
        options: PgConnectOptions,
        reason_policy: ReasonPolicy,
        initial_scope: usize,
    ) -> Result<Self, RepositoryError> {
        let pool = PgPool::connect_with(options).await.map_err(storage)?;
        crate::prepare_schema(&pool).await.map_err(storage)?;
        Self::new(pool, reason_policy, initial_scope)
    }
    pub fn new(
        pool: PgPool,
        reason_policy: ReasonPolicy,
        initial_scope: usize,
    ) -> Result<Self, RepositoryError> {
        if initial_scope == 0 {
            return Err(RepositoryError::Storage(
                "initial feed scope must be positive".into(),
            ));
        }
        Ok(Self {
            pool,
            reason_policy,
            initial_scope,
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

    async fn presentations(
        &self,
        workspace: WorkspaceId,
        include_content: bool,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        let articles = self.articles_by_workspace(workspace).await?;
        let mut result = Vec::with_capacity(articles.len());
        for article in articles {
            let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT o.source_record_id,s.document,m.document,f.document FROM library_origins o JOIN subscriptions s ON s.id=o.subscription_id LEFT JOIN content_manifests m ON m.id=o.source_record_id LEFT JOIN content_refresh_state f ON f.id='failure/' || o.source_record_id WHERE o.workspace_id=$1 AND o.article_id=$2",
            ).bind(workspace.as_uuid().to_string()).bind(article.id.as_uuid().to_string()).fetch_all(&self.pool).await.map_err(storage)?;
            let mut subscription_ids = Vec::new();
            let mut subscription_titles = Vec::new();
            let mut manifests = Vec::new();
            let mut failure_reason = None;
            for (record, subscription, manifest, failure) in rows {
                let subscription: Subscription =
                    serde_json::from_str(&subscription).map_err(storage)?;
                subscription.validate(self.reason_policy).map_err(storage)?;
                if subscription.workspace_id() != workspace {
                    return Err(storage("article origin crosses workspace ownership"));
                }
                if !subscription_ids.contains(&subscription.id()) {
                    subscription_ids.push(subscription.id());
                }
                if !subscription_titles
                    .iter()
                    .any(|v| v == subscription.title())
                {
                    subscription_titles.push(subscription.title().to_owned());
                }
                if let Some(manifest) = manifest {
                    manifests.push((
                        record,
                        serde_json::from_str::<reader_ingest::ContentManifestPointer>(&manifest)
                            .map_err(storage)?,
                    ));
                }
                if let Some(failure) = failure {
                    failure_reason = serde_json::from_str::<serde_json::Value>(&failure)
                        .ok()
                        .and_then(|v| {
                            v.get("diagnostic")
                                .and_then(|v| v.as_str())
                                .map(str::to_owned)
                        });
                }
            }
            let latest = manifests.into_iter().max_by_key(|(_, v)| v.fetched_at);
            let safe_html = if include_content {
                if let Some((record, pointer)) = latest.as_ref() {
                    let chunks = sqlx::query_scalar::<_, String>("SELECT bytes FROM staged_content_chunks WHERE record_id=$1 AND refresh_id=$2 AND representation='safe' ORDER BY ordinal")
                        .bind(record).bind(pointer.refresh_id.to_string()).fetch_all(&self.pool).await.map_err(storage)?;
                    let mut bytes = Vec::new();
                    for chunk in chunks {
                        bytes.extend(serde_json::from_str::<Vec<u8>>(&chunk).map_err(storage)?);
                    }
                    Some(
                        String::from_utf8(bytes)
                            .map_err(|_| storage("safe HTML content is not UTF-8"))?,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            let full_text_status = if latest.is_some() {
                "ready"
            } else if failure_reason.is_some() {
                "failed"
            } else {
                "pending"
            };
            result.push(ArticlePresentation {
                article,
                subscription_ids,
                subscription_titles,
                safe_html,
                full_text_status,
                failure_reason,
            });
        }
        Ok(result)
    }
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredSelector {
    language: String,
    expression: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredWebRecipe {
    workspace_id: Uuid,
    url: String,
    selector: String,
    loading: String,
    #[serde(rename = "preview")]
    _preview: Option<bool>,
    #[serde(default)]
    selector_language: Option<String>,
    #[serde(default)]
    viewport: Option<String>,
    #[serde(default)]
    listing_url: Option<String>,
    #[serde(default)]
    card_selector: Option<String>,
    #[serde(default)]
    title_selector: Option<String>,
    #[serde(default)]
    date_selector: Option<String>,
    #[serde(default)]
    content_selector: Option<String>,
    #[serde(default)]
    wait_selector: Option<String>,
    #[serde(default)]
    url_pattern: Option<String>,
    #[serde(default)]
    max_pages: Option<usize>,
    #[serde(default)]
    hide_overlays: Vec<StoredSelector>,
    #[serde(default)]
    start_pages: Vec<String>,
    #[serde(default)]
    next_page: Option<StoredSelector>,
    #[serde(default)]
    load_more: Option<StoredSelector>,
    #[serde(default)]
    load_more_clicks: usize,
    #[serde(default)]
    scrolls: usize,
}

fn selector(value: &StoredSelector) -> Result<WebSelector, RepositoryError> {
    let language = match value.language.as_str() {
        "css" => SelectorLanguage::Css,
        "xpath" => SelectorLanguage::XPath,
        _ => return Err(storage("invalid selector language")),
    };
    WebSelector::new(language, value.expression.clone()).map_err(storage)
}

fn stored_web_recipe(document: &str) -> Result<(StoredWebRecipe, WebFeedRecipe), RepositoryError> {
    let raw: StoredWebRecipe = serde_json::from_str(document).map_err(storage)?;
    let loading = match raw.loading.as_str() {
        "automatic" => WebLoading::Automatic,
        "static" => WebLoading::Static,
        "browser" => WebLoading::Browser,
        _ => return Err(storage("invalid web loading strategy")),
    };
    let language = match raw.selector_language.as_deref().unwrap_or("css") {
        "css" => SelectorLanguage::Css,
        "xpath" => SelectorLanguage::XPath,
        _ => return Err(storage("invalid selector language")),
    };
    let primary = WebSelector::new(language, raw.selector.clone()).map_err(storage)?;
    let viewport = match raw.viewport.as_deref().unwrap_or("desktop") {
        "desktop" => WebViewport::Desktop,
        "mobile" => WebViewport::Mobile,
        _ => return Err(storage("invalid viewport")),
    };
    let overlays = raw
        .hide_overlays
        .iter()
        .map(selector)
        .collect::<Result<Vec<_>, _>>()?;
    let pages = raw
        .start_pages
        .iter()
        .map(|value| url::Url::parse(value).map_err(|_| storage("invalid start page URL")))
        .collect::<Result<Vec<_>, _>>()?;
    let next = raw.next_page.as_ref().map(selector).transpose()?;
    let load_more = raw.load_more.as_ref().map(selector).transpose()?;
    let actions = WebFeedActions::new(
        viewport,
        overlays,
        pages,
        next,
        load_more,
        raw.load_more_clicks,
        raw.scrolls,
        usize::MAX,
        usize::MAX,
    )
    .map_err(storage)?;
    let listing = raw
        .listing_url
        .as_deref()
        .map(url::Url::parse)
        .transpose()
        .map_err(|_| storage("invalid listing URL"))?;
    let extraction = WebExtraction::new(
        listing,
        raw.card_selector.clone(),
        raw.title_selector.clone(),
        raw.date_selector.clone(),
        raw.content_selector.clone(),
        raw.wait_selector.clone(),
        raw.url_pattern.clone(),
    )
    .map_err(storage)?;
    let recipe = WebFeedRecipe::legacy(
        primary,
        loading,
        actions,
        extraction,
        raw.max_pages.unwrap_or(1),
    )
    .map_err(storage)?;
    Ok((raw, recipe))
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
        WebFeedRecipe::legacy(
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

async fn enqueue_work_tx(
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
        let key = format!(
            "{}\0{}",
            value.workspace_id().as_uuid(),
            value.source_url_exact()
        );
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

#[async_trait]
impl ReaderRepository for PostgresRepository {
    async fn readiness(&self) -> Result<(), RepositoryError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn save_job(&self, e: Option<u64>, v: DurableJob) -> Result<(), RepositoryError> {
        self.cas("jobs", v.id.to_string(), String::new(), e, v.revision, &v)
            .await
    }
    async fn account(&self, id: AccountId) -> Result<AccountRecord, RepositoryError> {
        self.read("accounts", id.as_uuid().to_string()).await
    }
    async fn account_by_username(&self, u: &str) -> Result<AccountRecord, RepositoryError> {
        let document: String =
            sqlx::query_scalar("SELECT document FROM username_reservations WHERE id = $1")
                .bind(u)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
                .ok_or(RepositoryError::NotFound)?;
        self.account(AccountId::from_uuid(
            Uuid::parse_str(&document).map_err(storage)?,
        ))
        .await
    }
    async fn save_account(&self, e: Option<u64>, v: AccountRecord) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if e.is_none() {
            sqlx::query(
                "INSERT INTO username_reservations (id, revision, document) VALUES ($1, 0, $2)",
            )
            .bind(&v.username)
            .bind(v.id.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        }
        if cas_tx(
            &mut tx,
            "accounts",
            v.id.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn create_account_and_workspace(
        &self,
        a: AccountRecord,
        w: Workspace,
    ) -> Result<(), RepositoryError> {
        if w.owner() != a.id || w.revision() != 0 {
            return Err(RepositoryError::Storage(
                "initial workspace must belong to the new account at revision zero".into(),
            ));
        }
        w.validate(self.reason_policy).map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query(
            "INSERT INTO username_reservations (id, revision, document) VALUES ($1, 0, $2)",
        )
        .bind(&a.username)
        .bind(a.id.as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(|_| RepositoryError::Conflict)?;
        if cas_tx(
            &mut tx,
            "accounts",
            a.id.as_uuid().to_string(),
            None,
            a.revision,
            &a,
        )
        .await?
            != 1
            || cas_tx(
                &mut tx,
                "workspaces",
                w.id().as_uuid().to_string(),
                None,
                w.revision(),
                &w,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn invite_by_token_hash(&self, h: &str) -> Result<InviteRecord, RepositoryError> {
        self.read("invites", h.to_owned()).await
    }
    async fn save_invite(&self, e: Option<u64>, v: InviteRecord) -> Result<(), RepositoryError> {
        self.cas(
            "invites",
            v.token_hash.clone(),
            String::new(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn session_by_verifier_hash(&self, h: &str) -> Result<SessionRecord, RepositoryError> {
        self.read("sessions", h.to_owned()).await
    }
    async fn save_session(&self, e: Option<u64>, v: SessionRecord) -> Result<(), RepositoryError> {
        self.cas(
            "sessions",
            v.verifier_hash.clone(),
            v.account_id.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn delete_session(&self, h: &str, e: u64) -> Result<(), RepositoryError> {
        let n = sqlx::query("DELETE FROM sessions WHERE id = $1 AND revision = $2")
            .bind(h)
            .bind(e as i64)
            .execute(&self.pool)
            .await
            .map_err(storage)?
            .rows_affected();
        if n != 1 {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }
    async fn password_reset_by_token_hash(
        &self,
        h: &str,
    ) -> Result<PasswordResetRecord, RepositoryError> {
        self.read("password_resets", h.to_owned()).await
    }
    async fn save_password_reset(
        &self,
        e: Option<u64>,
        v: PasswordResetRecord,
    ) -> Result<(), RepositoryError> {
        self.cas(
            "password_resets",
            v.token_hash.clone(),
            v.account_id.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn consume_invite_create_account_and_workspace(
        &self,
        expected: u64,
        invite: InviteRecord,
        account: AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError> {
        workspace.validate(self.reason_policy).map_err(storage)?;
        if workspace.owner() != account.id {
            return Err(storage(
                "initial workspace must belong to the invited account",
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let username_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM username_reservations WHERE id = $1)")
                .bind(&account.username)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if username_exists
            || cas_tx(
                &mut tx,
                "invites",
                invite.token_hash.clone(),
                Some(expected),
                invite.revision,
                &invite,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("INSERT INTO username_reservations (id,revision,document) VALUES ($1,0,$2)")
            .bind(&account.username)
            .bind(account.id.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| RepositoryError::Conflict)?;
        if cas_tx(
            &mut tx,
            "accounts",
            account.id.as_uuid().to_string(),
            None,
            account.revision,
            &account,
        )
        .await?
            != 1
            || cas_tx(
                &mut tx,
                "workspaces",
                workspace.id().as_uuid().to_string(),
                None,
                workspace.revision(),
                &workspace,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn consume_reset_and_update_account(
        &self,
        expected_reset: u64,
        reset: PasswordResetRecord,
        expected_account: u64,
        account: AccountRecord,
    ) -> Result<(), RepositoryError> {
        if reset.account_id != account.id {
            return Err(storage(
                "password reset account does not match updated account",
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if cas_tx(
            &mut tx,
            "password_resets",
            reset.token_hash.clone(),
            Some(expected_reset),
            reset.revision,
            &reset,
        )
        .await?
            != 1
            || cas_tx(
                &mut tx,
                "accounts",
                account.id.as_uuid().to_string(),
                Some(expected_account),
                account.revision,
                &account,
            )
            .await?
                != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
    async fn workspace(&self, id: WorkspaceId) -> Result<Workspace, RepositoryError> {
        let v: Workspace = self.read("workspaces", id.as_uuid().to_string()).await?;
        v.validate(self.reason_policy).map_err(storage)?;
        Ok(v)
    }
    async fn workspaces_by_owner(&self, o: AccountId) -> Result<Vec<Workspace>, RepositoryError> {
        let values: Vec<Workspace> = self.list("workspaces", o.as_uuid().to_string()).await?;
        for value in &values {
            value.validate(self.reason_policy).map_err(storage)?;
            if value.owner() != o {
                return Err(storage(
                    "workspace owner index returned a foreign workspace",
                ));
            }
        }
        Ok(values)
    }
    async fn save_workspace(&self, e: Option<u64>, v: Workspace) -> Result<(), RepositoryError> {
        v.validate(self.reason_policy).map_err(storage)?;
        self.cas(
            "workspaces",
            v.id().as_uuid().to_string(),
            v.owner().as_uuid().to_string(),
            e,
            v.revision(),
            &v,
        )
        .await
    }
    async fn restore_workspace_with_refreshes(
        &self,
        expected: u64,
        value: Workspace,
        active: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        value.validate(self.reason_policy).map_err(storage)?;
        if !value.accepts_delivery() {
            return Err(storage("workspace catch-up requires an active workspace"));
        }
        for subscription in &active {
            if subscription.workspace_id() != value.id()
                || !matches!(subscription.status(), SubscriptionStatus::Active)
            {
                return Err(storage(
                    "workspace catch-up contains an invalid subscription",
                ));
            }
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if cas_tx(
            &mut tx,
            "workspaces",
            value.id().as_uuid().to_string(),
            Some(expected),
            value.revision(),
            &value,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        for subscription in active {
            let source = source_for_subscription(&mut tx, subscription.id()).await?;
            enqueue_backfill_tx(&mut tx, source, subscription.id(), self.initial_scope).await?;
            let item = WorkItem::RefreshSource { source_id: source };
            let identity = format!(
                "workspace-restore/{}/{}/{}",
                value.id().as_uuid(),
                value.revision(),
                subscription.id().as_uuid()
            );
            enqueue_work_tx(
                &mut tx,
                Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
                &item,
            )
            .await?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn subscription(&self, id: SubscriptionId) -> Result<Subscription, RepositoryError> {
        let v: Subscription = self.read("subscriptions", id.as_uuid().to_string()).await?;
        v.validate(self.reason_policy).map_err(storage)?;
        Ok(v)
    }
    async fn subscriptions_by_workspace(
        &self,
        w: WorkspaceId,
    ) -> Result<Vec<Subscription>, RepositoryError> {
        let values: Vec<Subscription> = self.list("subscriptions", w.as_uuid().to_string()).await?;
        for value in &values {
            value.validate(self.reason_policy).map_err(storage)?;
            if value.workspace_id() != w {
                return Err(storage("subscription index returned a foreign workspace"));
            }
        }
        Ok(values)
    }
    async fn subscription_stats(
        &self,
        workspace: WorkspaceId,
        s: &[Subscription],
    ) -> Result<HashMap<SubscriptionId, SubscriptionStats>, RepositoryError> {
        if s.iter().any(|value| value.workspace_id() != workspace) {
            return Err(RepositoryError::NotFound);
        }
        let mut result = s
            .iter()
            .map(|v| (v.id(), SubscriptionStats::default()))
            .collect::<HashMap<_, _>>();
        if s.is_empty() {
            return Ok(result);
        }
        let wanted = s
            .iter()
            .map(|v| v.id().as_uuid().to_string())
            .collect::<HashSet<_>>();
        let origin_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT subscription_id,article_id FROM library_origins WHERE workspace_id = $1",
        )
        .bind(workspace.as_uuid().to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut articles = HashMap::<String, HashSet<String>>::new();
        for (subscription, article) in origin_rows {
            if wanted.contains(&subscription) {
                articles.entry(subscription).or_default().insert(article);
            }
        }
        let unread = self
            .articles_by_workspace(workspace)
            .await?
            .into_iter()
            .filter(|v| !v.state.read)
            .map(|v| v.id.as_uuid().to_string())
            .collect::<HashSet<_>>();
        for subscription in s {
            let id = subscription.id().as_uuid().to_string();
            let Some(source): Option<String> = sqlx::query_scalar(
                "SELECT source_id FROM subscription_sources WHERE subscription_id = $1",
            )
            .bind(&id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            else {
                continue;
            };
            let health: Option<String> =
                sqlx::query_scalar("SELECT document FROM source_health WHERE source_id = $1")
                    .bind(&source)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(storage)?;
            let health = health
                .as_deref()
                .map(serde_json::from_str::<SourceHealth>)
                .transpose()
                .map_err(storage)?;
            let definition: Option<String> =
                sqlx::query_scalar("SELECT document FROM sources WHERE id = $1")
                    .bind(&source)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(storage)?;
            let source_type = definition
                .as_deref()
                .map(serde_json::from_str::<SourceDefinition>)
                .transpose()
                .map_err(storage)?
                .map(|v| match v.kind() {
                    SourceKind::WebPage(_) => "web",
                    SourceKind::BuiltIn(_) => "built_in",
                    _ => "feed",
                })
                .unwrap_or("feed")
                .to_owned();
            let ids = articles.get(&id);
            let count = ids.map_or(0, HashSet::len);
            let unread_count = ids.map_or(0, |values| values.intersection(&unread).count());
            let editable_web_feed: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM web_feed_recipes WHERE id = $1)")
                    .bind(&id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(storage)?;
            result.insert(
                subscription.id(),
                SubscriptionStats {
                    article_count: count,
                    unread_count,
                    last_success_at: health
                        .as_ref()
                        .and_then(|v| v.last_success_ms)
                        .and_then(DateTime::from_timestamp_millis),
                    last_error_at: health
                        .as_ref()
                        .and_then(|v| v.last_error_ms)
                        .and_then(DateTime::from_timestamp_millis),
                    consecutive_failures: health.as_ref().map_or(0, |v| v.consecutive_failures),
                    incomplete: health.as_ref().is_some_and(|v| v.incomplete),
                    continuation: health
                        .as_ref()
                        .is_some_and(|v| v.incomplete)
                        .then(|| "A durable continuation is queued".to_owned()),
                    error: health.and_then(|v| v.error),
                    editable_web_feed,
                    source_type,
                },
            );
        }
        Ok(result)
    }
    async fn subscription_activity(
        &self,
        owner: AccountId,
        id: SubscriptionId,
        since: DateTime<Utc>,
    ) -> Result<Vec<SubscriptionActivity>, RepositoryError> {
        let s = self.subscription(id).await?;
        if self.workspace(s.workspace_id()).await?.owner() != owner {
            return Err(RepositoryError::NotFound);
        };
        sqlx::query_scalar::<_,String>("SELECT document FROM subscription_activity WHERE subscription_id=$1 AND occurred_at_ms >= $2 ORDER BY occurred_at_ms DESC").bind(id.as_uuid().to_string()).bind(since.timestamp_millis()).fetch_all(&self.pool).await.map_err(storage)?.into_iter().map(|v|serde_json::from_str(&v).map_err(storage)).collect()
    }
    async fn save_subscription(
        &self,
        e: Option<u64>,
        v: Subscription,
    ) -> Result<(), RepositoryError> {
        v.validate(self.reason_policy).map_err(storage)?;
        if e.is_none() {
            let source =
                SourceDefinition::new(SourceId::new(), v.source_url().clone(), SourceKind::Auto)
                    .map_err(storage)?;
            let item = WorkItem::PollSource {
                source_id: source.id(),
            };
            let mut tx = self.pool.begin().await.map_err(storage)?;
            provision_subscription_tx(
                &mut tx,
                &v,
                &source,
                item,
                JobId::new().as_uuid(),
                self.initial_scope,
                true,
                false,
                None,
            )
            .await?;
            return tx.commit().await.map_err(storage);
        }
        self.cas(
            "subscriptions",
            v.id().as_uuid().to_string(),
            v.workspace_id().as_uuid().to_string(),
            e,
            v.revision(),
            &v,
        )
        .await
    }
    async fn replace_subscription_source(
        &self,
        e: u64,
        v: Subscription,
    ) -> Result<(), RepositoryError> {
        v.validate(self.reason_policy).map_err(storage)?;
        let current = self.subscription(v.id()).await?;
        if current.revision() != e {
            return Err(RepositoryError::Conflict);
        }
        if current.workspace_id() != v.workspace_id() {
            return Err(storage("subscription source replacement changes workspace"));
        }
        let proposed =
            SourceDefinition::new(SourceId::new(), v.source_url().clone(), SourceKind::Auto)
                .map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let old_key = format!(
            "{}\0{}",
            v.workspace_id().as_uuid(),
            current.source_url_exact()
        );
        let new_key = format!("{}\0{}", v.workspace_id().as_uuid(), v.source_url_exact());
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT subscription_id FROM workspace_feed_urls WHERE id = $1 FOR UPDATE",
        )
        .bind(&new_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if existing
            .as_deref()
            .is_some_and(|owner| owner != v.id().as_uuid().to_string())
        {
            return Err(RepositoryError::Conflict);
        }
        let inserted = sqlx::query(
            "INSERT INTO source_urls(url,source_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
        )
        .bind(v.source_url().as_str())
        .bind(proposed.id().as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected()
            == 1;
        let source = if inserted {
            cas_tx(
                &mut tx,
                "sources",
                proposed.id().as_uuid().to_string(),
                None,
                0,
                &proposed,
            )
            .await?;
            proposed.id()
        } else {
            let id: String = sqlx::query_scalar("SELECT source_id FROM source_urls WHERE url=$1")
                .bind(v.source_url().as_str())
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
            SourceId::from_uuid(Uuid::parse_str(&id).map_err(storage)?)
        };
        if cas_tx(
            &mut tx,
            "subscriptions",
            v.id().as_uuid().to_string(),
            Some(e),
            v.revision(),
            &v,
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
        sqlx::query("INSERT INTO workspace_feed_urls(id,subscription_id) VALUES($1,$2) ON CONFLICT(id) DO UPDATE SET subscription_id=EXCLUDED.subscription_id").bind(new_key).bind(v.id().as_uuid().to_string()).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE subscription_sources SET source_id=$1 WHERE subscription_id=$2")
            .bind(source.as_uuid().to_string())
            .bind(v.id().as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        enqueue_work_tx(
            &mut tx,
            JobId::new().as_uuid(),
            &WorkItem::PollSource { source_id: source },
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn source_url_preview(
        &self,
        id: Uuid,
    ) -> Result<SourceUrlPreviewRecord, RepositoryError> {
        self.read("source_url_previews", id.to_string()).await
    }
    async fn save_source_url_preview(
        &self,
        v: SourceUrlPreviewRecord,
    ) -> Result<(), RepositoryError> {
        self.cas(
            "source_url_previews",
            v.id.to_string(),
            v.subscription_id.as_uuid().to_string(),
            None,
            v.revision,
            &v,
        )
        .await
    }
    async fn activate_subscription_with_refresh(
        &self,
        expected: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        value.validate(self.reason_policy).map_err(storage)?;
        if !matches!(value.status(), SubscriptionStatus::Active) {
            return Err(storage(
                "catch-up activation requires an active subscription",
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if cas_tx(
            &mut tx,
            "subscriptions",
            value.id().as_uuid().to_string(),
            Some(expected),
            value.revision(),
            &value,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        let source = source_for_subscription(&mut tx, value.id()).await?;
        enqueue_backfill_tx(&mut tx, source, value.id(), self.initial_scope).await?;
        let identity = format!(
            "subscription-activation/{}/{}",
            value.id().as_uuid(),
            value.revision()
        );
        enqueue_work_tx(
            &mut tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &WorkItem::RefreshSource { source_id: source },
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn enqueue_subscription_refresh(
        &self,
        value: &Subscription,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let source = source_for_subscription(&mut tx, value.id()).await?;
        let identity = format!("manual-poll/{}/{}", value.id().as_uuid(), value.revision());
        enqueue_work_tx(
            &mut tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &WorkItem::RefreshSource { source_id: source },
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn save_web_feed_subscription(
        &self,
        value: Subscription,
        recipe_json: String,
    ) -> Result<(), RepositoryError> {
        value.validate(self.reason_policy).map_err(storage)?;
        let (raw, recipe) = stored_web_recipe(&recipe_json)?;
        if raw.workspace_id != value.workspace_id().as_uuid()
            || raw.url != value.source_url().as_str()
        {
            return Err(storage(
                "web feed recipe scope or URL does not match subscription",
            ));
        }
        let source = SourceDefinition::new(
            SourceId::new(),
            value.source_url().clone(),
            SourceKind::WebPage(recipe),
        )
        .map_err(storage)?;
        let item = WorkItem::CollectWebFeed {
            source_id: source.id(),
        };
        let mut tx = self.pool.begin().await.map_err(storage)?;
        provision_subscription_tx(
            &mut tx,
            &value,
            &source,
            item,
            JobId::new().as_uuid(),
            self.initial_scope,
            false,
            true,
            Some(&recipe_json),
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn web_feed_recipe(&self, id: SubscriptionId) -> Result<(u64, String), RepositoryError> {
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT revision,document FROM web_feed_recipes WHERE id=$1")
                .bind(id.as_uuid().to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        let (r, d) = row.ok_or(RepositoryError::NotFound)?;
        Ok((u64::try_from(r).map_err(storage)?, d))
    }
    async fn update_web_feed_recipe(
        &self,
        subscription: SubscriptionId,
        expected: u64,
        recipe_json: String,
    ) -> Result<u64, RepositoryError> {
        let value = self.subscription(subscription).await?;
        let (raw, recipe) = stored_web_recipe(&recipe_json)?;
        if raw.workspace_id != value.workspace_id().as_uuid()
            || raw.url != value.source_url().as_str()
        {
            return Err(storage(
                "web feed recipe scope or URL does not match subscription",
            ));
        }
        let next = expected
            .checked_add(1)
            .ok_or_else(|| storage("web feed recipe version overflow"))?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let source_id = source_for_subscription(&mut tx, subscription).await?;
        let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id=$1")
            .bind(source_id.as_uuid().to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
        let source: SourceDefinition = serde_json::from_str(&document).map_err(storage)?;
        let source = source
            .revise_kind(SourceKind::WebPage(recipe))
            .map_err(storage)?;
        let changed = sqlx::query(
            "UPDATE web_feed_recipes SET revision=$1,document=$2 WHERE id=$3 AND revision=$4",
        )
        .bind(i64::try_from(next).map_err(storage)?)
        .bind(&recipe_json)
        .bind(subscription.as_uuid().to_string())
        .bind(i64::try_from(expected).map_err(storage)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("UPDATE sources SET revision=$1,document=$2 WHERE id=$3")
            .bind(i64::try_from(next).map_err(storage)?)
            .bind(serde_json::to_string(&source).map_err(storage)?)
            .bind(source_id.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let identity = format!("web-recipe/{}/{next}", subscription.as_uuid());
        enqueue_work_tx(
            &mut tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &WorkItem::CollectWebFeed { source_id },
        )
        .await?;
        tx.commit().await.map_err(storage)?;
        Ok(next)
    }
    async fn article(&self, w: WorkspaceId, id: ArticleId) -> Result<Article, RepositoryError> {
        self.read("articles", article_key(w, id)).await
    }
    async fn articles_by_workspace(&self, w: WorkspaceId) -> Result<Vec<Article>, RepositoryError> {
        self.list("articles", w.as_uuid().to_string()).await
    }
    async fn article_presentations_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        self.presentations(workspace, true).await
    }
    async fn article_summaries_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        self.presentations(workspace, false).await
    }
    async fn article_presentation(
        &self,
        workspace: WorkspaceId,
        id: ArticleId,
    ) -> Result<ArticlePresentation, RepositoryError> {
        self.presentations(workspace, true)
            .await?
            .into_iter()
            .find(|v| v.article.id == id)
            .ok_or(RepositoryError::NotFound)
    }
    async fn save_article(
        &self,
        w: WorkspaceId,
        e: Option<u64>,
        v: Article,
    ) -> Result<(), RepositoryError> {
        self.cas(
            "articles",
            article_key(w, v.id),
            w.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn enqueue_article_full_text_refresh(
        &self,
        workspace: WorkspaceId,
        value: &Article,
    ) -> Result<(), RepositoryError> {
        let value = self.article(workspace, value.id).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut enqueued = 0;
        for origin in &value.origins {
            let document: String =
                sqlx::query_scalar("SELECT document FROM source_records WHERE id=$1")
                    .bind(origin.as_uuid().to_string())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
                    .ok_or(RepositoryError::NotFound)?;
            let record: SourceRecord = serde_json::from_str(&document).map_err(storage)?;
            let Some(url) = record.key().location.fetch_url().cloned() else {
                continue;
            };
            let item = WorkItem::ExtractFullText {
                record_id: record.id(),
                source_revision: record.revision(),
                url,
                manual: true,
            };
            enqueue_work_tx(&mut tx, Uuid::new_v4(), &item).await?;
            enqueued += 1;
        }
        if enqueued == 0 {
            return Err(storage("article has no fetchable URL origin"));
        }
        tx.commit().await.map_err(storage)
    }
    async fn rules_by_workspace(&self, w: WorkspaceId) -> Result<Vec<Rule>, RepositoryError> {
        let values: Vec<Rule> = self.list("rules", w.as_uuid().to_string()).await?;
        for value in &values {
            value.validate().map_err(storage)?;
        }
        Ok(values)
    }
    async fn rule(&self, w: WorkspaceId, id: RuleId) -> Result<Rule, RepositoryError> {
        let value: Rule = self.read("rules", rule_key(w, id)).await?;
        value.validate().map_err(storage)?;
        Ok(value)
    }
    async fn save_rule(
        &self,
        w: WorkspaceId,
        e: Option<u64>,
        v: Rule,
    ) -> Result<(), RepositoryError> {
        v.validate().map_err(storage)?;
        self.cas(
            "rules",
            rule_key(w, v.id),
            w.as_uuid().to_string(),
            e,
            v.version,
            &v,
        )
        .await
    }
    async fn delete_rule(&self, w: WorkspaceId, id: RuleId, e: u64) -> Result<(), RepositoryError> {
        let n = sqlx::query("DELETE FROM rules WHERE id=$1 AND revision=$2")
            .bind(rule_key(w, id))
            .bind(i64::try_from(e).map_err(storage)?)
            .execute(&self.pool)
            .await
            .map_err(storage)?
            .rows_affected();
        if n == 1 {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn enqueue_rule_application(
        &self,
        workspace: WorkspaceId,
        rule: Rule,
    ) -> Result<Uuid, RepositoryError> {
        rule.validate().map_err(storage)?;
        let identity = format!(
            "apply-rule/{}/{}/{}",
            workspace.as_uuid(),
            rule.id.as_uuid(),
            rule.version
        );
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes());
        let through_article = self
            .articles_by_workspace(workspace)
            .await?
            .into_iter()
            .map(|v| v.id)
            .max_by_key(|v| v.as_uuid());
        let item = WorkItem::ApplyRule {
            workspace_id: workspace,
            rule_id: rule.id,
            rule_version: rule.version,
            after_article: None,
            through_article,
        };
        let mut tx = self.pool.begin().await.map_err(storage)?;
        enqueue_work_tx(&mut tx, id, &item).await?;
        tx.commit().await.map_err(storage)?;
        Ok(id)
    }
    async fn rule_application_progress(
        &self,
        workspace: WorkspaceId,
        operation: Uuid,
    ) -> Result<RuleApplicationProgress, RepositoryError> {
        let initial: String = sqlx::query_scalar("SELECT item FROM ingest_jobs WHERE id=$1")
            .bind(operation.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
        let WorkItem::ApplyRule {
            workspace_id,
            rule_id,
            rule_version,
            ..
        } = serde_json::from_str(&initial).map_err(storage)?
        else {
            return Err(RepositoryError::NotFound);
        };
        if workspace_id != workspace {
            return Err(RepositoryError::NotFound);
        }
        let cancel_reason = match self.rule(workspace, rule_id).await {
            Err(RepositoryError::NotFound) => Some("rule was deleted".to_owned()),
            Err(error) => return Err(error),
            Ok(rule) if rule.version != rule_version => {
                Some(format!("rule version changed to {}", rule.version))
            }
            Ok(rule) if !rule.enabled => Some("rule was disabled".to_owned()),
            Ok(_) => None,
        };
        let rows: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT status,item,diagnostic FROM ingest_jobs WHERE origin_key=$1")
                .bind(format!("internal:workspace:{}", workspace.as_uuid()))
                .fetch_all(&self.pool)
                .await
                .map_err(storage)?;
        let mut pending = false;
        let mut failure = None;
        for (status, item, diagnostic) in rows {
            let Ok(WorkItem::ApplyRule {
                workspace_id: job_workspace,
                rule_id: job_rule,
                rule_version: job_version,
                ..
            }) = serde_json::from_str::<WorkItem>(&item)
            else {
                continue;
            };
            if job_workspace != workspace || job_rule != rule_id || job_version != rule_version {
                continue;
            };
            match status.as_str() {
                "ready" | "leased" | "retry" => pending = true,
                "failed" => failure = diagnostic.or(Some("rule application failed".to_owned())),
                _ => {}
            }
        }
        let evaluated: i64 = sqlx::query_scalar("SELECT count(*) FROM rule_evaluations WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3").bind(workspace.as_uuid().to_string()).bind(rule_id.as_uuid().to_string()).bind(i64::try_from(rule_version).map_err(storage)?).fetch_one(&self.pool).await.map_err(storage)?;
        let (status, cancel_reason) = if let Some(reason) = cancel_reason {
            ("cancelled".to_owned(), Some(reason))
        } else if let Some(reason) = failure {
            ("failed".to_owned(), Some(reason))
        } else if pending {
            ("running".to_owned(), None)
        } else {
            ("completed".to_owned(), None)
        };
        Ok(RuleApplicationProgress {
            operation_id: operation,
            workspace_id: workspace,
            rule_id,
            rule_version,
            status,
            evaluated: usize::try_from(evaluated).map_err(storage)?,
            cancel_reason,
        })
    }
    async fn record_login_attempt(
        &self,
        u: &str,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<bool, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        #[derive(Serialize, Deserialize)]
        struct LoginAttempts {
            timestamps: Vec<DateTime<Utc>>,
            revision: u64,
        }
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT revision,document FROM login_attempts WHERE id=$1 FOR UPDATE")
                .bind(u)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        let (mut value, expected) = match row {
            Some((revision, document)) => (
                serde_json::from_str::<LoginAttempts>(&document).map_err(storage)?,
                Some(u64::try_from(revision).map_err(storage)?),
            ),
            None => (
                LoginAttempts {
                    timestamps: Vec::new(),
                    revision: 0,
                },
                None,
            ),
        };
        let cutoff = at - chrono::Duration::minutes(1);
        value.timestamps.retain(|v| *v > cutoff);
        if value.timestamps.len() >= limit as usize {
            return Ok(false);
        }
        value.timestamps.push(at);
        value.revision = expected.map_or(0, |v| v + 1);
        if cas_tx(
            &mut tx,
            "login_attempts",
            u.to_owned(),
            expected,
            value.revision,
            &value,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        tx.commit().await.map_err(storage)?;
        Ok(true)
    }
    async fn mark_articles_read_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Article>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        for value in &values {
            let expected = value
                .revision
                .checked_sub(1)
                .ok_or_else(|| storage("updated article revision is invalid"))?;
            let current: Option<i64> =
                sqlx::query_scalar("SELECT revision FROM articles WHERE id=$1 FOR UPDATE")
                    .bind(article_key(workspace, value.id))
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?;
            if current != Some(i64::try_from(expected).map_err(storage)?) {
                return Err(RepositoryError::Conflict);
            }
        }
        for value in values {
            let expected = value.revision - 1;
            if cas_tx(
                &mut tx,
                "articles",
                article_key(workspace, value.id),
                Some(expected),
                value.revision,
                &value,
            )
            .await?
                != 1
            {
                return Err(RepositoryError::Conflict);
            }
        }
        tx.commit().await.map_err(storage)
    }
    async fn import_subscriptions_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        for value in &values {
            if value.workspace_id() != workspace {
                return Err(storage(
                    "import subscription scope does not match target workspace",
                ));
            }
            value.validate(self.reason_policy).map_err(storage)?;
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        for value in values {
            let source = SourceDefinition::new(
                SourceId::new(),
                value.source_url().clone(),
                SourceKind::Auto,
            )
            .map_err(storage)?;
            let item = WorkItem::PollSource {
                source_id: source.id(),
            };
            provision_subscription_tx(
                &mut tx,
                &value,
                &source,
                item,
                JobId::new().as_uuid(),
                self.initial_scope,
                false,
                false,
                None,
            )
            .await?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn apply_seed_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<(String, Subscription, SeedSource, serde_json::Value)>,
    ) -> Result<(), RepositoryError> {
        struct Prepared {
            key: String,
            value: Subscription,
            configuration: String,
            source: SourceDefinition,
            item: WorkItem,
            recipe: Option<String>,
        }
        let mut prepared = Vec::with_capacity(values.len());
        for (key, value, kind, configuration) in values {
            if value.workspace_id() != workspace {
                return Err(storage(
                    "seed subscription scope does not match target workspace",
                ));
            }
            value.validate(self.reason_policy).map_err(storage)?;
            let source_id = SourceId::from_uuid(Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                value.source_url().as_str().as_bytes(),
            ));
            let source_kind = match kind {
                SeedSource::Feed => SourceKind::Auto,
                SeedSource::Imported => imported_source_kind(&configuration)?,
            };
            let recipe = editable_recipe_document(workspace, value.source_url(), &source_kind)?;
            let source =
                SourceDefinition::new(source_id, value.source_url().clone(), source_kind.clone())
                    .map_err(storage)?;
            let item = match source_kind {
                SourceKind::WebPage(_) | SourceKind::BuiltIn(_) => {
                    WorkItem::CollectWebFeed { source_id }
                }
                _ => WorkItem::PollSource { source_id },
            };
            prepared.push(Prepared {
                key,
                value,
                configuration: serde_json::to_string(&configuration).map_err(storage)?,
                source,
                item,
                recipe,
            });
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        for row in &prepared {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT document FROM subscriptions WHERE id=$1 FOR UPDATE",
            )
            .bind(row.value.id().as_uuid().to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            {
                if existing != serde_json::to_string(&row.value).map_err(storage)? {
                    return Err(storage(
                        "seed idempotency key conflicts with existing subscription",
                    ));
                }
            }
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT document FROM seed_items WHERE id=$1 FOR UPDATE",
            )
            .bind(&row.key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            {
                if existing != row.configuration {
                    return Err(storage(
                        "seed configuration conflicts with applied idempotency key",
                    ));
                }
            }
        }
        for row in prepared {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id=$1)")
                    .bind(row.value.id().as_uuid().to_string())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
            if exists {
                continue;
            }
            let job = Uuid::new_v5(
                &Uuid::NAMESPACE_OID,
                format!("seed-job/{}", row.key).as_bytes(),
            );
            provision_subscription_tx(
                &mut tx,
                &row.value,
                &row.source,
                row.item,
                job,
                self.initial_scope,
                false,
                row.recipe.is_some(),
                row.recipe.as_deref(),
            )
            .await?;
            sqlx::query("INSERT INTO seed_items(id,revision,document) VALUES($1,0,$2)")
                .bind(row.key)
                .bind(row.configuration)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
}
