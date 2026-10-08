mod error;
mod search;
use error::ApiFailure;
mod presentation;
use presentation::*;
mod session;
use session::*;
mod workspaces;
use workspaces::*;
mod subscriptions;
use subscriptions::*;
mod discovery;
use discovery::*;
mod subscription_commands;
use subscription_commands::*;
mod articles;
mod focused_reading;
use articles::*;
mod rules;
use rules::*;
mod imports;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
    Json, Router,
};
use chrono::Utc;
use imports::*;
use reader_application::{
    ArticlePageCursor, ArticlePageDirection, ArticlePageRequest, AuthError, AuthPolicy,
    AuthService, CommandError, ReaderRepository, ReaderService, RepositoryError, SessionRecord,
};
use reader_core::{
    AccountId, ActorId, ArticleId, ReasonPolicy, Rule, RuleAction, RuleField, RuleId, Subscription,
    SubscriptionId, Workspace, WorkspaceId, WorkspaceStatus,
};
use reader_server_contracts::*;
use std::sync::Arc;
use url::Url;
use uuid::Uuid;

const SESSION_COOKIE: &str = "reader_session";
const ARTICLE_PAGE_SIZE: usize = 50;

mod ai;
mod api_observability;
mod glossary;
mod opml;
mod wiki;
mod zhihu;

#[async_trait::async_trait]
pub trait FeedDiscovery: Send + Sync {
    async fn discover(&self, url: Url) -> Result<FeedPreviewResponse, String>;
    async fn preview_web_feed(
        &self,
        draft: &WebFeedRecipeDraft,
    ) -> Result<(FeedPreviewResponse, reader_core::PreparedWebFeed), String>;
    async fn visual_preview(
        &self,
        _session_id: Uuid,
        _request: &VisualPreviewRequest,
    ) -> Result<VisualPreviewResponse, String> {
        Err("visual selection unavailable".into())
    }
    async fn visual_select(
        &self,
        _session_id: Uuid,
        _request: &VisualSelectionRequest,
    ) -> Result<VisualSelectionResponse, String> {
        Err("visual selection unavailable".into())
    }
}

pub struct AppState<R> {
    ai: Option<Arc<reader_ai::AiService>>,
    zhihu: Option<Arc<dyn reader_application::ZhihuProfilePort>>,
    glossary: Option<Arc<reader_glossary::GlossaryService>>,
    search: Option<(
        Arc<dyn reader_application::SearchPort>,
        reader_application::SearchLimits,
    )>,
    wiki: Option<(Arc<dyn reader_wiki::Store>, reader_wiki::Limits)>,
    repository: Arc<R>,
    discovery: Arc<dyn FeedDiscovery>,
    reason_policy: ReasonPolicy,
    auth_policy: AuthPolicy,
    external_origin: String,
    login_attempts_per_minute: u32,
    bulk_mutation_batch: reader_application::SelectionLimit,
}
impl<R> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            ai: self.ai.clone(),
            zhihu: self.zhihu.clone(),
            glossary: self.glossary.clone(),
            wiki: self.wiki.clone(),
            search: self.search.clone(),
            repository: self.repository.clone(),
            discovery: self.discovery.clone(),
            reason_policy: self.reason_policy,
            auth_policy: self.auth_policy,
            external_origin: self.external_origin.clone(),
            login_attempts_per_minute: self.login_attempts_per_minute,
            bulk_mutation_batch: self.bulk_mutation_batch,
        }
    }
}
impl<R> AppState<R> {
    pub fn new(
        repository: Arc<R>,
        discovery: Arc<dyn FeedDiscovery>,
        reason_policy: ReasonPolicy,
        auth_policy: AuthPolicy,
        external_origin: String,
        login_attempts_per_minute: u32,
        bulk_mutation_batch: reader_application::SelectionLimit,
    ) -> Self {
        Self {
            ai: None,
            zhihu: None,
            glossary: None,
            wiki: None,
            search: None,
            repository,
            discovery,
            reason_policy,
            auth_policy,
            external_origin,
            login_attempts_per_minute,
            bulk_mutation_batch,
        }
    }

