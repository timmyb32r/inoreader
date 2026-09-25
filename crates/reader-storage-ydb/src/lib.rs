#![recursion_limit = "256"]
// The YDB SDK transaction macro clones captured values and presents them as
// references inside generated async closures. These lints misidentify those
// macro-bound values and the explicit atomic-operation ports as simplifiable.
#![allow(
    clippy::needless_borrow,
    clippy::needless_question_mark,
    clippy::possible_missing_else,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

//! YDB persistence boundary. The adapter uses optimistic revisions and conditional writes;
//! SDK-specific transport is injected so domain/application crates stay YDB-independent.
use async_trait::async_trait;
use reader_application::{ReaderRepository, RepositoryError};
use reader_core::*;
use reader_ingest::{
    BuiltInAdapter, JobId, SelectorLanguage, SourceDefinition, SourceKind, WebExtraction,
    WebFeedActions, WebFeedRecipe, WebLoading, WebSelector, WebViewport, WorkItem,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{ops::ControlFlow, sync::Arc, time::Duration};
use ydb::{
    Client, ClientBuilder, FromEnvCredentials, RetrySettings, RetryState, RetryStrategy,
    SessionPoolSettings, Transaction,
};
mod ingest_store;
pub use ingest_store::YdbIngestStore;

const SCHEMA_VERSION: u64 = 5;
#[derive(serde::Serialize, serde::Deserialize)]
struct SchemaMarker {
    version: u64,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct LoginAttempts {
    timestamps: Vec<chrono::DateTime<chrono::Utc>>,
    revision: u64,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredSelector {
    language: String,
    expression: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredWebRecipe {
    workspace_id: uuid::Uuid,
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
fn selector(value: StoredSelector) -> Result<reader_ingest::WebSelector, RepositoryError> {
    let language = match value.language.as_str() {
        "css" => reader_ingest::SelectorLanguage::Css,
        "xpath" => reader_ingest::SelectorLanguage::XPath,
        _ => return Err(RepositoryError::Storage("invalid selector language".into())),
    };
    reader_ingest::WebSelector::new(language, value.expression)
        .map_err(|e| RepositoryError::Storage(e.to_string()))
}
fn stored_web_recipe(
    document: &str,
) -> Result<(StoredWebRecipe, reader_ingest::WebFeedRecipe), RepositoryError> {
    let raw: StoredWebRecipe =
        serde_json::from_str(document).map_err(|e| RepositoryError::Storage(e.to_string()))?;
    let loading = match raw.loading.as_str() {
        "automatic" => WebLoading::Automatic,
        "static" => WebLoading::Static,
        "browser" => WebLoading::Browser,
        _ => {
            return Err(RepositoryError::Storage(
                "invalid web loading strategy".into(),
            ))
        }
    };
    let language = match raw.selector_language.as_deref().unwrap_or("css") {
        "css" => SelectorLanguage::Css,
        "xpath" => SelectorLanguage::XPath,
        _ => return Err(RepositoryError::Storage("invalid selector language".into())),
    };
    let primary = WebSelector::new(language, raw.selector.clone())
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    let viewport = match raw.viewport.as_deref().unwrap_or("desktop") {
        "desktop" => WebViewport::Desktop,
        "mobile" => WebViewport::Mobile,
        _ => return Err(RepositoryError::Storage("invalid viewport".into())),
    };
    let overlays = raw
        .hide_overlays
        .iter()
        .map(|value| {
            selector(StoredSelector {
                language: value.language.clone(),
                expression: value.expression.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pages = raw
        .start_pages
        .iter()
        .map(|value| {
            url::Url::parse(value)
                .map_err(|_| RepositoryError::Storage("invalid start page URL".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let next = raw
        .next_page
        .as_ref()
        .map(|value| {
            selector(StoredSelector {
                language: value.language.clone(),
                expression: value.expression.clone(),
            })
        })
        .transpose()?;
    let load_more = raw
        .load_more
        .as_ref()
        .map(|value| {
            selector(StoredSelector {
                language: value.language.clone(),
                expression: value.expression.clone(),
            })
        })
        .transpose()?;
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
    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    let listing = raw
        .listing_url
        .as_deref()
        .map(url::Url::parse)
        .transpose()
        .map_err(|_| RepositoryError::Storage("invalid listing URL".into()))?;
    let extraction = WebExtraction::new(
        listing,
        raw.card_selector.clone(),
        raw.title_selector.clone(),
        raw.date_selector.clone(),
        raw.content_selector.clone(),
        raw.wait_selector.clone(),
        raw.url_pattern.clone(),
    )
    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    let recipe = WebFeedRecipe::legacy(
        primary,
        loading,
        actions,
        extraction,
        raw.max_pages.unwrap_or(1),
    )
    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    Ok((raw, recipe))
}
fn editable_recipe_document(
    workspace: WorkspaceId,
    url: &url::Url,
    kind: &SourceKind,
) -> Result<Option<Vec<u8>>, RepositoryError> {
    let SourceKind::WebPage(recipe) = kind else {
        return Ok(None);
    };
    let language = |value: &WebSelector| match value.language() {
        SelectorLanguage::Css => "css",
        SelectorLanguage::XPath => "xpath",
    };
    let draft = serde_json::json!({"workspaceId":workspace.as_uuid(),"url":url.as_str(),"selector":recipe.selector(),"loading":match recipe.loading(){WebLoading::Automatic=>"automatic",WebLoading::Static=>"static",WebLoading::Browser=>"browser"},"preview":false,"selectorLanguage":match recipe.selector_kind(){SelectorLanguage::Css=>"css",SelectorLanguage::XPath=>"xpath"},"viewport":match recipe.actions().viewport(){WebViewport::Desktop=>"desktop",WebViewport::Mobile=>"mobile"},"listingUrl":recipe.extraction().listing_url().map(url::Url::as_str),"cardSelector":recipe.extraction().card_selector().map(WebSelector::expression),"titleSelector":recipe.extraction().title_selector().map(WebSelector::expression),"dateSelector":recipe.extraction().date_selector().map(WebSelector::expression),"contentSelector":recipe.extraction().content_selector().map(WebSelector::expression),"waitSelector":recipe.extraction().wait_selector().map(WebSelector::expression),"urlPattern":recipe.extraction().url_pattern(),"maxPages":recipe.max_pages(),"hideOverlays":recipe.actions().hide_overlays().iter().map(|v|serde_json::json!({"language":language(v),"expression":v.expression()})).collect::<Vec<_>>(),"startPages":recipe.actions().start_pages().iter().map(url::Url::as_str).collect::<Vec<_>>(),"nextPage":recipe.actions().next_page().map(|v|serde_json::json!({"language":language(v),"expression":v.expression()})),"loadMore":recipe.actions().load_more().map(|v|serde_json::json!({"language":language(v),"expression":v.expression()})),"loadMoreClicks":recipe.actions().load_more_clicks(),"scrolls":recipe.actions().scrolls()});
    Ok(Some(
        serde_json::to_vec(&draft).map_err(|e| RepositoryError::Storage(e.to_string()))?,
    ))
}
#[derive(serde::Deserialize)]
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
        .map_err(|e| RepositoryError::Storage(format!("invalid imported source recipe: {e}")))?;
    if raw.id.trim().is_empty() || raw.name.trim().is_empty() {
        return Err(RepositoryError::Storage(
            "imported source id/name is empty".into(),
        ));
    }
    let canonical = url::Url::parse(&raw.url)
        .map_err(|_| RepositoryError::Storage("invalid imported source URL".into()))?;
    if !matches!(canonical.scheme(), "http" | "https") {
        return Err(RepositoryError::Storage(
            "invalid imported source URL scheme".into(),
        ));
    }
    if raw.feed_url.is_some() {
        return Ok(SourceKind::Auto);
    }
    let parse_url =
        |value: Option<String>, fallback: &url::Url| -> Result<url::Url, RepositoryError> {
            match value {
                Some(value) => url::Url::parse(&value)
                    .map_err(|_| RepositoryError::Storage("invalid imported listing URL".into())),
                None => Ok(fallback.clone()),
            }
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
            _ => {
                return Err(RepositoryError::Storage(format!(
                    "unsupported imported adapter {adapter}"
                )))
            }
        };
        return Ok(SourceKind::BuiltIn(adapter));
    }
    let selector = raw.link_selector.ok_or_else(|| {
        RepositoryError::Storage("generic imported source requires link_selector".into())
    })?;
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
        .map(|value| {
            url::Url::parse(&value)
                .map_err(|_| RepositoryError::Storage("invalid extra listing URL".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let next = raw
        .next_selector
        .map(|value| {
            WebSelector::new(SelectorLanguage::Css, value)
                .map_err(|e| RepositoryError::Storage(e.to_string()))
        })
        .transpose()?;
    let load_more = raw
        .load_more_selector
        .map(|value| {
            WebSelector::new(SelectorLanguage::Css, value)
                .map_err(|e| RepositoryError::Storage(e.to_string()))
        })
        .transpose()?;
    let max_pages = raw.max_pages.unwrap_or(3);
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
    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    let listing_url = raw
        .listing_url
        .map(|value| {
            url::Url::parse(&value)
                .map_err(|_| RepositoryError::Storage("invalid imported listing URL".into()))
        })
        .transpose()?;
    let extraction = WebExtraction::new(
        listing_url,
        raw.card_selector,
        raw.title_selector,
        raw.date_selector,
        raw.content_selector,
        raw.wait_selector,
        raw.url_pattern,
    )
    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    let recipe = WebFeedRecipe::legacy(
        WebSelector::new(SelectorLanguage::Css, selector)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?,
        loading,
        actions,
        extraction,
        max_pages,
    )
    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
    Ok(SourceKind::WebPage(recipe))
}

/// Explicit production schema. None of these tables has a TTL.
pub const SCHEMA_DDL: &[&str] = &[
    "CREATE TABLE schema_metadata (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE workspaces (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE subscriptions (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE articles (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE jobs (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE outbox (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE content_manifests (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE accounts (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE username_reservations (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE invites (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE sessions (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE password_resets (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE login_attempts (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE rules (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE web_feed_recipes (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE sources (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE source_records (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE delivery_origins (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE content_chunks (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE content_refresh_state (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE ingest_jobs (id Utf8 NOT NULL, status Utf8 NOT NULL, run_at_ms Int64 NOT NULL, first_attempt_ms Int64 NOT NULL, origin_key Utf8 NOT NULL, attempt Uint32 NOT NULL, lease_token Utf8, lease_deadline_ms Int64, item String NOT NULL, revision Uint64 NOT NULL, diagnostic Utf8, PRIMARY KEY (id), INDEX ready_jobs GLOBAL ON (status, run_at_ms), INDEX leased_by_origin GLOBAL ON (status, origin_key, lease_deadline_ms))",
    "CREATE TABLE source_record_identity (source_id Utf8 NOT NULL, upstream_id Utf8 NOT NULL, record_id Utf8 NOT NULL, observed_at_ms Int64 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (source_id, upstream_id), INDEX recent_by_source GLOBAL ON (source_id, observed_at_ms))",
    "CREATE TABLE library_dedup (workspace_id Utf8 NOT NULL, dedup_key String NOT NULL, article_id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (workspace_id, dedup_key))",
    "CREATE TABLE library_origins (workspace_id Utf8 NOT NULL, article_id Utf8 NOT NULL, subscription_id Utf8 NOT NULL, source_record_id Utf8 NOT NULL, PRIMARY KEY (workspace_id, article_id, subscription_id, source_record_id), INDEX by_subscription GLOBAL ON (subscription_id, workspace_id))",
    "CREATE TABLE staged_content_chunks (record_id Utf8 NOT NULL, refresh_id Utf8 NOT NULL, representation Utf8 NOT NULL, ordinal Uint32 NOT NULL, bytes String NOT NULL, PRIMARY KEY (record_id, refresh_id, representation, ordinal))",
    "CREATE TABLE source_urls (url Utf8 NOT NULL, source_id Utf8 NOT NULL, PRIMARY KEY (url))",
    "CREATE TABLE subscription_sources (subscription_id Utf8 NOT NULL, source_id Utf8 NOT NULL, PRIMARY KEY (subscription_id))",
    "CREATE TABLE source_health (source_id Utf8 NOT NULL, document String NOT NULL, PRIMARY KEY (source_id))",
    "CREATE TABLE seed_items (id Utf8 NOT NULL, revision Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (id))",
    "CREATE TABLE rule_evaluations (workspace_id Utf8 NOT NULL, article_id Utf8 NOT NULL, rule_id Utf8 NOT NULL, rule_version Uint64 NOT NULL, document String NOT NULL, PRIMARY KEY (workspace_id, article_id, rule_id, rule_version))",
];

pub struct ProductionYdbTransport {
    pub(crate) client: Client,
}

/// Validated limits applied directly to the official YDB driver's retry budget
/// and query-session pool before the transport becomes operational.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct YdbClientLimits {
    request_timeout: Duration,
    max_concurrency: usize,
    retry_attempts: u32,
}
impl YdbClientLimits {
    pub fn new(
        request_timeout: Duration,
        max_concurrency: usize,
        retry_attempts: u32,
    ) -> Result<Self, String> {
        if request_timeout.is_zero() {
            return Err("YDB request timeout must be positive".into());
        }
        if max_concurrency == 0 {
            return Err("YDB max concurrency must be positive".into());
        }
        if retry_attempts == 0 {
            return Err("YDB retry attempts must be positive".into());
        }
        Ok(Self {
            request_timeout,
            max_concurrency,
            retry_attempts,
        })
    }
}
#[derive(Debug)]
struct MaximumAttempts(u32);
impl RetryStrategy for MaximumAttempts {
    async fn wait_retry(&self, state: &RetryState) -> ControlFlow<()> {
        if state.attempt.saturating_add(1) < self.0 as usize {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

fn http_origin(url: &url::Url) -> Result<String, ydb::YdbOrCustomerError> {
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(
            "job URL has no HTTP origin".into(),
        )));
    }
    Ok(url.origin().ascii_serialization())
}
async fn source_origin(
    tx: &mut Transaction,
    source: SourceId,
) -> Result<String, ydb::YdbOrCustomerError> {
    let mut row = tx
        .query_row("SELECT document FROM sources WHERE id=$id")
        .param("$id", source.as_uuid().to_string())
        .await?;
    let document: String = row.remove_field_by_name("document")?.try_into()?;
    let source: SourceDefinition = serde_json::from_str(&document)
        .map_err(|error| ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(error.to_string())))?;
    http_origin(source.url())
}
async fn work_origin(
    tx: &mut Transaction,
    item: &WorkItem,
) -> Result<String, ydb::YdbOrCustomerError> {
    match item {
        WorkItem::PollSource { source_id }
        | WorkItem::RefreshSource { source_id }
        | WorkItem::CollectWebFeed { source_id }
        | WorkItem::FanOut { source_id, .. } => source_origin(tx, *source_id).await,
        WorkItem::ExtractFullText { url, .. } => http_origin(url),
        WorkItem::CleanupContent { record_id, .. } => {
            let mut row = tx
                .query_row("SELECT document FROM source_records WHERE id=$id")
                .param("$id", record_id.as_uuid().to_string())
                .await?;
            let document: String = row.remove_field_by_name("document")?.try_into()?;
            let record: reader_ingest::SourceRecord =
                serde_json::from_str(&document).map_err(|error| {
                    ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(error.to_string()))
                })?;
            source_origin(tx, record.source_id()).await
        }
        WorkItem::EvaluateArticleRules { workspace_id, .. }
        | WorkItem::ApplyRule { workspace_id, .. } => {
            Ok(format!("internal:workspace:{}", workspace_id.as_uuid()))
        }
    }
}
async fn enqueue_work(
    tx: &mut Transaction,
    id: String,
    item: &WorkItem,
    first_attempt_ms: i64,
) -> Result<(), ydb::YdbOrCustomerError> {
    let origin = work_origin(tx, item).await?;
    let document = serde_json::to_string(item)
        .map_err(|error| ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(error.to_string())))?;
    if let Some(mut existing) = tx
        .query_row("SELECT item FROM ingest_jobs WHERE id=$id")
        .param("$id", id.clone())
        .optional()
        .await?
    {
        let existing_document: String = existing.remove_field_by_name("item")?.try_into()?;
        if existing_document == document {
            return Ok(());
        }
        return Err(ydb::YdbOrCustomerError::from_err(std::io::Error::other(
            "durable job identity collision",
        )));
    }
    tx.exec("UPSERT INTO ingest_jobs (id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision) VALUES ($id,'ready',0,$first,$origin,0,$item,0)").param("$id",id).param("$first",first_attempt_ms).param("$origin",origin).param("$item",document).await.map_err(Into::into)
}
async fn enqueue_subscription_backfill(
    tx: &mut Transaction,
    source_id: SourceId,
    subscription_id: &str,
    limit: usize,
) -> Result<(), ydb::YdbOrCustomerError> {
    if limit == 0 {
        return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(
            "subscription backfill limit must be positive".into(),
        )));
    }
    let rows=tx.query_result_set("SELECT document FROM source_record_identity VIEW recent_by_source WHERE source_id=$source ORDER BY observed_at_ms DESC LIMIT $limit").param("$source",source_id.as_uuid().to_string()).param("$limit",limit as u64).await?;
    let now = chrono::Utc::now().timestamp_millis();
    for mut row in rows {
        let document: String = row.remove_field_by_name("document")?.try_into()?;
        let record: reader_ingest::SourceRecord =
            serde_json::from_str(&document).map_err(|error| {
                ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(error.to_string()))
            })?;
        let item = WorkItem::FanOut {
            source_id,
            record_id: record.id(),
            after_subscription: None,
        };
        let identity = format!(
            "subscription-backfill/{subscription_id}/{}",
            record.id().as_uuid()
        );
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, identity.as_bytes()).to_string();
        enqueue_work(tx, id, &item, now).await?;
    }
    Ok(())
}
impl ProductionYdbTransport {
    pub async fn connect_from_environment(
        connection_string: &str,
        limits: YdbClientLimits,
    ) -> Result<Self, String> {
        let credentials = FromEnvCredentials::new().map_err(|e| e.to_string())?;
        let retry = RetrySettings::with_default_backoff()
            .with(MaximumAttempts(limits.retry_attempts))
            .with_deadline(limits.request_timeout);
        let client = ClientBuilder::new_from_connection_string(connection_string)
            .map_err(|e| e.to_string())?
            .with_credentials(credentials)
            .with_retry_settings(retry)
            .build()
            .await
            .map_err(|e| e.to_string())?
            .with_session_pool(
                SessionPoolSettings::new()
                    .with_limit(limits.max_concurrency)
                    .with_acquire_timeout(limits.request_timeout)
                    .with_session_create_timeout(limits.request_timeout)
                    .with_session_delete_timeout(limits.request_timeout),
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }
    pub async fn health(&self) -> Result<(), String> {
        let mut qc = self.client.query_client();
        let mut row = qc
            .query_row("SELECT 1 AS healthy")
            .await
            .map_err(|e| e.to_string())?;
        let _: i32 = row
            .remove_field_by_name("healthy")
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|e: ydb::YdbError| e.to_string())?;
        Ok(())
    }
    fn checked_table(table: &'static str) -> Result<&'static str, String> {
        match table {
            "schema_metadata"
            | "workspaces"
            | "subscriptions"
            | "articles"
            | "jobs"
            | "outbox"
            | "content_manifests"
            | "accounts"
            | "username_reservations"
            | "invites"
            | "sessions"
            | "password_resets"
            | "login_attempts"
            | "rules"
            | "web_feed_recipes"
            | "sources"
            | "source_records"
            | "delivery_origins"
            | "content_chunks"
            | "content_refresh_state"
            | "subscription_sources"
            | "source_health" => Ok(table),
            _ => Err("unknown table".into()),
        }
    }
}

#[async_trait]
pub trait YdbTransport: Send + Sync {
    async fn read(&self, table: &'static str, key: String) -> Result<Option<Vec<u8>>, String>;
    async fn scan_prefix(
        &self,
        table: &'static str,
        prefix: String,
    ) -> Result<Vec<Vec<u8>>, String>;
    async fn compare_and_swap(
        &self,
        table: &'static str,
        key: String,
        expected_revision: Option<u64>,
        revision: u64,
        document: Vec<u8>,
    ) -> Result<bool, String>;
    async fn delete(
        &self,
        table: &'static str,
        key: String,
        expected_revision: u64,
    ) -> Result<bool, String>;
    async fn atomic_create_account(
        &self,
        username: String,
        account_key: String,
        account_document: Vec<u8>,
    ) -> Result<bool, String>;
    async fn atomic_accept_invite(
        &self,
        invite_key: String,
        expected_invite_revision: u64,
        invite_revision: u64,
        invite_document: Vec<u8>,
        username: String,
        account_key: String,
        account_document: Vec<u8>,
        workspace_key: String,
        workspace_document: Vec<u8>,
    ) -> Result<bool, String>;
    async fn atomic_reset_password(
        &self,
        reset_key: String,
        expected_reset_revision: u64,
        reset_revision: u64,
        reset_document: Vec<u8>,
        account_key: String,
        expected_account_revision: u64,
        account_revision: u64,
        account_document: Vec<u8>,
    ) -> Result<bool, String>;
    async fn restore_workspace_with_refreshes(
        &self,
        workspace_id: String,
        expected_revision: u64,
        revision: u64,
        workspace_document: Vec<u8>,
        subscriptions: Vec<(String, String, String, Vec<u8>)>,
        backfill_limit: usize,
    ) -> Result<bool, String>;
    async fn execute_ddl(&self, statement: &'static str) -> Result<(), String>;
    async fn provision_subscription(
        &self,
        subscription_id: String,
        subscription_document: Vec<u8>,
        source_url: String,
        proposed_source_id: String,
        source_document: Vec<u8>,
        job_id: String,
        job_item: Vec<u8>,
        backfill_limit: usize,
        isolated_source: bool,
        recipe_document: Option<String>,
    ) -> Result<(), String>;
    async fn update_web_feed_recipe(
        &self,
        subscription_id: String,
        expected_version: u64,
        next_version: u64,
        recipe_document: String,
        source_document: Vec<u8>,
        job_id: String,
        job_item: Vec<u8>,
    ) -> Result<bool, String>;
    async fn web_feed_recipe(
        &self,
        subscription_id: String,
    ) -> Result<Option<(u64, String)>, String>;
    async fn atomic_cas_many(
        &self,
        table: &'static str,
        rows: Vec<(String, u64, u64, Vec<u8>)>,
    ) -> Result<bool, String>;
    async fn provision_subscriptions(
        &self,
        rows: Vec<(String, Vec<u8>, String, String, Vec<u8>, String)>,
        backfill_limit: usize,
    ) -> Result<(), String>;
    async fn presentation_metadata(
        &self,
        workspace: String,
        article: String,
    ) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>), String>;
    async fn enqueue_ingest_job(&self, id: String, item: Vec<u8>) -> Result<(), String>;
    async fn refresh_subscription(
        &self,
        subscription_id: String,
        source_url: String,
        id: String,
        item: Vec<u8>,
        backfill_limit: usize,
    ) -> Result<(), String>;
    async fn activate_subscription_with_refresh(
        &self,
        subscription_id: String,
        expected_revision: u64,
        revision: u64,
        subscription_document: Vec<u8>,
        source_url: String,
        job_id: String,
        job_item: Vec<u8>,
        backfill_limit: usize,
    ) -> Result<bool, String>;
    async fn source_id_for_subscription(
        &self,
        subscription_id: String,
    ) -> Result<Option<String>, String>;
    async fn subscription_stats(
        &self,
        workspace_id: String,
    ) -> Result<Vec<(String, usize, Option<i64>, bool, Option<String>, bool)>, String>;
    async fn apply_seed(
        &self,
        rows: Vec<(
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
        backfill_limit: usize,
    ) -> Result<(), String>;
    async fn ingest_job(
        &self,
        id: String,
    ) -> Result<Option<(String, String, Option<String>)>, String>;
    async fn ingest_jobs(&self) -> Result<Vec<(String, String, Option<String>)>, String>;
    async fn rule_evaluation_count(
        &self,
        workspace: String,
        rule: String,
        version: u64,
    ) -> Result<usize, String>;
}

#[async_trait]
impl YdbTransport for ProductionYdbTransport {
    async fn read(&self, table: &'static str, key: String) -> Result<Option<Vec<u8>>, String> {
        let table = Self::checked_table(table)?;
        let mut qc = self.client.query_client();
        let row = qc
            .query_row(format!("SELECT document FROM `{table}` WHERE id=$key"))
            .param("$key", key)
            .optional()
            .await
            .map_err(|e| e.to_string())?;
        match row {
            None => Ok(None),
            Some(mut row) => {
                let value: String = row
                    .remove_field_by_name("document")
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|e: ydb::YdbError| e.to_string())?;
                Ok(Some(value.into_bytes()))
            }
        }
    }
    async fn scan_prefix(
        &self,
        table: &'static str,
        prefix: String,
    ) -> Result<Vec<Vec<u8>>, String> {
        let table = Self::checked_table(table)?;
        let mut qc = self.client.query_client();
        let set = qc
            .query_result_set(format!(
                "SELECT document FROM `{table}` WHERE StartsWith(id,$prefix) ORDER BY id"
            ))
            .param("$prefix", prefix)
            .await
            .map_err(|e| e.to_string())?;
        set.into_iter()
            .map(|mut row| {
                let value: String = row
                    .remove_field_by_name("document")
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|e: ydb::YdbError| e.to_string())?;
                Ok(value.into_bytes())
            })
            .collect()
    }
    async fn compare_and_swap(
        &self,
        table: &'static str,
        key: String,
        expected_revision: Option<u64>,
        revision: u64,
        document: Vec<u8>,
    ) -> Result<bool, String> {
        let table = Self::checked_table(table)?;
        let document =
            String::from_utf8(document).map_err(|_| "document must be UTF-8 JSON".to_string())?;
        let qc = self.client.query_client();
        qc.retry_tx(ydb::closure!([key,document],async |tx:&mut Transaction|{let current=tx.query_row(format!("SELECT revision FROM `{table}` WHERE id=$key")).param("$key",key.clone()).optional().await?;let current=current.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;if current!=expected_revision{return Ok(false)}tx.exec(format!("UPSERT INTO `{table}` (id,revision,document) VALUES ($key,$revision,$document)")).param("$key",key.clone()).param("$revision",revision).param("$document",document.clone()).await?;Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn delete(
        &self,
        table: &'static str,
        key: String,
        expected_revision: u64,
    ) -> Result<bool, String> {
        let table = Self::checked_table(table)?;
        let qc = self.client.query_client();
        qc.retry_tx(ydb::closure!([key], async |tx: &mut Transaction| {
            let current = tx
                .query_row(format!("SELECT revision FROM `{table}` WHERE id=$key"))
                .param("$key", key.clone())
                .optional()
                .await?;
            let current = current
                .map(|mut row| -> ydb::YdbResult<u64> {
                    Ok(row.remove_field_by_name("revision")?.try_into()?)
                })
                .transpose()?;
            if current != Some(expected_revision) {
                return Ok(false);
            }
            tx.exec(format!("DELETE FROM `{table}` WHERE id=$key"))
                .param("$key", key.clone())
                .await?;
            Ok(true)
        }))
        .await
        .map_err(|e| e.to_string())
    }
    async fn atomic_create_account(
        &self,
        username: String,
        account_key: String,
        account_document: Vec<u8>,
    ) -> Result<bool, String> {
        let document = String::from_utf8(account_document)
            .map_err(|_| "account document is not UTF-8".to_owned())?;
        let qc = self.client.query_client();
        qc.retry_tx(ydb::closure!([username,account_key,document],async |tx:&mut Transaction|{if tx.query_row("SELECT id FROM `username_reservations` WHERE id=$key").param("$key",username.clone()).optional().await?.is_some(){return Ok(false)}tx.exec("UPSERT INTO `username_reservations` (id,revision,document) VALUES ($key,0,$document)").param("$key",username.clone()).param("$document",account_key.clone()).await?;tx.exec("UPSERT INTO `accounts` (id,revision,document) VALUES ($key,0,$document)").param("$key",account_key.clone()).param("$document",document.clone()).await?;Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn atomic_accept_invite(
        &self,
        invite_key: String,
        expected_invite_revision: u64,
        invite_revision: u64,
        invite_document: Vec<u8>,
        username: String,
        account_key: String,
        account_document: Vec<u8>,
        workspace_key: String,
        workspace_document: Vec<u8>,
    ) -> Result<bool, String> {
        let invite_document = String::from_utf8(invite_document)
            .map_err(|_| "invite document is not UTF-8".to_owned())?;
        let account_document = String::from_utf8(account_document)
            .map_err(|_| "account document is not UTF-8".to_owned())?;
        let workspace_document = String::from_utf8(workspace_document)
            .map_err(|_| "workspace document is not UTF-8".to_owned())?;
        let qc = self.client.query_client();
        qc.retry_tx(ydb::closure!([invite_key,invite_document,username,account_key,account_document,workspace_key,workspace_document],async |tx:&mut Transaction|{let invite=tx.query_row("SELECT revision FROM `invites` WHERE id=$key").param("$key",invite_key.clone()).optional().await?;let revision=invite.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;let username_exists=tx.query_row("SELECT id FROM `username_reservations` WHERE id=$key").param("$key",username.clone()).optional().await?.is_some();if revision!=Some(expected_invite_revision)||username_exists{return Ok(false)}tx.exec("UPSERT INTO `invites` (id,revision,document) VALUES ($key,$revision,$document)").param("$key",invite_key.clone()).param("$revision",invite_revision).param("$document",invite_document.clone()).await?;tx.exec("UPSERT INTO `username_reservations` (id,revision,document) VALUES ($key,0,$document)").param("$key",username.clone()).param("$document",account_key.clone()).await?;tx.exec("UPSERT INTO `accounts` (id,revision,document) VALUES ($key,0,$document)").param("$key",account_key.clone()).param("$document",account_document.clone()).await?;tx.exec("UPSERT INTO `workspaces` (id,revision,document) VALUES ($key,0,$document)").param("$key",workspace_key.clone()).param("$document",workspace_document.clone()).await?;Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn atomic_reset_password(
        &self,
        reset_key: String,
        expected_reset_revision: u64,
        reset_revision: u64,
        reset_document: Vec<u8>,
        account_key: String,
        expected_account_revision: u64,
        account_revision: u64,
        account_document: Vec<u8>,
    ) -> Result<bool, String> {
        let reset_document = String::from_utf8(reset_document)
            .map_err(|_| "reset document is not UTF-8".to_owned())?;
        let account_document = String::from_utf8(account_document)
            .map_err(|_| "account document is not UTF-8".to_owned())?;
        let qc = self.client.query_client();
        qc.retry_tx(ydb::closure!([reset_key,reset_document,account_key,account_document],async |tx:&mut Transaction|{let reset=tx.query_row("SELECT revision FROM `password_resets` WHERE id=$key").param("$key",reset_key.clone()).optional().await?;let reset=reset.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;let account=tx.query_row("SELECT revision FROM `accounts` WHERE id=$key").param("$key",account_key.clone()).optional().await?;let account=account.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;if reset!=Some(expected_reset_revision)||account!=Some(expected_account_revision){return Ok(false)}tx.exec("UPSERT INTO `password_resets` (id,revision,document) VALUES ($key,$revision,$document)").param("$key",reset_key.clone()).param("$revision",reset_revision).param("$document",reset_document.clone()).await?;tx.exec("UPSERT INTO `accounts` (id,revision,document) VALUES ($key,$revision,$document)").param("$key",account_key.clone()).param("$revision",account_revision).param("$document",account_document.clone()).await?;Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn restore_workspace_with_refreshes(
        &self,
        workspace_id: String,
        expected_revision: u64,
        revision: u64,
        workspace_document: Vec<u8>,
        subscriptions: Vec<(String, String, String, Vec<u8>)>,
        backfill_limit: usize,
    ) -> Result<bool, String> {
        let workspace_document = String::from_utf8(workspace_document)
            .map_err(|_| "workspace document must be UTF-8 JSON".to_owned())?;
        let mut rows = Vec::with_capacity(subscriptions.len());
        for (subscription_id, source_url, job_id, item) in subscriptions {
            rows.push((
                subscription_id,
                source_url,
                job_id,
                serde_json::from_slice::<WorkItem>(&item).map_err(|e| e.to_string())?,
            ));
        }
        self.client.query_client().retry_tx(ydb::closure!([workspace_id,workspace_document,rows],async |tx:&mut Transaction|{let current=tx.query_row("SELECT revision FROM workspaces WHERE id=$id").param("$id",workspace_id.clone()).optional().await?;let current=current.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;if current!=Some(expected_revision){return Ok(false)}tx.exec("UPSERT INTO workspaces (id,revision,document) VALUES ($id,$revision,$document)").param("$id",workspace_id.clone()).param("$revision",revision).param("$document",workspace_document.clone()).await?;for(subscription_id,_source_url,job_id,item)in rows.iter(){let mut row=tx.query_row("SELECT source_id FROM subscription_sources WHERE subscription_id=$id").param("$id",subscription_id.clone()).await?;let source:String=row.remove_field_by_name("source_id")?.try_into()?;let source=uuid::Uuid::parse_str(&source).map(SourceId::from_uuid).map_err(|_|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("stored source id is invalid".into())))?;enqueue_subscription_backfill(tx,source,subscription_id,backfill_limit).await?;enqueue_work(tx,job_id.clone(),item,chrono::Utc::now().timestamp_millis()).await?;}Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn execute_ddl(&self, statement: &'static str) -> Result<(), String> {
        let mut qc = self.client.query_client();
        qc.exec(statement).await.map_err(|e| e.to_string())
    }
    async fn provision_subscription(
        &self,
        subscription_id: String,
        subscription_document: Vec<u8>,
        source_url: String,
        proposed_source_id: String,
        source_document: Vec<u8>,
        job_id: String,
        job_item: Vec<u8>,
        backfill_limit: usize,
        isolated_source: bool,
        recipe_document: Option<String>,
    ) -> Result<(), String> {
        let subscription_document = String::from_utf8(subscription_document)
            .map_err(|_| "subscription document is not UTF-8".to_owned())?;
        let source_document = String::from_utf8(source_document)
            .map_err(|_| "source document is not UTF-8".to_owned())?;
        let job_item =
            String::from_utf8(job_item).map_err(|_| "job item is not UTF-8".to_owned())?;
        self.client.query_client().retry_tx(ydb::closure!([subscription_id,subscription_document,source_url,proposed_source_id,source_document,job_id,job_item,isolated_source,recipe_document],async |tx:&mut Transaction|{
            let existing=if *isolated_source{None}else{tx.query_row("SELECT source_id FROM source_urls WHERE url=$url").param("$url",source_url.clone()).optional().await?};
            let source_id=match existing{Some(mut row)=>{let value:String=row.remove_field_by_name("source_id")?.try_into()?;value},None=>{
                if !*isolated_source{tx.exec("UPSERT INTO source_urls (url,source_id) VALUES ($url,$source_id)").param("$url",source_url.clone()).param("$source_id",proposed_source_id.clone()).await?;}
                tx.exec("UPSERT INTO sources (id,revision,document) VALUES ($source_id,0,$document)").param("$source_id",proposed_source_id.clone()).param("$document",source_document.clone()).await?;
                proposed_source_id.clone()
            }};
            let duplicate=tx.query_row("SELECT id FROM subscriptions WHERE id=$id").param("$id",subscription_id.clone()).optional().await?;
            if duplicate.is_some(){return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("subscription already exists".into())))}
            tx.exec("UPSERT INTO subscriptions (id,revision,document) VALUES ($id,0,$document)").param("$id",subscription_id.clone()).param("$document",subscription_document.clone()).await?;
            tx.exec("UPSERT INTO subscription_sources (subscription_id,source_id) VALUES ($subscription_id,$source_id)").param("$subscription_id",subscription_id.clone()).param("$source_id",source_id.clone()).await?;
            if let Some(recipe)=recipe_document.clone(){tx.exec("UPSERT INTO web_feed_recipes (id,revision,document) VALUES ($id,0,$document)").param("$id",subscription_id.clone()).param("$document",recipe).await?;}
            let source_uuid=uuid::Uuid::parse_str(&source_id).map_err(|_|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("stored source id is invalid".into())))?;
            let mut item:WorkItem=serde_json::from_str(&job_item).map_err(|e|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(e.to_string())))?;
            match &mut item{WorkItem::PollSource{source_id}|WorkItem::RefreshSource{source_id}|WorkItem::CollectWebFeed{source_id}=>*source_id=SourceId::from_uuid(source_uuid),_=>return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("invalid subscription work item".into())))}
            enqueue_work(tx,job_id.clone(),&item,chrono::Utc::now().timestamp_millis()).await?;
            enqueue_subscription_backfill(tx,SourceId::from_uuid(source_uuid),&subscription_id,backfill_limit).await?;
            Ok::<(),ydb::YdbOrCustomerError>(())
        })).await.map_err(|e|e.to_string())
    }
    async fn update_web_feed_recipe(
        &self,
        subscription_id: String,
        expected_version: u64,
        next_version: u64,
        recipe_document: String,
        source_document: Vec<u8>,
        job_id: String,
        job_item: Vec<u8>,
    ) -> Result<bool, String> {
        let source_document = String::from_utf8(source_document)
            .map_err(|_| "source document is not UTF-8".to_owned())?;
        let item: WorkItem = serde_json::from_slice(&job_item).map_err(|e| e.to_string())?;
        self.client.query_client().retry_tx(ydb::closure!([subscription_id,recipe_document,source_document,job_id,item],async |tx:&mut Transaction|{let current=tx.query_row("SELECT revision FROM web_feed_recipes WHERE id=$id").param("$id",subscription_id.clone()).optional().await?;let current=current.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;if current!=Some(expected_version){return Ok(false)}let mut mapping=tx.query_row("SELECT source_id FROM subscription_sources WHERE subscription_id=$id").param("$id",subscription_id.clone()).await?;let source_id:String=mapping.remove_field_by_name("source_id")?.try_into()?;tx.exec("UPSERT INTO web_feed_recipes (id,revision,document) VALUES ($id,$revision,$document)").param("$id",subscription_id.clone()).param("$revision",next_version).param("$document",recipe_document.clone()).await?;tx.exec("UPSERT INTO sources (id,revision,document) VALUES ($id,$revision,$document)").param("$id",source_id).param("$revision",next_version).param("$document",source_document.clone()).await?;enqueue_work(tx,job_id.clone(),&item,chrono::Utc::now().timestamp_millis()).await?;Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn web_feed_recipe(
        &self,
        subscription_id: String,
    ) -> Result<Option<(u64, String)>, String> {
        let mut query = self.client.query_client();
        let row = query
            .query_row("SELECT revision,document FROM web_feed_recipes WHERE id=$id")
            .param("$id", subscription_id)
            .optional()
            .await
            .map_err(|e| e.to_string())?;
        row.map(|mut row| {
            let revision: u64 = row
                .remove_field_by_name("revision")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let document: String = row
                .remove_field_by_name("document")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            Ok((revision, document))
        })
        .transpose()
    }
    async fn atomic_cas_many(
        &self,
        table: &'static str,
        rows: Vec<(String, u64, u64, Vec<u8>)>,
    ) -> Result<bool, String> {
        let table = Self::checked_table(table)?;
        let mut converted = Vec::with_capacity(rows.len());
        for (key, expected, next, document) in rows {
            converted.push((
                key,
                expected,
                next,
                String::from_utf8(document)
                    .map_err(|_| "document must be UTF-8 JSON".to_owned())?,
            ));
        }
        self.client.query_client().retry_tx(ydb::closure!([converted],async |tx:&mut Transaction|{for(key,expected,_,_)in converted.iter(){let current=tx.query_row(format!("SELECT revision FROM `{table}` WHERE id=$key")).param("$key",key.clone()).optional().await?;let current=current.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;if current!=Some(*expected){return Ok(false)}}for(key,_,next,document)in converted.iter(){tx.exec(format!("UPSERT INTO `{table}` (id,revision,document) VALUES ($key,$revision,$document)")).param("$key",key.clone()).param("$revision",*next).param("$document",document.clone()).await?;}Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn provision_subscriptions(
        &self,
        rows: Vec<(String, Vec<u8>, String, String, Vec<u8>, String)>,
        backfill_limit: usize,
    ) -> Result<(), String> {
        let mut converted = Vec::with_capacity(rows.len());
        for (id, sub, url, source_id, source, job) in rows {
            converted.push((
                id,
                String::from_utf8(sub)
                    .map_err(|_| "subscription document is not UTF-8".to_owned())?,
                url,
                source_id,
                String::from_utf8(source).map_err(|_| "source document is not UTF-8".to_owned())?,
                job,
            ));
        }
        self.client.query_client().retry_tx(ydb::closure!([converted],async |tx:&mut Transaction|{for(id,_,_,_,_,_)in converted.iter(){if tx.query_row("SELECT id FROM subscriptions WHERE id=$id").param("$id",id.clone()).optional().await?.is_some(){return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("subscription already exists".into())))}}for(id,sub,url,proposed_source_id,source,job)in converted.iter(){let existing=tx.query_row("SELECT source_id FROM source_urls WHERE url=$url").param("$url",url.clone()).optional().await?;let source_id=match existing{Some(mut row)=>{let value:String=row.remove_field_by_name("source_id")?.try_into()?;value},None=>{tx.exec("UPSERT INTO source_urls (url,source_id) VALUES ($url,$source_id)").param("$url",url.clone()).param("$source_id",proposed_source_id.clone()).await?;tx.exec("UPSERT INTO sources (id,revision,document) VALUES ($source_id,0,$document)").param("$source_id",proposed_source_id.clone()).param("$document",source.clone()).await?;proposed_source_id.clone()}};tx.exec("UPSERT INTO subscriptions (id,revision,document) VALUES ($id,0,$document)").param("$id",id.clone()).param("$document",sub.clone()).await?;tx.exec("UPSERT INTO subscription_sources (subscription_id,source_id) VALUES ($subscription_id,$source_id)").param("$subscription_id",id.clone()).param("$source_id",source_id.clone()).await?;let source_uuid=uuid::Uuid::parse_str(&source_id).map_err(|_|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("stored source id is invalid".into())))?;let source_id=SourceId::from_uuid(source_uuid);let item=WorkItem::PollSource{source_id};enqueue_work(tx,job.clone(),&item,chrono::Utc::now().timestamp_millis()).await?;enqueue_subscription_backfill(tx,source_id,id,backfill_limit).await?;}Ok::<(),ydb::YdbOrCustomerError>(())})).await.map_err(|e|e.to_string())
    }
    async fn presentation_metadata(
        &self,
        workspace: String,
        article: String,
    ) -> Result<(Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>), String> {
        let mut query = self.client.query_client();
        let rows=query.query_result_set("SELECT subscription_id,source_record_id FROM library_origins WHERE workspace_id=$workspace AND article_id=$article").param("$workspace",workspace).param("$article",article).await.map_err(|e|e.to_string())?;
        let mut subscriptions = Vec::new();
        let mut manifests = Vec::new();
        let mut failures = Vec::new();
        for mut row in rows {
            let subscription: String = row
                .remove_field_by_name("subscription_id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let record: String = row
                .remove_field_by_name("source_record_id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            if let Some(value) = self.read("subscriptions", subscription).await? {
                subscriptions.push(value)
            }
            if let Some(value) = self.read("content_manifests", record.clone()).await? {
                let pointer: reader_ingest::ContentManifestPointer =
                    serde_json::from_slice(&value).map_err(|e| e.to_string())?;
                let rows=query.query_result_set("SELECT ordinal,bytes FROM staged_content_chunks WHERE record_id=$record AND refresh_id=$refresh AND representation='safe' ORDER BY ordinal").param("$record",record.clone()).param("$refresh",pointer.refresh_id.to_string()).await.map_err(|e|e.to_string())?;
                let mut chunks = Vec::new();
                for mut chunk in rows {
                    let ordinal: u32 = chunk
                        .remove_field_by_name("ordinal")
                        .map_err(|e| e.to_string())?
                        .try_into()
                        .map_err(|e: ydb::YdbError| e.to_string())?;
                    let encoded: String = chunk
                        .remove_field_by_name("bytes")
                        .map_err(|e| e.to_string())?
                        .try_into()
                        .map_err(|e: ydb::YdbError| e.to_string())?;
                    let bytes = serde_json::from_str(&encoded).map_err(|e| e.to_string())?;
                    chunks.push(reader_ingest::ContentChunk { ordinal, bytes })
                }
                if chunks.len() != pointer.safe_html_chunks as usize
                    || chunks
                        .iter()
                        .enumerate()
                        .any(|(expected, chunk)| chunk.ordinal != expected as u32)
                {
                    return Err(
                        "current content manifest references incomplete safe HTML chunks".into(),
                    );
                }
                let revision = reader_ingest::ContentRevision {
                    record_id: pointer.record_id,
                    source_revision: pointer.source_revision,
                    refresh_id: pointer.refresh_id,
                    raw_chunks: Vec::new(),
                    safe_html_chunks: chunks,
                    fetched_at: pointer.fetched_at,
                    final_url: pointer.final_url,
                };
                manifests.push(serde_json::to_vec(&revision).map_err(|e| e.to_string())?)
            }
            if let Some(value) = self
                .read("content_refresh_state", format!("failure/{record}"))
                .await?
            {
                failures.push(value)
            }
        }
        Ok((subscriptions, manifests, failures))
    }
    async fn enqueue_ingest_job(&self, id: String, item: Vec<u8>) -> Result<(), String> {
        let item: WorkItem = serde_json::from_slice(&item).map_err(|e| e.to_string())?;
        self.client
            .query_client()
            .retry_tx(ydb::closure!([id, item], async |tx: &mut Transaction| {
                enqueue_work(tx, id.clone(), &item, chrono::Utc::now().timestamp_millis()).await
            }))
            .await
            .map_err(|e| e.to_string())
    }
    async fn refresh_subscription(
        &self,
        subscription_id: String,
        _source_url: String,
        id: String,
        item: Vec<u8>,
        backfill_limit: usize,
    ) -> Result<(), String> {
        let item: WorkItem = serde_json::from_slice(&item).map_err(|e| e.to_string())?;
        self.client
            .query_client()
            .retry_tx(ydb::closure!(
                [subscription_id, id, item],
                async |tx: &mut Transaction| {
                    let mut row = tx
                        .query_row(
                            "SELECT source_id FROM subscription_sources WHERE subscription_id=$id",
                        )
                        .param("$id", subscription_id.clone())
                        .await?;
                    let source: String = row.remove_field_by_name("source_id")?.try_into()?;
                    let source = uuid::Uuid::parse_str(&source)
                        .map(SourceId::from_uuid)
                        .map_err(|_| {
                            ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(
                                "stored source id is invalid".into(),
                            ))
                        })?;
                    enqueue_subscription_backfill(tx, source, &subscription_id, backfill_limit)
                        .await?;
                    enqueue_work(tx, id.clone(), &item, chrono::Utc::now().timestamp_millis())
                        .await?;
                    Ok::<(), ydb::YdbOrCustomerError>(())
                }
            ))
            .await
            .map_err(|e| e.to_string())
    }
    async fn activate_subscription_with_refresh(
        &self,
        subscription_id: String,
        expected_revision: u64,
        revision: u64,
        subscription_document: Vec<u8>,
        source_url: String,
        job_id: String,
        job_item: Vec<u8>,
        backfill_limit: usize,
    ) -> Result<bool, String> {
        let subscription_document = String::from_utf8(subscription_document)
            .map_err(|_| "subscription document must be UTF-8 JSON".to_owned())?;
        let item: WorkItem = serde_json::from_slice(&job_item).map_err(|e| e.to_string())?;
        self.client.query_client().retry_tx(ydb::closure!([subscription_id,subscription_document,source_url,job_id,item],async |tx:&mut Transaction|{let current=tx.query_row("SELECT revision FROM subscriptions WHERE id=$id").param("$id",subscription_id.clone()).optional().await?;let current=current.map(|mut row|->ydb::YdbResult<u64>{Ok(row.remove_field_by_name("revision")?.try_into()?)}).transpose()?;if current!=Some(expected_revision){return Ok(false)}let mut row=tx.query_row("SELECT source_id FROM subscription_sources WHERE subscription_id=$id").param("$id",subscription_id.clone()).await?;let source:String=row.remove_field_by_name("source_id")?.try_into()?;let source=uuid::Uuid::parse_str(&source).map(SourceId::from_uuid).map_err(|_|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("stored source id is invalid".into())))?;tx.exec("UPSERT INTO subscriptions (id,revision,document) VALUES ($id,$revision,$document)").param("$id",subscription_id.clone()).param("$revision",revision).param("$document",subscription_document.clone()).await?;enqueue_subscription_backfill(tx,source,&subscription_id,backfill_limit).await?;enqueue_work(tx,job_id.clone(),&item,chrono::Utc::now().timestamp_millis()).await?;Ok(true)})).await.map_err(|e|e.to_string())
    }
    async fn source_id_for_subscription(
        &self,
        subscription_id: String,
    ) -> Result<Option<String>, String> {
        let mut query = self.client.query_client();
        let row = query
            .query_row("SELECT source_id FROM subscription_sources WHERE subscription_id=$id")
            .param("$id", subscription_id)
            .optional()
            .await
            .map_err(|e| e.to_string())?;
        row.map(|mut row| {
            row.remove_field_by_name("source_id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())
        })
        .transpose()
    }
    async fn subscription_stats(
        &self,
        workspace_id: String,
    ) -> Result<Vec<(String, usize, Option<i64>, bool, Option<String>, bool)>, String> {
        let mut query = self.client.query_client();
        let mut counts = std::collections::HashMap::new();
        for mut row in query.query_result_set("SELECT subscription_id,COUNT(*) AS total FROM library_origins WHERE workspace_id=$workspace GROUP BY subscription_id").param("$workspace",workspace_id).await.map_err(|e|e.to_string())?{let id:String=row.remove_field_by_name("subscription_id").map_err(|e|e.to_string())?.try_into().map_err(|e:ydb::YdbError|e.to_string())?;let total:u64=row.remove_field_by_name("total").map_err(|e|e.to_string())?.try_into().map_err(|e:ydb::YdbError|e.to_string())?;counts.insert(id,total as usize);}
        let mut health = std::collections::HashMap::new();
        for mut row in query
            .query_result_set("SELECT source_id,document FROM source_health")
            .await
            .map_err(|e| e.to_string())?
        {
            let id: String = row
                .remove_field_by_name("source_id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let document: String = row
                .remove_field_by_name("document")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            health.insert(
                id,
                serde_json::from_str::<ingest_store::SourceHealth>(&document)
                    .map_err(|e| e.to_string())?,
            );
        }
        let mut editable = std::collections::HashSet::new();
        for mut row in query
            .query_result_set("SELECT id FROM web_feed_recipes")
            .await
            .map_err(|e| e.to_string())?
        {
            let id: String = row
                .remove_field_by_name("id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            editable.insert(id);
        }
        let mut result = Vec::new();
        for mut row in query
            .query_result_set("SELECT subscription_id,source_id FROM subscription_sources")
            .await
            .map_err(|e| e.to_string())?
        {
            let subscription: String = row
                .remove_field_by_name("subscription_id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let source: String = row
                .remove_field_by_name("source_id")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let state = health.get(&source);
            let count = counts.get(&subscription).copied().unwrap_or(0);
            let is_editable = editable.contains(&subscription);
            result.push((
                subscription,
                count,
                state.and_then(|v| v.last_success_ms),
                state.is_some_and(|v| v.incomplete),
                state.and_then(|v| v.error.clone()),
                is_editable,
            ))
        }
        Ok(result)
    }
    async fn apply_seed(
        &self,
        rows: Vec<(
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
        backfill_limit: usize,
    ) -> Result<(), String> {
        let mut converted = Vec::with_capacity(rows.len());
        for (
            id,
            sub,
            url,
            source_id,
            source,
            job,
            item,
            seed_key,
            seed_document,
            recipe_document,
        ) in rows
        {
            let recipe_document = recipe_document
                .map(String::from_utf8)
                .transpose()
                .map_err(|_| "recipe document is not UTF-8".to_owned())?;
            converted.push((
                id,
                String::from_utf8(sub)
                    .map_err(|_| "subscription document is not UTF-8".to_owned())?,
                url,
                source_id,
                String::from_utf8(source).map_err(|_| "source document is not UTF-8".to_owned())?,
                job,
                String::from_utf8(item).map_err(|_| "job item is not UTF-8".to_owned())?,
                seed_key,
                String::from_utf8(seed_document)
                    .map_err(|_| "seed document is not UTF-8".to_owned())?,
                recipe_document,
            ));
        }
        self.client.query_client().retry_tx(ydb::closure!([converted],async |tx:&mut Transaction|{
            for(id,sub,_,_,_,_,_,seed_key,seed_document,_)in converted.iter(){
                if let Some(mut row)=tx.query_row("SELECT document FROM subscriptions WHERE id=$id").param("$id",id.clone()).optional().await?{
                    let existing:String=row.remove_field_by_name("document")?.try_into()?;
                    if existing!=*sub{return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("seed idempotency key conflicts with existing subscription".into())))}
                }
                if let Some(mut row)=tx.query_row("SELECT document FROM seed_items WHERE id=$id").param("$id",seed_key.clone()).optional().await?{
                    let existing:String=row.remove_field_by_name("document")?.try_into()?;
                    if existing!=*seed_document{return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("seed configuration conflicts with applied idempotency key".into())))}
                }
            }
            for(id,sub,url,proposed_source_id,source,job,item,seed_key,seed_document,recipe_document)in converted.iter(){
                if tx.query_row("SELECT id FROM subscriptions WHERE id=$id").param("$id",id.clone()).optional().await?.is_some(){continue}
                let existing=if recipe_document.is_some(){None}else{tx.query_row("SELECT source_id FROM source_urls WHERE url=$url").param("$url",url.clone()).optional().await?};
                let actual_source=if let Some(mut row)=existing{row.remove_field_by_name("source_id")?.try_into()?}else{
                    if recipe_document.is_none(){tx.exec("UPSERT INTO source_urls (url,source_id) VALUES ($url,$source_id)").param("$url",url.clone()).param("$source_id",proposed_source_id.clone()).await?;}
                    tx.exec("UPSERT INTO sources (id,revision,document) VALUES ($source_id,0,$document)").param("$source_id",proposed_source_id.clone()).param("$document",source.clone()).await?;
                    proposed_source_id.clone()
                };
                let actual_uuid=uuid::Uuid::parse_str(&actual_source).map_err(|e|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(e.to_string())))?;
                let mut work:WorkItem=serde_json::from_str(item).map_err(|e|ydb::YdbOrCustomerError::from(ydb::YdbError::Custom(e.to_string())))?;
                match &mut work{WorkItem::PollSource{source_id}|WorkItem::RefreshSource{source_id}|WorkItem::CollectWebFeed{source_id}=>*source_id=SourceId::from_uuid(actual_uuid),_=>return Err(ydb::YdbOrCustomerError::from(ydb::YdbError::Custom("invalid seed work item".into())))}
                tx.exec("UPSERT INTO subscriptions (id,revision,document) VALUES ($id,0,$document)").param("$id",id.clone()).param("$document",sub.clone()).await?;
                tx.exec("UPSERT INTO subscription_sources (subscription_id,source_id) VALUES ($subscription_id,$source_id)").param("$subscription_id",id.clone()).param("$source_id",actual_source.clone()).await?;
                if let Some(recipe)=recipe_document{tx.exec("UPSERT INTO web_feed_recipes (id,revision,document) VALUES ($id,0,$document)").param("$id",id.clone()).param("$document",recipe.clone()).await?;}
                enqueue_work(tx,job.clone(),&work,chrono::Utc::now().timestamp_millis()).await?;
                enqueue_subscription_backfill(tx,SourceId::from_uuid(actual_uuid),id,backfill_limit).await?;
                tx.exec("UPSERT INTO seed_items (id,revision,document) VALUES ($id,0,$document)").param("$id",seed_key.clone()).param("$document",seed_document.clone()).await?;
            }
            Ok::<(),ydb::YdbOrCustomerError>(())
        })).await.map_err(|e|e.to_string())
    }
    async fn ingest_job(
        &self,
        id: String,
    ) -> Result<Option<(String, String, Option<String>)>, String> {
        let mut query = self.client.query_client();
        let row = query
            .query_row("SELECT status,item,diagnostic FROM ingest_jobs WHERE id=$id")
            .param("$id", id)
            .optional()
            .await
            .map_err(|e| e.to_string())?;
        row.map(|mut row| {
            let status: String = row
                .remove_field_by_name("status")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let item: String = row
                .remove_field_by_name("item")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            let diagnostic: Option<String> = row
                .remove_field_by_name("diagnostic")
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|e: ydb::YdbError| e.to_string())?;
            Ok((status, item, diagnostic))
        })
        .transpose()
    }
    async fn ingest_jobs(&self) -> Result<Vec<(String, String, Option<String>)>, String> {
        let mut query = self.client.query_client();
        let rows = query
            .query_result_set("SELECT status,item,diagnostic FROM ingest_jobs")
            .await
            .map_err(|e| e.to_string())?;
        rows.into_iter()
            .map(|mut row| {
                let status: String = row
                    .remove_field_by_name("status")
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|e: ydb::YdbError| e.to_string())?;
                let item: String = row
                    .remove_field_by_name("item")
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|e: ydb::YdbError| e.to_string())?;
                let diagnostic: Option<String> = row
                    .remove_field_by_name("diagnostic")
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|e: ydb::YdbError| e.to_string())?;
                Ok((status, item, diagnostic))
            })
            .collect()
    }
    async fn rule_evaluation_count(
        &self,
        workspace: String,
        rule: String,
        version: u64,
    ) -> Result<usize, String> {
        let mut query = self.client.query_client();
        let rows=query.query_result_set("SELECT article_id FROM rule_evaluations WHERE workspace_id=$workspace AND rule_id=$rule AND rule_version=$version").param("$workspace",workspace).param("$rule",rule).param("$version",version).await.map_err(|e|e.to_string())?;
        Ok(rows.into_iter().count())
    }
}

pub async fn prepare_schema(transport: &impl YdbTransport) -> Result<(), RepositoryError> {
    for statement in SCHEMA_DDL {
        transport
            .execute_ddl(statement)
            .await
            .map_err(RepositoryError::Storage)?;
    }
    let key = "schema".to_owned();
    let current = transport
        .read("schema_metadata", key.clone())
        .await
        .map_err(RepositoryError::Storage)?;
    let expected = current
        .as_deref()
        .map(decode::<SchemaMarker>)
        .transpose()?
        .map(|v| v.version);
    if expected == Some(SCHEMA_VERSION) {
        return Ok(());
    }
    let marker = encode(&SchemaMarker {
        version: SCHEMA_VERSION,
    })?;
    if transport
        .compare_and_swap("schema_metadata", key, expected, SCHEMA_VERSION, marker)
        .await
        .map_err(RepositoryError::Storage)?
    {
        Ok(())
    } else {
        Err(RepositoryError::Conflict)
    }
}

pub struct YdbRepository<T> {
    transport: Arc<T>,
    reason_policy: ReasonPolicy,
    initial_scope: usize,
}
impl<T> YdbRepository<T> {
    pub fn new(
        transport: Arc<T>,
        reason_policy: ReasonPolicy,
        initial_scope: usize,
    ) -> Result<Self, RepositoryError> {
        if initial_scope == 0 {
            return Err(RepositoryError::Storage(
                "initial source scope must be positive".into(),
            ));
        }
        Ok(Self {
            transport,
            reason_policy,
            initial_scope,
        })
    }
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, RepositoryError> {
    serde_json::to_vec(value).map_err(|e| RepositoryError::Storage(e.to_string()))
}
fn decode<T: DeserializeOwned>(raw: &[u8]) -> Result<T, RepositoryError> {
    serde_json::from_slice(raw).map_err(|e| RepositoryError::Storage(e.to_string()))
}

#[async_trait]
impl<T: YdbTransport> ReaderRepository for YdbRepository<T> {
    async fn readiness(&self) -> Result<(), RepositoryError> {
        let marker: SchemaMarker = read_one(
            self.transport.as_ref(),
            "schema_metadata",
            "schema".to_owned(),
        )
        .await?;
        if marker.version == SCHEMA_VERSION {
            Ok(())
        } else {
            Err(RepositoryError::Storage(format!(
                "schema version {} does not match required {SCHEMA_VERSION}",
                marker.version
            )))
        }
    }
    async fn save_job(
        &self,
        expected: Option<u64>,
        value: DurableJob,
    ) -> Result<(), RepositoryError> {
        cas(
            self.transport.as_ref(),
            "jobs",
            value.id.to_string(),
            expected,
            value.revision,
            &value,
        )
        .await
    }
    async fn account(
        &self,
        id: AccountId,
    ) -> Result<reader_application::AccountRecord, RepositoryError> {
        read_one(
            self.transport.as_ref(),
            "accounts",
            id.as_uuid().to_string(),
        )
        .await
    }
    async fn account_by_username(
        &self,
        username: &str,
    ) -> Result<reader_application::AccountRecord, RepositoryError> {
        let raw = self
            .transport
            .read("username_reservations", username.to_owned())
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)?;
        let id = String::from_utf8(raw).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let id = uuid::Uuid::parse_str(&id).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        self.account(AccountId::from_uuid(id)).await
    }
    async fn save_account(
        &self,
        expected: Option<u64>,
        value: reader_application::AccountRecord,
    ) -> Result<(), RepositoryError> {
        if expected.is_none() {
            let ok = self
                .transport
                .atomic_create_account(
                    value.username.clone(),
                    value.id.as_uuid().to_string(),
                    encode(&value)?,
                )
                .await
                .map_err(RepositoryError::Storage)?;
            if ok {
                Ok(())
            } else {
                Err(RepositoryError::Conflict)
            }
        } else {
            cas(
                self.transport.as_ref(),
                "accounts",
                value.id.as_uuid().to_string(),
                expected,
                value.revision,
                &value,
            )
            .await
        }
    }
    async fn invite_by_token_hash(
        &self,
        hash: &str,
    ) -> Result<reader_application::InviteRecord, RepositoryError> {
        read_one(self.transport.as_ref(), "invites", hash.to_owned()).await
    }
    async fn save_invite(
        &self,
        expected: Option<u64>,
        value: reader_application::InviteRecord,
    ) -> Result<(), RepositoryError> {
        cas(
            self.transport.as_ref(),
            "invites",
            value.token_hash.clone(),
            expected,
            value.revision,
            &value,
        )
        .await
    }
    async fn session_by_verifier_hash(
        &self,
        hash: &str,
    ) -> Result<reader_application::SessionRecord, RepositoryError> {
        read_one(self.transport.as_ref(), "sessions", hash.to_owned()).await
    }
    async fn save_session(
        &self,
        expected: Option<u64>,
        value: reader_application::SessionRecord,
    ) -> Result<(), RepositoryError> {
        cas(
            self.transport.as_ref(),
            "sessions",
            value.verifier_hash.clone(),
            expected,
            value.revision,
            &value,
        )
        .await
    }
    async fn delete_session(&self, hash: &str, expected: u64) -> Result<(), RepositoryError> {
        if self
            .transport
            .delete("sessions", hash.to_owned(), expected)
            .await
            .map_err(RepositoryError::Storage)?
        {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn password_reset_by_token_hash(
        &self,
        hash: &str,
    ) -> Result<reader_application::PasswordResetRecord, RepositoryError> {
        read_one(self.transport.as_ref(), "password_resets", hash.to_owned()).await
    }
    async fn save_password_reset(
        &self,
        expected: Option<u64>,
        value: reader_application::PasswordResetRecord,
    ) -> Result<(), RepositoryError> {
        cas(
            self.transport.as_ref(),
            "password_resets",
            value.token_hash.clone(),
            expected,
            value.revision,
            &value,
        )
        .await
    }
    async fn consume_invite_create_account_and_workspace(
        &self,
        expected: u64,
        invite: reader_application::InviteRecord,
        account: reader_application::AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError> {
        workspace
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let ok = self
            .transport
            .atomic_accept_invite(
                invite.token_hash.clone(),
                expected,
                invite.revision,
                encode(&invite)?,
                account.username.clone(),
                account.id.as_uuid().to_string(),
                encode(&account)?,
                workspace.id().as_uuid().to_string(),
                encode(&workspace)?,
            )
            .await
            .map_err(RepositoryError::Storage)?;
        if ok {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn consume_reset_and_update_account(
        &self,
        expected_reset: u64,
        reset: reader_application::PasswordResetRecord,
        expected_account: u64,
        account: reader_application::AccountRecord,
    ) -> Result<(), RepositoryError> {
        let ok = self
            .transport
            .atomic_reset_password(
                reset.token_hash.clone(),
                expected_reset,
                reset.revision,
                encode(&reset)?,
                account.id.as_uuid().to_string(),
                expected_account,
                account.revision,
                encode(&account)?,
            )
            .await
            .map_err(RepositoryError::Storage)?;
        if ok {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn workspace(&self, id: WorkspaceId) -> Result<Workspace, RepositoryError> {
        let value: Workspace = read_one(
            self.transport.as_ref(),
            "workspaces",
            id.as_uuid().to_string(),
        )
        .await?;
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(value)
    }
    async fn workspaces_by_owner(
        &self,
        owner: AccountId,
    ) -> Result<Vec<Workspace>, RepositoryError> {
        let values: Vec<Workspace> = scan(self.transport.as_ref(), "workspaces", "").await?;
        for value in &values {
            value
                .validate(self.reason_policy)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        }
        Ok(values.into_iter().filter(|v| v.owner() == owner).collect())
    }
    async fn save_workspace(
        &self,
        expected: Option<u64>,
        value: Workspace,
    ) -> Result<(), RepositoryError> {
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        cas(
            self.transport.as_ref(),
            "workspaces",
            value.id().as_uuid().to_string(),
            expected,
            value.revision(),
            &value,
        )
        .await
    }
    async fn restore_workspace_with_refreshes(
        &self,
        expected: u64,
        value: Workspace,
        active: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        if !value.accepts_delivery() {
            return Err(RepositoryError::Storage(
                "workspace catch-up requires an active workspace".into(),
            ));
        }
        let mut rows = Vec::with_capacity(active.len());
        for subscription in active {
            if subscription.workspace_id() != value.id()
                || !matches!(subscription.status(), SubscriptionStatus::Active)
            {
                return Err(RepositoryError::Storage(
                    "workspace catch-up contains an invalid subscription".into(),
                ));
            }
            let source = self
                .transport
                .source_id_for_subscription(subscription.id().as_uuid().to_string())
                .await
                .map_err(RepositoryError::Storage)?
                .ok_or(RepositoryError::NotFound)?;
            let source = uuid::Uuid::parse_str(&source)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let item = WorkItem::RefreshSource {
                source_id: SourceId::from_uuid(source),
            };
            let identity = format!(
                "workspace-restore/{}/{}/{}",
                value.id().as_uuid(),
                value.revision(),
                subscription.id().as_uuid()
            );
            let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, identity.as_bytes());
            rows.push((
                subscription.id().as_uuid().to_string(),
                subscription.source_url().as_str().to_owned(),
                id.to_string(),
                encode(&item)?,
            ));
        }
        let ok = self
            .transport
            .restore_workspace_with_refreshes(
                value.id().as_uuid().to_string(),
                expected,
                value.revision(),
                encode(&value)?,
                rows,
                self.initial_scope,
            )
            .await
            .map_err(RepositoryError::Storage)?;
        if ok {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn subscription(&self, id: SubscriptionId) -> Result<Subscription, RepositoryError> {
        let value: Subscription = read_one(
            self.transport.as_ref(),
            "subscriptions",
            id.as_uuid().to_string(),
        )
        .await?;
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(value)
    }
    async fn subscriptions_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Subscription>, RepositoryError> {
        let values: Vec<Subscription> = scan(self.transport.as_ref(), "subscriptions", "").await?;
        for value in &values {
            value
                .validate(self.reason_policy)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        }
        Ok(values
            .into_iter()
            .filter(|v| v.workspace_id() == workspace)
            .collect())
    }
    async fn subscription_stats(
        &self,
        workspace: WorkspaceId,
        subscriptions: &[Subscription],
    ) -> Result<
        std::collections::HashMap<SubscriptionId, reader_application::SubscriptionStats>,
        RepositoryError,
    > {
        let wanted = subscriptions
            .iter()
            .map(|v| (v.id().as_uuid().to_string(), v.id()))
            .collect::<std::collections::HashMap<_, _>>();
        let mut result = std::collections::HashMap::new();
        for (id, count, last_success_ms, incomplete, error, editable_web_feed) in self
            .transport
            .subscription_stats(workspace.as_uuid().to_string())
            .await
            .map_err(RepositoryError::Storage)?
        {
            let Some(subscription) = wanted.get(&id).copied() else {
                continue;
            };
            result.insert(
                subscription,
                reader_application::SubscriptionStats {
                    article_count: count,
                    last_success_at: last_success_ms
                        .and_then(chrono::DateTime::from_timestamp_millis),
                    incomplete,
                    continuation: incomplete.then(|| "A durable continuation is queued".to_owned()),
                    error,
                    editable_web_feed,
                },
            );
        }
        for subscription in subscriptions {
            result.entry(subscription.id()).or_default();
        }
        Ok(result)
    }
    async fn save_subscription(
        &self,
        expected: Option<u64>,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        if expected.is_none() {
            let source = SourceDefinition::new(
                SourceId::new(),
                value.source_url().clone(),
                SourceKind::Auto,
            )
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let item = WorkItem::PollSource {
                source_id: source.id(),
            };
            return self
                .transport
                .provision_subscription(
                    value.id().as_uuid().to_string(),
                    encode(&value)?,
                    value.source_url().as_str().to_owned(),
                    source.id().as_uuid().to_string(),
                    encode(&source)?,
                    JobId::new().as_uuid().to_string(),
                    encode(&item)?,
                    self.initial_scope,
                    false,
                    None,
                )
                .await
                .map_err(RepositoryError::Storage);
        }
        cas(
            self.transport.as_ref(),
            "subscriptions",
            value.id().as_uuid().to_string(),
            expected,
            value.revision(),
            &value,
        )
        .await
    }
    async fn activate_subscription_with_refresh(
        &self,
        expected: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        if !matches!(value.status(), SubscriptionStatus::Active) {
            return Err(RepositoryError::Storage(
                "catch-up activation requires an active subscription".into(),
            ));
        }
        let source = self
            .transport
            .source_id_for_subscription(value.id().as_uuid().to_string())
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)?;
        let source =
            uuid::Uuid::parse_str(&source).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let item = WorkItem::RefreshSource {
            source_id: SourceId::from_uuid(source),
        };
        let identity = format!(
            "subscription-activation/{}/{}",
            value.id().as_uuid(),
            value.revision()
        );
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, identity.as_bytes());
        let ok = self
            .transport
            .activate_subscription_with_refresh(
                value.id().as_uuid().to_string(),
                expected,
                value.revision(),
                encode(&value)?,
                value.source_url().as_str().to_owned(),
                id.to_string(),
                encode(&item)?,
                self.initial_scope,
            )
            .await
            .map_err(RepositoryError::Storage)?;
        if ok {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn enqueue_subscription_refresh(
        &self,
        value: &Subscription,
    ) -> Result<(), RepositoryError> {
        let source = self
            .transport
            .source_id_for_subscription(value.id().as_uuid().to_string())
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)?;
        let source =
            uuid::Uuid::parse_str(&source).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let item = WorkItem::RefreshSource {
            source_id: SourceId::from_uuid(source),
        };
        let identity = format!("manual-poll/{}/{}", value.id().as_uuid(), value.revision());
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, identity.as_bytes());
        self.transport
            .refresh_subscription(
                value.id().as_uuid().to_string(),
                value.source_url().as_str().to_owned(),
                id.to_string(),
                encode(&item)?,
                self.initial_scope,
            )
            .await
            .map_err(RepositoryError::Storage)
    }
    async fn save_web_feed_subscription(
        &self,
        value: Subscription,
        recipe_json: String,
    ) -> Result<(), RepositoryError> {
        value
            .validate(self.reason_policy)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let raw: StoredWebRecipe = serde_json::from_str(&recipe_json)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        if raw.workspace_id != value.workspace_id().as_uuid()
            || raw.url != value.source_url().as_str()
        {
            return Err(RepositoryError::Storage(
                "web feed recipe scope or URL does not match subscription".into(),
            ));
        }
        let loading = match raw.loading.as_str() {
            "automatic" => reader_ingest::WebLoading::Automatic,
            "static" => reader_ingest::WebLoading::Static,
            "browser" => reader_ingest::WebLoading::Browser,
            _ => {
                return Err(RepositoryError::Storage(
                    "invalid web loading strategy".into(),
                ))
            }
        };
        let language = match raw.selector_language.as_deref().unwrap_or("css") {
            "css" => reader_ingest::SelectorLanguage::Css,
            "xpath" => reader_ingest::SelectorLanguage::XPath,
            _ => return Err(RepositoryError::Storage("invalid selector language".into())),
        };
        let primary = reader_ingest::WebSelector::new(language, raw.selector)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let viewport = match raw.viewport.as_deref().unwrap_or("desktop") {
            "desktop" => reader_ingest::WebViewport::Desktop,
            "mobile" => reader_ingest::WebViewport::Mobile,
            _ => return Err(RepositoryError::Storage("invalid viewport".into())),
        };
        let overlays = raw
            .hide_overlays
            .into_iter()
            .map(selector)
            .collect::<Result<Vec<_>, _>>()?;
        let pages = raw
            .start_pages
            .into_iter()
            .map(|url| {
                url::Url::parse(&url)
                    .map_err(|_| RepositoryError::Storage("invalid start page URL".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let next = raw.next_page.map(selector).transpose()?;
        let load_more = raw.load_more.map(selector).transpose()?;
        let actions = reader_ingest::WebFeedActions::new(
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
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let listing = raw
            .listing_url
            .map(|value| {
                url::Url::parse(&value)
                    .map_err(|_| RepositoryError::Storage("invalid listing URL".into()))
            })
            .transpose()?;
        let extraction = reader_ingest::WebExtraction::new(
            listing,
            raw.card_selector,
            raw.title_selector,
            raw.date_selector,
            raw.content_selector,
            raw.wait_selector,
            raw.url_pattern,
        )
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let recipe = reader_ingest::WebFeedRecipe::legacy(
            primary,
            loading,
            actions,
            extraction,
            raw.max_pages.unwrap_or(1),
        )
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let source = SourceDefinition::new(
            SourceId::new(),
            value.source_url().clone(),
            SourceKind::WebPage(recipe),
        )
        .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let item = WorkItem::CollectWebFeed {
            source_id: source.id(),
        };
        self.transport
            .provision_subscription(
                value.id().as_uuid().to_string(),
                encode(&value)?,
                value.source_url().as_str().to_owned(),
                source.id().as_uuid().to_string(),
                encode(&source)?,
                JobId::new().as_uuid().to_string(),
                encode(&item)?,
                self.initial_scope,
                true,
                Some(recipe_json),
            )
            .await
            .map_err(RepositoryError::Storage)
    }
    async fn web_feed_recipe(
        &self,
        subscription: SubscriptionId,
    ) -> Result<(u64, String), RepositoryError> {
        self.transport
            .web_feed_recipe(subscription.as_uuid().to_string())
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)
    }
    async fn update_web_feed_recipe(
        &self,
        subscription: SubscriptionId,
        expected_version: u64,
        recipe_json: String,
    ) -> Result<u64, RepositoryError> {
        let value = self.subscription(subscription).await?;
        let (raw, recipe) = stored_web_recipe(&recipe_json)?;
        if raw.workspace_id != value.workspace_id().as_uuid()
            || raw.url != value.source_url().as_str()
        {
            return Err(RepositoryError::Storage(
                "web feed recipe scope or URL does not match subscription".into(),
            ));
        }
        let source_id = self
            .transport
            .source_id_for_subscription(subscription.as_uuid().to_string())
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)?;
        let source_id = uuid::Uuid::parse_str(&source_id)
            .map(SourceId::from_uuid)
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let current: SourceDefinition = read_one(
            self.transport.as_ref(),
            "sources",
            source_id.as_uuid().to_string(),
        )
        .await?;
        let source = current
            .revise_kind(SourceKind::WebPage(recipe))
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let next = expected_version
            .checked_add(1)
            .ok_or_else(|| RepositoryError::Storage("web feed recipe version overflow".into()))?;
        let item = WorkItem::CollectWebFeed { source_id };
        let job = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            format!("web-recipe/{}/{next}", subscription.as_uuid()).as_bytes(),
        );
        let updated = self
            .transport
            .update_web_feed_recipe(
                subscription.as_uuid().to_string(),
                expected_version,
                next,
                recipe_json,
                encode(&source)?,
                job.to_string(),
                encode(&item)?,
            )
            .await
            .map_err(RepositoryError::Storage)?;
        if updated {
            Ok(next)
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn article(
        &self,
        workspace: WorkspaceId,
        id: ArticleId,
    ) -> Result<Article, RepositoryError> {
        read_one(
            self.transport.as_ref(),
            "articles",
            format!("{}/{}", workspace.as_uuid(), id.as_uuid()),
        )
        .await
    }
    async fn articles_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Article>, RepositoryError> {
        scan(
            self.transport.as_ref(),
            "articles",
            &format!("{}/", workspace.as_uuid()),
        )
        .await
    }
    async fn article_presentations_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<reader_application::ArticlePresentation>, RepositoryError> {
        let articles = self.articles_by_workspace(workspace).await?;
        let mut result = Vec::with_capacity(articles.len());
        for article in articles {
            let (sub_docs, manifest_docs, failure_docs) = self
                .transport
                .presentation_metadata(
                    workspace.as_uuid().to_string(),
                    article.id.as_uuid().to_string(),
                )
                .await
                .map_err(RepositoryError::Storage)?;
            let mut subscription_ids = Vec::new();
            let mut titles = Vec::new();
            for raw in sub_docs {
                let sub: Subscription = decode(&raw)?;
                sub.validate(self.reason_policy)
                    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
                if !subscription_ids.contains(&sub.id()) {
                    subscription_ids.push(sub.id())
                }
                if !titles.iter().any(|v| v == sub.title()) {
                    titles.push(sub.title().to_owned())
                }
            }
            let mut revisions = Vec::new();
            for raw in manifest_docs {
                revisions.push(decode::<reader_ingest::ContentRevision>(&raw)?)
            }
            let latest = revisions.into_iter().max_by_key(|v| v.fetched_at);
            let safe_html = latest
                .as_ref()
                .map(|revision| {
                    let mut chunks = revision.safe_html_chunks.clone();
                    chunks.sort_by_key(|v| v.ordinal);
                    let mut bytes = Vec::new();
                    for chunk in chunks {
                        bytes.extend_from_slice(&chunk.bytes)
                    }
                    String::from_utf8(bytes).map_err(|_| {
                        RepositoryError::Storage("safe HTML content is not UTF-8".into())
                    })
                })
                .transpose()?;
            let failure_reason = failure_docs
                .last()
                .and_then(|raw| serde_json::from_slice::<serde_json::Value>(raw).ok())
                .and_then(|v| {
                    v.get("diagnostic")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                });
            let status = if safe_html.is_some() {
                "ready"
            } else if failure_reason.is_some() {
                "failed"
            } else {
                "pending"
            };
            result.push(reader_application::ArticlePresentation {
                article,
                subscription_ids,
                subscription_titles: titles,
                safe_html,
                full_text_status: status,
                failure_reason,
            })
        }
        Ok(result)
    }
    async fn save_article(
        &self,
        workspace: WorkspaceId,
        expected: Option<u64>,
        value: Article,
    ) -> Result<(), RepositoryError> {
        cas(
            self.transport.as_ref(),
            "articles",
            format!("{}/{}", workspace.as_uuid(), value.id.as_uuid()),
            expected,
            value.revision,
            &value,
        )
        .await
    }
    async fn enqueue_article_full_text_refresh(
        &self,
        _workspace: WorkspaceId,
        value: &Article,
    ) -> Result<(), RepositoryError> {
        let mut enqueued = 0usize;
        for origin in &value.origins {
            let record: reader_ingest::SourceRecord = read_one(
                self.transport.as_ref(),
                "source_records",
                origin.as_uuid().to_string(),
            )
            .await?;
            let Some(url) = record.key().location.fetch_url().cloned() else {
                continue;
            };
            let item = WorkItem::ExtractFullText {
                record_id: record.id(),
                source_revision: record.revision(),
                url,
                manual: true,
            };
            let id = uuid::Uuid::new_v4();
            self.transport
                .enqueue_ingest_job(id.to_string(), encode(&item)?)
                .await
                .map_err(RepositoryError::Storage)?;
            enqueued += 1;
        }
        if enqueued == 0 {
            Err(RepositoryError::Storage(
                "article has no fetchable URL origin".into(),
            ))
        } else {
            Ok(())
        }
    }
    async fn rules_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Rule>, RepositoryError> {
        let values: Vec<Rule> = scan(
            self.transport.as_ref(),
            "rules",
            &format!("{}/", workspace.as_uuid()),
        )
        .await?;
        for value in &values {
            value
                .validate()
                .map_err(|e| RepositoryError::Storage(e.to_string()))?
        }
        Ok(values)
    }
    async fn rule(&self, workspace: WorkspaceId, id: RuleId) -> Result<Rule, RepositoryError> {
        let value: Rule = read_one(
            self.transport.as_ref(),
            "rules",
            format!("{}/{}", workspace.as_uuid(), id.as_uuid()),
        )
        .await?;
        value
            .validate()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(value)
    }
    async fn save_rule(
        &self,
        workspace: WorkspaceId,
        expected: Option<u64>,
        value: Rule,
    ) -> Result<(), RepositoryError> {
        value
            .validate()
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        cas(
            self.transport.as_ref(),
            "rules",
            format!("{}/{}", workspace.as_uuid(), value.id.as_uuid()),
            expected,
            value.version,
            &value,
        )
        .await
    }
    async fn delete_rule(
        &self,
        workspace: WorkspaceId,
        id: RuleId,
        expected: u64,
    ) -> Result<(), RepositoryError> {
        if self
            .transport
            .delete(
                "rules",
                format!("{}/{}", workspace.as_uuid(), id.as_uuid()),
                expected,
            )
            .await
            .map_err(RepositoryError::Storage)?
        {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn enqueue_rule_application(
        &self,
        workspace: WorkspaceId,
        rule: Rule,
    ) -> Result<uuid::Uuid, RepositoryError> {
        let identity = format!(
            "apply-rule/{}/{}/{}",
            workspace.as_uuid(),
            rule.id.as_uuid(),
            rule.version
        );
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, identity.as_bytes());
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
        self.transport
            .enqueue_ingest_job(id.to_string(), encode(&item)?)
            .await
            .map_err(RepositoryError::Storage)?;
        Ok(id)
    }
    async fn rule_application_progress(
        &self,
        workspace: WorkspaceId,
        operation: uuid::Uuid,
    ) -> Result<reader_application::RuleApplicationProgress, RepositoryError> {
        let (_, initial, _) = self
            .transport
            .ingest_job(operation.to_string())
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)?;
        let initial: WorkItem =
            serde_json::from_str(&initial).map_err(|e| RepositoryError::Storage(e.to_string()))?;
        let WorkItem::ApplyRule {
            workspace_id,
            rule_id,
            rule_version,
            ..
        } = initial
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
        let mut pending = false;
        let mut failure = None;
        for (status, item, diagnostic) in self
            .transport
            .ingest_jobs()
            .await
            .map_err(RepositoryError::Storage)?
        {
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
            }
            match status.as_str() {
                "ready" | "leased" | "retry" => pending = true,
                "failed" => failure = diagnostic.or(Some("rule application failed".to_owned())),
                _ => {}
            }
        }
        let evaluated = self
            .transport
            .rule_evaluation_count(
                workspace.as_uuid().to_string(),
                rule_id.as_uuid().to_string(),
                rule_version,
            )
            .await
            .map_err(RepositoryError::Storage)?;
        let (status, cancel_reason) = if let Some(reason) = cancel_reason {
            ("cancelled".to_owned(), Some(reason))
        } else if let Some(reason) = failure {
            ("failed".to_owned(), Some(reason))
        } else if pending {
            ("running".to_owned(), None)
        } else {
            ("completed".to_owned(), None)
        };
        Ok(reader_application::RuleApplicationProgress {
            operation_id: operation,
            workspace_id: workspace,
            rule_id,
            rule_version,
            status,
            evaluated,
            cancel_reason,
        })
    }
    async fn record_login_attempt(
        &self,
        username: &str,
        at: chrono::DateTime<chrono::Utc>,
        limit: u32,
    ) -> Result<bool, RepositoryError> {
        let key = username.to_owned();
        let current = self
            .transport
            .read("login_attempts", key.clone())
            .await
            .map_err(RepositoryError::Storage)?;
        let (mut value, expected) = match current {
            Some(raw) => {
                let value: LoginAttempts = decode(&raw)?;
                let revision = value.revision;
                (value, Some(revision))
            }
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
        let next = expected.map_or(0, |v| v + 1);
        value.revision = next;
        if self
            .transport
            .compare_and_swap("login_attempts", key, expected, next, encode(&value)?)
            .await
            .map_err(RepositoryError::Storage)?
        {
            Ok(true)
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn mark_articles_read_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Article>,
    ) -> Result<(), RepositoryError> {
        let mut rows = Vec::with_capacity(values.len());
        for value in values {
            let expected = value.revision.checked_sub(1).ok_or_else(|| {
                RepositoryError::Storage("updated article revision is invalid".into())
            })?;
            rows.push((
                format!("{}/{}", workspace.as_uuid(), value.id.as_uuid()),
                expected,
                value.revision,
                encode(&value)?,
            ));
        }
        if self
            .transport
            .atomic_cas_many("articles", rows)
            .await
            .map_err(RepositoryError::Storage)?
        {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn import_subscriptions_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        let mut rows = Vec::with_capacity(values.len());
        for value in values {
            if value.workspace_id() != workspace {
                return Err(RepositoryError::Storage(
                    "import subscription scope does not match target workspace".into(),
                ));
            }
            value
                .validate(self.reason_policy)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let source = SourceDefinition::new(
                SourceId::new(),
                value.source_url().clone(),
                SourceKind::Auto,
            )
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            rows.push((
                value.id().as_uuid().to_string(),
                encode(&value)?,
                value.source_url().as_str().to_owned(),
                source.id().as_uuid().to_string(),
                encode(&source)?,
                JobId::new().as_uuid().to_string(),
            ));
        }
        self.transport
            .provision_subscriptions(rows, self.initial_scope)
            .await
            .map_err(RepositoryError::Storage)
    }
    async fn apply_seed_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<(
            String,
            Subscription,
            reader_application::SeedSource,
            serde_json::Value,
        )>,
    ) -> Result<(), RepositoryError> {
        let mut rows = Vec::with_capacity(values.len());
        for (key, value, kind, configuration) in values {
            if value.workspace_id() != workspace {
                return Err(RepositoryError::Storage(
                    "seed subscription scope does not match target workspace".into(),
                ));
            }
            value
                .validate(self.reason_policy)
                .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let source_id = SourceId::from_uuid(uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                value.source_url().as_str().as_bytes(),
            ));
            let source_kind = match kind {
                reader_application::SeedSource::Feed => SourceKind::Auto,
                reader_application::SeedSource::Imported => imported_source_kind(&configuration)?,
            };
            let recipe_document =
                editable_recipe_document(workspace, value.source_url(), &source_kind)?;
            let source =
                SourceDefinition::new(source_id, value.source_url().clone(), source_kind.clone())
                    .map_err(|e| RepositoryError::Storage(e.to_string()))?;
            let item = match source_kind {
                SourceKind::WebPage(_) | SourceKind::BuiltIn(_) => {
                    WorkItem::CollectWebFeed { source_id }
                }
                _ => WorkItem::PollSource { source_id },
            };
            let job = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_OID,
                format!("seed-job/{key}").as_bytes(),
            );
            rows.push((
                value.id().as_uuid().to_string(),
                encode(&value)?,
                value.source_url().as_str().to_owned(),
                source_id.as_uuid().to_string(),
                encode(&source)?,
                job.to_string(),
                encode(&item)?,
                key.clone(),
                serde_json::to_vec(&configuration)
                    .map_err(|e| RepositoryError::Storage(e.to_string()))?,
                recipe_document,
            ));
        }
        self.transport
            .apply_seed(rows, self.initial_scope)
            .await
            .map_err(RepositoryError::Storage)
    }
}

#[cfg(test)]
mod tests;

async fn read_one<T: YdbTransport, V: DeserializeOwned>(
    transport: &T,
    table: &'static str,
    key: String,
) -> Result<V, RepositoryError> {
    decode(
        &transport
            .read(table, key)
            .await
            .map_err(RepositoryError::Storage)?
            .ok_or(RepositoryError::NotFound)?,
    )
}
async fn scan<T: YdbTransport, V: DeserializeOwned>(
    transport: &T,
    table: &'static str,
    prefix: &str,
) -> Result<Vec<V>, RepositoryError> {
    transport
        .scan_prefix(table, prefix.to_owned())
        .await
        .map_err(RepositoryError::Storage)?
        .iter()
        .map(|v| decode(v))
        .collect()
}
async fn cas<T: YdbTransport, V: Serialize>(
    transport: &T,
    table: &'static str,
    key: String,
    expected: Option<u64>,
    revision: u64,
    value: &V,
) -> Result<(), RepositoryError> {
    if transport
        .compare_and_swap(table, key, expected, revision, encode(value)?)
        .await
        .map_err(RepositoryError::Storage)?
    {
        Ok(())
    } else {
        Err(RepositoryError::Conflict)
    }
}