    pub fn with_search(
        mut self,
        store: Arc<dyn reader_application::SearchPort>,
        limits: reader_application::SearchLimits,
    ) -> Self {
        self.search = Some((store, limits));
        self
    }
    pub fn with_wiki(
        mut self,
        store: Arc<dyn reader_wiki::Store>,
        limits: reader_wiki::Limits,
    ) -> Self {
        self.wiki = Some((store, limits));
        self
    }
    pub fn with_glossary(mut self, glossary: Arc<reader_glossary::GlossaryService>) -> Self {
        self.glossary = Some(glossary);
        self
    }
    pub fn with_zhihu(mut self, service: Arc<dyn reader_application::ZhihuProfilePort>) -> Self {
        self.zhihu = Some(service);
        self
    }
    pub fn with_ai(mut self, ai: Arc<reader_ai::AiService>) -> Self {
        self.ai = Some(ai);
        self
    }
}

pub fn router<R: ReaderRepository + 'static>(state: AppState<R>) -> Router {
    Router::new()
        .route("/live", get(|| async { StatusCode::NO_CONTENT }))
        .route("/ready", get(ready::<R>))
        .route("/api/health", get(ready::<R>))
        .route(
            "/api/auth/sessions",
            post(sign_in::<R>).delete(sign_out::<R>),
        )
        .route("/api/auth/invites", post(create_invite::<R>))
        .route("/api/auth/invites/accept", post(accept_invite::<R>))
        .route("/api/auth/password/change", post(change_password::<R>))
        .route(
            "/api/auth/password-resets",
            post(create_password_reset::<R>),
        )
        .route("/api/auth/password/reset", post(reset_password::<R>))
        .route("/api/bootstrap", get(bootstrap::<R>))
        .route("/api/workspaces", post(create_workspace::<R>))
        .route("/api/workspaces/{id}", patch(rename_workspace::<R>))
        .route("/api/workspaces/{id}/archive", post(archive::<R>))
        .route("/api/workspaces/{id}/restore", post(restore::<R>))
        .route(
            "/api/workspaces/{id}/articles/mark-all-read",
            post(mark_all_read::<R>),
        )
        .route(
            "/api/subscriptions",
            get(list_subscriptions::<R>).post(add_subscription::<R>),
        )
        .route(
            "/api/subscriptions/{id}",
            get(get_subscription::<R>).patch(rename_subscription::<R>),
        )
        .route(
            "/api/subscriptions/{id}/delete",
            post(delete_subscription::<R>),
        )
        .route(
            "/api/subscriptions/{id}/archive",
            post(archive_subscription::<R>),
        )
        .route(
            "/api/subscriptions/{id}/activity",
            get(subscription_activity::<R>),
        )
        .route(
            "/api/subscriptions/{id}/publication-history",
            get(publication_history::<R>),
        )
        .route(
            "/api/subscriptions/{id}/extraction",
            get(subscription_extraction::<R>),
        )
        .route(
            "/api/subscriptions/{id}/source-url/preview",
            post(preview_subscription_source_url::<R>),
        )
        .route(
            "/api/subscriptions/{id}/source-url",
            put(commit_subscription_source_url::<R>),
        )
        .route("/api/feeds/discover", post(discover_feed::<R>))
        .route("/api/web-feeds/recipes", post(web_feed_recipe::<R>))
        .route(
            "/api/web-feeds/recipes/{id}",
            get(get_web_feed_recipe::<R>).put(update_web_feed_recipe::<R>),
        )
        .route("/api/web-feeds/visual-previews", post(visual_preview::<R>))
        .route("/api/web-feeds/visual-selections", post(visual_select::<R>))
        .route("/api/subscriptions/{id}/pause", post(pause::<R>))
        .route("/api/subscriptions/{id}/resume", post(resume::<R>))
        .route(
            "/api/subscriptions/{id}/restore",
            post(restore_subscription::<R>),
        )
        .route(
            "/api/subscriptions/{id}/refresh",
            post(refresh_subscription::<R>),
        )
        .route(
            "/api/workspaces/{id}/reading-activity",
            get(reading_activity::<R>),
        )
        .route(
            "/api/workspaces/{id}/source-activity",
            get(source_activity::<R>),
        )
        .route("/api/articles", get(list_articles::<R>))
        .route(
            "/api/articles/{id}/reading/commits",
            get(focused_reading::commits::<R>),
        )
        .route("/api/articles/{id}", get(get_article::<R>))
        .route("/api/articles/{id}/state", post(update_article::<R>))
        .route(
            "/api/articles/{id}/reading/next",
            get(focused_reading::next::<R>),
        )
        .route(
            "/api/articles/{id}/reading",
            get(focused_reading::state::<R>).post(focused_reading::complete::<R>),
        )
        .route(
            "/api/articles/{id}/reading/{operation}/undo",
            post(focused_reading::undo::<R>),
        )
        .route(
            "/api/articles/{id}/full-text/refresh",
            post(refresh_full_text::<R>),
        )
        .route("/api/rules", get(list_rules::<R>).post(create_rule::<R>))
        .route("/api/rules/preview", post(preview_rule::<R>))
        .route(
            "/api/rules/{id}",
            put(update_rule::<R>).delete(delete_rule::<R>),
        )
        .route("/api/rules/{id}/apply", post(apply_rule::<R>))
        .route(
            "/api/rule-applications/{id}",
            get(rule_application_status::<R>),
        )
        .route("/api/opml/import", post(import_opml::<R>))
        .route("/api/opml/export", get(export_opml::<R>))
        .merge(ai::routes::<R>())
        .merge(glossary::routes::<R>())
        .merge(zhihu::routes::<R>())
        .merge(wiki::routes::<R>())
        .merge(search::routes::<R>())
        .with_state(state)
        .layer(middleware::from_fn(api_observability::observe))
}

async fn ready<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
) -> Result<StatusCode, ApiFailure> {
    s.repository.readiness().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Clone)]
struct RequestAuth {
    session: SessionRecord,
    account: reader_application::AccountRecord,
}

async fn auth<R: ReaderRepository>(
    state: &AppState<R>,
    headers: &HeaderMap,
) -> Result<RequestAuth, ApiFailure> {
    let raw = cookie(headers, SESSION_COOKIE).ok_or(ApiFailure::Unauthorized)?;
    let (session, account) = AuthService::new(state.repository.clone(), state.auth_policy)
        .authenticate(raw, Utc::now())
        .await?;
    Ok(RequestAuth { session, account })
}
fn csrf<R>(state: &AppState<R>, headers: &HeaderMap) -> Result<(), ApiFailure> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiFailure::Csrf)?;
    if origin == state.external_origin {
        Ok(())
    } else {
        Err(ApiFailure::Csrf)
    }
}
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|v| {
            v.split_once('=')
                .filter(|(n, _)| *n == name)
                .map(|(_, v)| v)
        })
}
fn session_cookie(raw: &str, max_age: u64) -> Result<HeaderValue, ApiFailure> {
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={raw}; Path=/; Max-Age={max_age}; Secure; HttpOnly; SameSite=Strict"
    ))
    .map_err(|_| ApiFailure::Internal)
}
fn expired_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "reader_session=; Path=/; Max-Age=0; Secure; HttpOnly; SameSite=Strict",
    )
}

async fn bootstrap<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<BootstrapQuery>,
) -> Result<Json<BootstrapResponse>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspaces = s.repository.workspaces_by_owner(actor.account.id).await?;
    let default_id = workspaces
        .iter()
        .find(|w| matches!(w.status(), WorkspaceStatus::Active))
        .or_else(|| workspaces.first())
        .map(Workspace::id)
        .ok_or(ApiFailure::NoWorkspace)?;
    let active_id = match query.workspace_id {
        Some(id) => workspaces
            .iter()
            .find(|workspace| workspace.id().as_uuid() == id)
            .map(Workspace::id)
            .ok_or(RepositoryError::NotFound)?,
        None => default_id,
    };
    let subscriptions = s.repository.subscriptions_by_workspace(active_id).await?;
    let article_page = s
        .repository
        .article_summary_page(
            active_id,
            article_page_request(
                query.view.as_deref().unwrap_or("feed"),
                query.subscription_id.map(SubscriptionId::from_uuid),
                query.cursor.as_deref(),
                query.direction.as_deref(),
            )?
            .with_read_period(read_period(query.read_from, query.read_until)?)
            .map_err(ApiFailure::Validation)?,
        )
        .await?;
    let initials = actor
        .account
        .username
        .chars()
        .next()
        .map(|v| v.to_uppercase().to_string())
        .unwrap_or_default();
    Ok(Json(BootstrapResponse {
        account: BootstrapAccount {
            id: actor.account.id.as_uuid(),
            display_name: actor.account.username,
            initials,
        },
        workspaces: workspaces.iter().map(workspace_view).collect(),
        active_workspace_id: active_id.as_uuid(),
        subscriptions: subscription_views(s.repository.as_ref(), &subscriptions).await?,
        article_page: article_page_view(article_page),
    }))
}
async fn owned_workspace<R: ReaderRepository>(
    s: &AppState<R>,
    id: WorkspaceId,
    owner: AccountId,
) -> Result<Workspace, ApiFailure> {
    let value = s.repository.workspace(id).await?;
    require_owner(value.owner(), owner)?;
    Ok(value)
}
fn require_owner(actual: AccountId, expected: AccountId) -> Result<(), ApiFailure> {
    if actual == expected {
        Ok(())
    } else {
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    }
}
async fn owned_subscription<R: ReaderRepository>(
    s: &AppState<R>,
    id: SubscriptionId,
    owner: AccountId,
) -> Result<Subscription, ApiFailure> {
    let value = s.repository.subscription(id).await?;
    owned_workspace(s, value.workspace_id(), owner).await?;
    Ok(value)
}
fn valid_name(value: &str) -> Result<(), ApiFailure> {
    if value.is_empty() || value.trim() != value || value.len() > 256 {
        Err(ApiFailure::Validation("invalid name"))
    } else {
        Ok(())
    }
}
fn parse_rule(identity: Option<(RuleId, u64)>, body: &RuleDraft) -> Result<Rule, ApiFailure> {
    if body.phrase.is_empty() || body.phrase.trim() != body.phrase {
        return Err(ApiFailure::Validation("invalid rule phrase"));
    }
    let field = match body.field.as_str() {
        "title" => RuleField::Title,
        "full_text" => RuleField::Text,
        "title_or_full_text" => RuleField::Both,
        _ => return Err(ApiFailure::Validation("invalid rule field")),
    };
    let action = match body.action.as_str() {
        "mark_read" => RuleAction::MarkRead,
        _ => return Err(ApiFailure::Validation("invalid rule action")),
    };
    let (id, version) = identity.unwrap_or((RuleId::new(), 0));
    Ok(Rule {
        id,
        subscription_id: SubscriptionId::from_uuid(body.subscription_id),
        version,
        enabled: body.enabled,
        field,
        needles: vec![body.phrase.clone()],
        action,
    })
}
fn rule_draft(value: Rule) -> RuleDraft {
    RuleDraft {
        id: Some(value.id.as_uuid()),
        subscription_id: value.subscription_id.as_uuid(),
        field: match value.field {
            RuleField::Title => "title",
            RuleField::Text => "full_text",
            RuleField::Both => "title_or_full_text",
        }
        .into(),
        phrase: value.needles.into_iter().next().unwrap_or_default(),
        action: match value.action {
            RuleAction::MarkRead => "mark_read",
        }
        .into(),
        enabled: value.enabled,
    }
}
fn repository_failure(e: RepositoryError) -> (StatusCode, &'static str, String) {
    match e {
        RepositoryError::NotFound => (StatusCode::NOT_FOUND, "not_found", e.to_string()),
        RepositoryError::Conflict => (StatusCode::CONFLICT, "conflict", e.to_string()),
        RepositoryError::Storage(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "storage_error",
            "storage operation failed".into(),
        ),
    }
}

#[cfg(test)]
mod tests;
