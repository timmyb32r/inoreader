use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
    Json, Router,
};
use chrono::Utc;
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

#[async_trait::async_trait]
pub trait FeedDiscovery: Send + Sync {
    async fn discover(&self, url: Url) -> Result<FeedPreviewResponse, String>;
    async fn preview_web_feed(
        &self,
        draft: &WebFeedRecipeDraft,
    ) -> Result<FeedPreviewResponse, String>;
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
    repository: Arc<R>,
    discovery: Arc<dyn FeedDiscovery>,
    reason_policy: ReasonPolicy,
    auth_policy: AuthPolicy,
    external_origin: String,
    login_attempts_per_minute: u32,
    bulk_mutation_limit: usize,
}
impl<R> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            repository: self.repository.clone(),
            discovery: self.discovery.clone(),
            reason_policy: self.reason_policy,
            auth_policy: self.auth_policy,
            external_origin: self.external_origin.clone(),
            login_attempts_per_minute: self.login_attempts_per_minute,
            bulk_mutation_limit: self.bulk_mutation_limit,
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
        bulk_mutation_limit: usize,
    ) -> Self {
        Self {
            repository,
            discovery,
            reason_policy,
            auth_policy,
            external_origin,
            login_attempts_per_minute,
            bulk_mutation_limit,
        }
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
            get(get_subscription::<R>)
                .patch(rename_subscription::<R>)
                .delete(unsubscribe::<R>),
        )
        .route(
            "/api/subscriptions/{id}/note",
            put(save_subscription_note::<R>),
        )
        .route(
            "/api/subscriptions/{id}/activity",
            get(subscription_activity::<R>),
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
        .route("/api/articles", get(list_articles::<R>))
        .route("/api/articles/{id}", get(get_article::<R>))
        .route("/api/articles/{id}/state", post(update_article::<R>))
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
        .with_state(state)
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

async fn sign_in<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Response, ApiFailure> {
    csrf(&s, &headers)?;
    if !s
        .repository
        .record_login_attempt(&body.username, Utc::now(), s.login_attempts_per_minute)
        .await?
    {
        return Err(ApiFailure::RateLimited);
    }
    let (raw, session) = AuthService::new(s.repository, s.auth_policy)
        .sign_in(&body.username, &body.password, Utc::now())
        .await?;
    let mut response = Json(SessionResponse {
        session_id: session.id,
        expires_at: session.expires_at,
    })
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        session_cookie(&raw, s.auth_policy.session_lifetime_seconds)?,
    );
    Ok(response)
}
async fn sign_out<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Response, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    AuthService::new(s.repository, s.auth_policy)
        .sign_out(&actor.session)
        .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, expired_cookie());
    Ok(response)
}
async fn create_invite<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreateInviteRequest>,
) -> Result<Json<InviteResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let (token, value) = AuthService::new(s.repository, s.auth_policy)
        .create_invite(&actor.account, body.username, Utc::now())
        .await?;
    Ok(Json(InviteResponse {
        url: format!(
            "{}/invite?token={}",
            s.external_origin.trim_end_matches('/'),
            token
        ),
        expires_at: value.expires_at,
    }))
}
async fn accept_invite<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<AcceptInviteRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    AuthService::new(s.repository.clone(), s.auth_policy)
        .accept_invite(&body.token, body.username, &body.password, Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn change_password<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    AuthService::new(s.repository, s.auth_policy)
        .change_password(actor.account.id, &body.current_password, &body.new_password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn reset_password<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    AuthService::new(s.repository, s.auth_policy)
        .reset_password(&body.token, &body.new_password, Utc::now())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn create_password_reset<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreatePasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let (token, record) = AuthService::new(s.repository, s.auth_policy)
        .create_password_reset(&actor.account, &body.username, Utc::now())
        .await?;
    Ok(Json(PasswordResetResponse {
        url: format!(
            "{}/reset-password?token={}",
            s.external_origin.trim_end_matches('/'),
            token
        ),
        expires_at: record.expires_at,
    }))
}

async fn bootstrap<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<BootstrapQuery>,
) -> Result<Json<BootstrapResponse>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspaces = s.repository.workspaces_by_owner(actor.account.id).await?;
    let active_id = workspaces
        .iter()
        .find(|w| matches!(w.status(), WorkspaceStatus::Active))
        .or_else(|| workspaces.first())
        .map(Workspace::id)
        .ok_or(ApiFailure::NoWorkspace)?;
    let subscriptions = s.repository.subscriptions_by_workspace(active_id).await?;
    let article_page = s
        .repository
        .article_summary_page(
            active_id,
            article_page_request(
                query.view.as_deref().unwrap_or("all"),
                query.subscription_id.map(SubscriptionId::from_uuid),
                query.cursor.as_deref(),
                query.direction.as_deref(),
            )?,
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
            display_name: actor.account.username,
            initials,
        },
        workspaces: workspaces.iter().map(workspace_view).collect(),
        active_workspace_id: active_id.as_uuid(),
        subscriptions: subscription_views(s.repository.as_ref(), &subscriptions).await?,
        article_page: article_page_view(article_page),
    }))
}
async fn create_workspace<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Result<Json<WorkspaceView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    valid_name(&body.name)?;
    let value = Workspace::new(WorkspaceId::new(), actor.account.id, body.name);
    s.repository.save_workspace(None, value.clone()).await?;
    Ok(Json(workspace_view(&value)))
}
async fn rename_workspace<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RenameWorkspaceRequest>,
) -> Result<Json<WorkspaceView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    valid_name(&body.name)?;
    let mut value = owned_workspace(&s, WorkspaceId::from_uuid(id), actor.account.id).await?;
    let revision = value.revision();
    value.rename(body.name);
    s.repository
        .save_workspace(Some(revision), value.clone())
        .await?;
    Ok(Json(workspace_view(&value)))
}
async fn archive<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ReasonCommand>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    owned_workspace(&s, WorkspaceId::from_uuid(id), actor.account.id).await?;
    ReaderService::new(s.repository, s.reason_policy)
        .archive_workspace(
            WorkspaceId::from_uuid(id),
            body.reason,
            ActorId::from_uuid(actor.account.id.as_uuid()),
            Utc::now(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn restore<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    owned_workspace(&s, WorkspaceId::from_uuid(id), actor.account.id).await?;
    ReaderService::new(s.repository, s.reason_policy)
        .restore_workspace(WorkspaceId::from_uuid(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<AddSubscriptionRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(body.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let url =
        Url::parse(&body.url).map_err(|_| ApiFailure::Validation("invalid subscription URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ApiFailure::Validation(
            "subscription URL must use HTTP or HTTPS",
        ));
    }
    if let Some(existing) = s
        .repository
        .subscriptions_by_workspace(workspace)
        .await?
        .into_iter()
        .find(|subscription| subscription.source_url_exact() == body.url)
    {
        return Err(ApiFailure::ExistingSubscription(matches!(
            existing.status(),
            reader_core::SubscriptionStatus::Archived
        )));
    }
    let discovered = s
        .discovery
        .discover(url.clone())
        .await
        .map_err(ApiFailure::Discovery)?;
    let title = trusted_discovery_title(body.title.as_deref(), discovered.title);
    valid_name(&title)?;
    let value =
        Subscription::new_with_exact_url(SubscriptionId::new(), workspace, url, body.url, title)
            .map_err(|_| ApiFailure::Validation("source URL does not match parsed URL"))?;
    s.repository.save_subscription(None, value.clone()).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
fn trusted_discovery_title(_client_title: Option<&str>, discovered_title: String) -> String {
    discovered_title
}
async fn list_subscriptions<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<Vec<SubscriptionView>>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let values = s.repository.subscriptions_by_workspace(workspace).await?;
    Ok(Json(
        subscription_views(s.repository.as_ref(), &values).await?,
    ))
}
async fn rename_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RenameSubscriptionRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    if !body.name.is_empty() {
        valid_name(&body.name)?;
    }
    let mut value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let revision = value.revision();
    value.rename(body.name);
    s.repository
        .save_subscription(Some(revision), value.clone())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
async fn get_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
async fn save_subscription_note<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveSubscriptionNoteRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let mut value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let revision = value.revision();
    value.set_personal_note(body.note);
    s.repository
        .save_subscription(Some(revision), value.clone())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
async fn subscription_activity<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<SubscriptionActivityView>>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let since = Utc::now() - chrono::Duration::days(30);
    let events = s
        .repository
        .subscription_activity(actor.account.id, subscription.id(), since)
        .await?;
    Ok(Json(
        events
            .into_iter()
            .map(|event| SubscriptionActivityView {
                id: event.id,
                occurred_at: event.occurred_at,
                successful: event.successful,
                duration_ms: event.duration_ms,
                discovered_items: event.discovered_items,
                diagnostic: event.diagnostic,
            })
            .collect(),
    ))
}
async fn subscription_extraction<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionExtractionView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    let (feed_urls, recipe_version, recipe_summary) = if stats.editable_web_feed {
        let (version, document) = s.repository.web_feed_recipe(subscription.id()).await?;
        let draft: WebFeedRecipeDraft =
            serde_json::from_str(&document).map_err(|_| ApiFailure::Internal)?;
        (
            Vec::new(),
            Some(version),
            Some(web_feed_recipe_summary(&draft)),
        )
    } else if stats.source_type == "feed" {
        (vec![subscription.source_url_exact().to_owned()], None, None)
    } else {
        (Vec::new(), None, Some("Built-in adapter".to_owned()))
    };
    Ok(Json(SubscriptionExtractionView {
        source_type: stats.source_type,
        feed_urls,
        recipe_version,
        recipe_summary,
        last_preview: None,
    }))
}
fn web_feed_recipe_summary(draft: &WebFeedRecipeDraft) -> String {
    format!(
        "Cards: {}; title: {}; content: {}",
        draft.card_selector.as_deref().unwrap_or(&draft.selector),
        draft.title_selector.as_deref().unwrap_or("not configured"),
        draft
            .content_selector
            .as_deref()
            .unwrap_or("not configured")
    )
}
fn ensure_source_url_change_allowed(
    stats: &reader_application::SubscriptionStats,
) -> Result<(), ApiFailure> {
    if stats.editable_web_feed {
        Err(ApiFailure::Validation(
            "web feed source URL must be changed through its extraction recipe",
        ))
    } else {
        Ok(())
    }
}
async fn preview_subscription_source_url<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PreviewSourceUrlRequest>,
) -> Result<Json<SourceUrlPreviewResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    ensure_source_url_change_allowed(&stats)?;
    let url = Url::parse(&body.url).map_err(|_| ApiFailure::Validation("invalid feed URL"))?;
    let preview = s
        .discovery
        .discover(url.clone())
        .await
        .map_err(ApiFailure::Discovery)?;
    let token = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::minutes(10);
    s.repository
        .save_source_url_preview(reader_application::SourceUrlPreviewRecord {
            id: token,
            subscription_id: subscription.id(),
            url: body.url.clone(),
            source_title: preview.title.clone(),
            expires_at,
            revision: 0,
        })
        .await?;
    Ok(Json(SourceUrlPreviewResponse {
        token,
        url: body.url,
        title: preview.title,
        expires_at,
    }))
}
async fn commit_subscription_source_url<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<CommitSourceUrlRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let mut subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    ensure_source_url_change_allowed(&stats)?;
    let preview = s.repository.source_url_preview(body.preview_token).await?;
    if preview.subscription_id != subscription.id() {
        return Err(ApiFailure::Repository(RepositoryError::NotFound));
    }
    if preview.expires_at <= Utc::now() {
        return Err(ApiFailure::Validation("source URL preview expired"));
    }
    let url = Url::parse(&preview.url)
        .map_err(|_| ApiFailure::Validation("stored source URL preview is invalid"))?;
    if s.repository
        .subscriptions_by_workspace(subscription.workspace_id())
        .await?
        .iter()
        .any(|candidate| {
            candidate.id() != subscription.id() && candidate.source_url_exact() == preview.url
        })
    {
        return Err(ApiFailure::ExistingSubscription(false));
    }
    let verified = s
        .discovery
        .discover(url.clone())
        .await
        .map_err(ApiFailure::Discovery)?;
    if verified.title != preview.source_title {
        return Err(ApiFailure::Validation("source changed since preview"));
    }
    let revision = subscription.revision();
    subscription
        .replace_source(url, preview.url, preview.source_title)
        .map_err(|_| ApiFailure::Validation("source URL preview is inconsistent"))?;
    s.repository
        .replace_subscription_source(revision, subscription.clone())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    Ok(Json(subscription_view(&subscription, &stats)))
}
async fn unsubscribe<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let value = ReaderService::new(s.repository.clone(), s.reason_policy)
        .archive_subscription(value.id())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
async fn discover_feed<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<FeedDiscoverRequest>,
) -> Result<Json<FeedPreviewResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    auth(&s, &headers).await?;
    let url = Url::parse(&body.url).map_err(|_| ApiFailure::Validation("invalid feed URL"))?;
    let preview = s
        .discovery
        .discover(url)
        .await
        .map_err(ApiFailure::Discovery)?;
    Ok(Json(preview))
}
async fn web_feed_recipe<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<WebFeedRecipeDraft>,
) -> Result<Response, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(body.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let preview = s
        .discovery
        .preview_web_feed(&body)
        .await
        .map_err(ApiFailure::Discovery)?;
    if body.preview.unwrap_or(false) {
        return Ok(Json(preview).into_response());
    }
    let url = Url::parse(&body.url).map_err(|_| ApiFailure::Validation("invalid web feed URL"))?;
    let value = Subscription::new_with_exact_url(
        SubscriptionId::new(),
        workspace,
        url,
        body.url.clone(),
        preview.title,
    )
    .map_err(|_| ApiFailure::Validation("web feed URL does not match parsed URL"))?;
    let recipe_json = serde_json::to_string(&body)
        .map_err(|_| ApiFailure::Validation("invalid web feed recipe"))?;
    s.repository
        .save_web_feed_subscription(value.clone(), recipe_json)
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)).into_response())
}
async fn get_web_feed_recipe<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WebFeedRecipeView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let (version, document) = s.repository.web_feed_recipe(subscription.id()).await?;
    let draft = serde_json::from_str(&document).map_err(|_| ApiFailure::Internal)?;
    Ok(Json(WebFeedRecipeView {
        subscription_id: id,
        version,
        draft,
    }))
}
async fn update_web_feed_recipe<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateWebFeedRecipeRequest>,
) -> Result<Json<WebFeedRecipeView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    if body.draft.workspace_id != subscription.workspace_id().as_uuid()
        || body.draft.url != subscription.source_url().as_str()
    {
        return Err(ApiFailure::Validation(
            "web feed recipe scope or URL does not match subscription",
        ));
    }
    s.discovery
        .preview_web_feed(&body.draft)
        .await
        .map_err(ApiFailure::Discovery)?;
    let document = serde_json::to_string(&body.draft)
        .map_err(|_| ApiFailure::Validation("invalid web feed recipe"))?;
    let version = s
        .repository
        .update_web_feed_recipe(subscription.id(), body.expected_version, document)
        .await?;
    Ok(Json(WebFeedRecipeView {
        subscription_id: id,
        version,
        draft: body.draft,
    }))
}
async fn visual_preview<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<VisualPreviewRequest>,
) -> Result<Json<VisualPreviewResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    owned_workspace(
        &s,
        WorkspaceId::from_uuid(body.workspace_id),
        actor.account.id,
    )
    .await?;
    let value = s
        .discovery
        .visual_preview(actor.session.id, &body)
        .await
        .map_err(ApiFailure::Discovery)?;
    Ok(Json(value))
}
async fn visual_select<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<VisualSelectionRequest>,
) -> Result<Json<VisualSelectionResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    owned_workspace(
        &s,
        WorkspaceId::from_uuid(body.workspace_id),
        actor.account.id,
    )
    .await?;
    let value = s
        .discovery
        .visual_select(actor.session.id, &body)
        .await
        .map_err(ApiFailure::Discovery)?;
    Ok(Json(value))
}
async fn pause<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ReasonCommand>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let sub = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    ReaderService::new(s.repository, s.reason_policy)
        .pause_subscription(
            sub.id(),
            body.reason,
            ActorId::from_uuid(actor.account.id.as_uuid()),
            Utc::now(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn resume<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let sub = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    ReaderService::new(s.repository, s.reason_policy)
        .resume_subscription(sub.id())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn restore_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let sub = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let value = ReaderService::new(s.repository.clone(), s.reason_policy)
        .restore_subscription(sub.id())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
async fn refresh_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let sub = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    s.repository.enqueue_subscription_refresh(&sub).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_articles<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<ArticleListQuery>,
) -> Result<Json<ArticlePageView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let values = s
        .repository
        .article_summary_page(
            workspace,
            article_page_request(
                query.view.as_deref().unwrap_or("all"),
                query.subscription_id.map(SubscriptionId::from_uuid),
                query.cursor.as_deref(),
                query.direction.as_deref(),
            )?,
        )
        .await?;
    Ok(Json(article_page_view(values)))
}
async fn get_article<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<ArticleView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let value = s
        .repository
        .article_presentation(workspace, ArticleId::from_uuid(id))
        .await?;
    Ok(Json(article_view(&value)))
}
async fn update_article<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
    Json(body): Json<ArticleStatePatch>,
) -> Result<Json<ArticleView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let article_id = ArticleId::from_uuid(id);
    let mut value = s.repository.article(workspace, article_id).await?;
    let revision = value.revision;
    if let Some(v) = body.read {
        value.state.read = v;
        if !v {
            value.state.protect_unread = true
        }
    }
    if let Some(v) = body.saved {
        value.state.saved = v
    }
    if let Some(v) = body.later {
        value.state.later = v
    }
    if let Some(v) = body.trash {
        value.state.trashed = v;
        if !v {
            value.state.protect_restored = true
        }
    }
    value.revision += 1;
    s.repository
        .save_article(workspace, Some(revision), value.clone())
        .await?;
    let presentation = s
        .repository
        .article_presentation(workspace, article_id)
        .await?;
    Ok(Json(article_view(&presentation)))
}
async fn refresh_full_text<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let article = s
        .repository
        .article(workspace, ArticleId::from_uuid(id))
        .await?;
    s.repository
        .enqueue_article_full_text_refresh(workspace, &article)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn mark_all_read<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<MarkAllReadRequest>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let subscription = body.subscription_id.map(SubscriptionId::from_uuid);
    if let Some(subscription) = subscription {
        let value = owned_subscription(&s, subscription, actor.account.id).await?;
        if value.workspace_id() != workspace {
            return Err(ApiFailure::Forbidden);
        }
    }
    let mut selected = Vec::new();
    for presentation in s
        .repository
        .article_summaries_by_workspace(workspace)
        .await?
    {
        if subscription
            .is_some_and(|subscription| !presentation.subscription_ids.contains(&subscription))
        {
            continue;
        }
        let mut value = presentation.article;
        let matches = match body.view.as_str() {
            "unread" => !value.state.read && !value.state.trashed,
            "all" => !value.state.trashed,
            "saved" => value.state.saved && !value.state.trashed,
            "later" => value.state.later && !value.state.trashed,
            "trash" => value.state.trashed,
            _ => return Err(ApiFailure::Validation("unknown article view")),
        };
        if matches && !value.state.read {
            value.state.read = true;
            value.revision += 1;
            selected.push(value)
        }
    }
    if selected.len() > s.bulk_mutation_limit {
        return Err(ApiFailure::Validation(
            "selection exceeds configured bulk mutation limit",
        ));
    }
    s.repository
        .mark_articles_read_atomic(workspace, selected)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_rules<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<Vec<RuleDraft>>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    Ok(Json(
        s.repository
            .rules_by_workspace(workspace)
            .await?
            .into_iter()
            .map(rule_draft)
            .collect(),
    ))
}
async fn preview_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<RuleListQuery>,
    Json(body): Json<RuleDraft>,
) -> Result<Json<RulePreviewResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let subscription = SubscriptionId::from_uuid(body.subscription_id);
    let sub = owned_subscription(&s, subscription, actor.account.id).await?;
    if sub.workspace_id() != workspace {
        return Err(ApiFailure::Forbidden);
    }
    let rule = parse_rule(None, &body)?;
    let values = s
        .repository
        .article_presentations_by_workspace(workspace)
        .await?;
    let mut matched = 0usize;
    let mut shared = 0usize;
    let mut total = 0usize;
    let mut samples = Vec::new();
    for value in values
        .into_iter()
        .filter(|value| value.subscription_ids.contains(&subscription))
    {
        total += 1;
        let text = value
            .safe_html
            .as_deref()
            .map(html_text_paragraphs)
            .map(|parts| parts.join("\n"));
        if rule.matches(&value.article.key.title, text.as_deref()) {
            matched += 1;
            if value.subscription_ids.len() > 1 {
                shared += 1
            }
            if samples.len() < 20 {
                samples.push(value.article.id.as_uuid())
            }
        }
    }
    Ok(Json(RulePreviewResponse {
        matched_articles: matched,
        shared_articles: shared,
        total_subscription_articles: total,
        sample_article_ids: samples,
    }))
}
async fn create_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<RuleListQuery>,
    Json(body): Json<RuleDraft>,
) -> Result<Json<RuleDraft>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let sub = owned_subscription(
        &s,
        SubscriptionId::from_uuid(body.subscription_id),
        actor.account.id,
    )
    .await?;
    if sub.workspace_id() != workspace {
        return Err(ApiFailure::Forbidden);
    }
    let value = parse_rule(None, &body)?;
    s.repository
        .save_rule(workspace, None, value.clone())
        .await?;
    Ok(Json(rule_draft(value)))
}
async fn update_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
    Json(body): Json<RuleDraft>,
) -> Result<Json<RuleDraft>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let current = s.repository.rule(workspace, RuleId::from_uuid(id)).await?;
    let sub = owned_subscription(
        &s,
        SubscriptionId::from_uuid(body.subscription_id),
        actor.account.id,
    )
    .await?;
    if sub.workspace_id() != workspace {
        return Err(ApiFailure::Forbidden);
    }
    let value = parse_rule(Some((RuleId::from_uuid(id), current.version + 1)), &body)?;
    s.repository
        .save_rule(workspace, Some(current.version), value.clone())
        .await?;
    Ok(Json(rule_draft(value)))
}
async fn delete_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let rule = s.repository.rule(workspace, RuleId::from_uuid(id)).await?;
    s.repository
        .delete_rule(workspace, rule.id, rule.version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn apply_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<RuleApplicationAccepted>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let rule = s.repository.rule(workspace, RuleId::from_uuid(id)).await?;
    let operation_id = s
        .repository
        .enqueue_rule_application(workspace, rule)
        .await?;
    Ok(Json(RuleApplicationAccepted {
        operation_id,
        status: "queued".into(),
    }))
}
async fn rule_application_status<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<RuleApplicationStatus>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let value = s
        .repository
        .rule_application_progress(workspace, id)
        .await?;
    Ok(Json(RuleApplicationStatus {
        operation_id: value.operation_id,
        status: value.status,
        evaluated: value.evaluated,
        cancel_reason: value.cancel_reason,
    }))
}

async fn import_opml<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<OpmlImportRequest>,
) -> Result<Json<OpmlImportResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(body.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let preview_id = Uuid::new_v5(&body.workspace_id, body.opml.as_bytes());
    if body.apply && body.preview_id != Some(preview_id) {
        return Err(ApiFailure::Validation(
            "OPML must be previewed unchanged before apply",
        ));
    }
    let outlines = opml_outlines(&body.opml)?;
    let existing = s.repository.subscriptions_by_workspace(workspace).await?;
    let mut values = Vec::new();
    let mut warnings = Vec::new();
    if opml_has_folders(&body.opml)? {
        warnings.push("OPML folders are flattened into the target workspace".to_owned())
    }
    let mut seen = std::collections::HashSet::new();
    for (url, title) in outlines {
        if !seen.insert(url.as_str().to_owned()) || existing.iter().any(|v| v.source_url() == &url)
        {
            warnings.push(format!("duplicate: {url}"));
            continue;
        }
        values.push(Subscription::new(
            SubscriptionId::new(),
            workspace,
            url,
            title,
        ))
    }
    let added = values.len();
    if body.apply {
        s.repository
            .import_subscriptions_atomic(workspace, values)
            .await?
    }
    Ok(Json(OpmlImportResponse {
        preview_id,
        subscriptions: added,
        warnings,
    }))
}
async fn export_opml<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<OpmlExportQuery>,
) -> Result<Json<String>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let subscriptions = s.repository.subscriptions_by_workspace(workspace).await?;
    let mut result=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><opml version=\"2.0\"><head><title>Subscriptions</title></head><body>");
    for value in subscriptions {
        result.push_str("<outline type=\"rss\" text=\"");
        result.push_str(&xml(value.title()));
        result.push_str("\" xmlUrl=\"");
        result.push_str(&xml(value.source_url().as_str()));
        result.push_str("\"/>")
    }
    result.push_str("</body></opml>");
    Ok(Json(result))
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
        "move_to_trash" => RuleAction::MoveToTrash,
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
            RuleAction::MoveToTrash => "move_to_trash",
        }
        .into(),
        enabled: value.enabled,
    }
}
fn opml_outlines(document: &str) -> Result<Vec<(Url, String)>, ApiFailure> {
    if document.contains("<!DOCTYPE") || document.contains("<!ENTITY") {
        return Err(ApiFailure::Validation(
            "OPML document declarations are forbidden",
        ));
    }
    let tree = roxmltree::Document::parse(document)
        .map_err(|_| ApiFailure::Validation("invalid OPML document"))?;
    if tree.root_element().tag_name().name() != "opml" {
        return Err(ApiFailure::Validation("root element must be opml"));
    }
    let mut values = Vec::new();
    for node in tree
        .descendants()
        .filter(|v| v.is_element() && v.tag_name().name() == "outline")
    {
        let Some(raw) = node.attribute("xmlUrl") else {
            continue;
        };
        let url = Url::parse(raw).map_err(|_| ApiFailure::Validation("invalid OPML URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ApiFailure::Validation("OPML URL must use HTTP or HTTPS"));
        }
        let title = node
            .attribute("text")
            .or_else(|| node.attribute("title"))
            .unwrap_or(raw)
            .to_owned();
        valid_name(&title)?;
        values.push((url, title))
    }
    Ok(values)
}
fn opml_has_folders(document: &str) -> Result<bool, ApiFailure> {
    let tree = roxmltree::Document::parse(document)
        .map_err(|_| ApiFailure::Validation("invalid OPML document"))?;
    Ok(tree.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "outline"
            && node.attribute("xmlUrl").is_none()
            && node.descendants().any(|child| {
                child.is_element()
                    && child.tag_name().name() == "outline"
                    && child.attribute("xmlUrl").is_some()
            })
    }))
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn workspace_view(value: &Workspace) -> WorkspaceView {
    let (reason, archive_reason_at) = match value.status() {
        WorkspaceStatus::Archived(event) => {
            (Some(event.reason.as_str().to_owned()), Some(event.at))
        }
        WorkspaceStatus::Active => (None, None),
    };
    WorkspaceView {
        id: value.id().as_uuid(),
        name: value.name().to_owned(),
        archived: !value.accepts_delivery(),
        archive_reason: reason,
        archive_reason_at,
    }
}
async fn subscription_views<R: ReaderRepository>(
    repository: &R,
    values: &[Subscription],
) -> Result<Vec<SubscriptionView>, ApiFailure> {
    if values.is_empty() {
        return Ok(vec![]);
    }
    let stats = repository
        .subscription_stats(values[0].workspace_id(), values)
        .await?;
    Ok(values
        .iter()
        .map(|value| {
            subscription_view(
                value,
                stats
                    .get(&value.id())
                    .unwrap_or(&reader_application::SubscriptionStats::default()),
            )
        })
        .collect())
}
async fn subscription_stats_for<R: ReaderRepository>(
    repository: &R,
    value: &Subscription,
) -> Result<reader_application::SubscriptionStats, ApiFailure> {
    let stats = repository
        .subscription_stats(value.workspace_id(), std::slice::from_ref(value))
        .await?;
    Ok(stats.get(&value.id()).cloned().unwrap_or_default())
}
fn subscription_view(
    value: &Subscription,
    stats: &reader_application::SubscriptionStats,
) -> SubscriptionView {
    let (status, reason, reason_at) = match value.status() {
        reader_core::SubscriptionStatus::Active => ("active", None, None),
        reader_core::SubscriptionStatus::Paused(event) => (
            "paused",
            Some(event.reason.as_str().to_owned()),
            Some(event.at),
        ),
        reader_core::SubscriptionStatus::Archived => ("archived", None, None),
    };
    SubscriptionView {
        id: value.id().as_uuid(),
        name: value.title().to_owned(),
        source_title: value.source_title().to_owned(),
        custom_name: value.custom_name().map(str::to_owned),
        personal_note: value.personal_note().to_owned(),
        source_url: value.source_url_exact().to_owned(),
        source_type: stats.source_type.clone(),
        created_at: value.created_at(),
        count: stats.article_count,
        unread_count: stats.unread_count,
        status: status.to_owned(),
        last_update: stats.last_success_at,
        last_error_at: stats.last_error_at,
        consecutive_failures: stats.consecutive_failures,
        needs_attention: stats.consecutive_failures >= 3 || stats.incomplete,
        attention_reason: if stats.consecutive_failures >= 3 {
            stats
                .error
                .clone()
                .or_else(|| Some("Three consecutive refresh failures".to_owned()))
        } else if stats.incomplete {
            Some("Source refresh is incomplete and requires continuation".to_owned())
        } else {
            None
        },
        incomplete: stats.incomplete,
        continuation: stats.continuation.clone(),
        error: stats.error.clone(),
        editable_web_feed: stats.editable_web_feed,
        reason,
        reason_at,
    }
}
fn article_view(value: &reader_application::ArticlePresentation) -> ArticleView {
    let mut view = article_domain_view(&value.article);
    view.sources = value.subscription_titles.clone();
    view.subscription_ids = value
        .subscription_ids
        .iter()
        .map(|id| id.as_uuid())
        .collect();
    view.source = view
        .sources
        .first()
        .cloned()
        .unwrap_or_else(|| "Unknown source".to_owned());
    if let Some(content) = &value.safe_html {
        let rendered = reader_web_runtime::SafeRenderedContent::from_untrusted_html_with_base(
            content,
            Some(&view.url),
        );
        view.body_html = Some(rendered.html().to_owned());
        view.body.clear();
    }
    view.full_text = value.full_text_status.to_owned();
    view.full_text_reason = value.failure_reason.clone();
    view
}

fn article_page_request(
    view: &str,
    subscription_id: Option<SubscriptionId>,
    cursor: Option<&str>,
    direction: Option<&str>,
) -> Result<ArticlePageRequest, ApiFailure> {
    if !matches!(view, "all" | "unread" | "saved" | "later" | "trash") {
        return Err(ApiFailure::Validation("invalid article view"));
    }
    let direction = match direction.unwrap_or("older") {
        "older" => ArticlePageDirection::Older,
        "newer" => ArticlePageDirection::Newer,
        _ => return Err(ApiFailure::Validation("invalid article page direction")),
    };
    let cursor = cursor.map(decode_article_cursor).transpose()?;
    if direction == ArticlePageDirection::Newer && cursor.is_none() {
        return Err(ApiFailure::Validation("newer direction requires a cursor"));
    }
    Ok(ArticlePageRequest {
        view: view.to_owned(),
        subscription_id,
        cursor,
        direction,
        limit: ARTICLE_PAGE_SIZE,
    })
}

fn article_page_view(page: reader_application::ArticlePage) -> ArticlePageView {
    let newer_cursor = page
        .has_newer
        .then(|| page.articles.first().map(article_cursor))
        .flatten();
    let older_cursor = page
        .has_older
        .then(|| page.articles.last().map(article_cursor))
        .flatten();
    ArticlePageView {
        articles: page.articles.iter().map(article_view).collect(),
        total: page.total,
        unread_total: page.unread_total,
        newer_cursor,
        older_cursor,
    }
}

fn article_cursor(value: &reader_application::ArticlePresentation) -> String {
    format!(
        "{}.{}",
        value.article.first_arrived_at.timestamp_micros(),
        value.article.id.as_uuid()
    )
}

fn decode_article_cursor(value: &str) -> Result<ArticlePageCursor, ApiFailure> {
    let (micros, id) = value
        .split_once('.')
        .ok_or(ApiFailure::Validation("invalid article cursor"))?;
    let micros = micros
        .parse::<i64>()
        .map_err(|_| ApiFailure::Validation("invalid article cursor"))?;
    let arrived_at = chrono::DateTime::from_timestamp_micros(micros)
        .ok_or(ApiFailure::Validation("invalid article cursor"))?;
    let article_id = Uuid::parse_str(id)
        .map(ArticleId::from_uuid)
        .map_err(|_| ApiFailure::Validation("invalid article cursor"))?;
    Ok(ArticlePageCursor {
        arrived_at,
        article_id,
    })
}
fn article_domain_view(value: &reader_core::Article) -> ArticleView {
    let excerpt = value.key.description.clone().unwrap_or_default();
    let url = value
        .key
        .location
        .exact_url()
        .unwrap_or_default()
        .to_owned();
    ArticleView {
        id: value.id.as_uuid(),
        url,
        source: "Unknown source".to_owned(),
        sources: Vec::new(),
        subscription_ids: Vec::new(),
        title: value.key.title.clone(),
        excerpt: excerpt.clone(),
        body: if excerpt.is_empty() {
            Vec::new()
        } else {
            vec![excerpt]
        },
        body_html: None,
        author: None,
        age: value.first_arrived_at.to_rfc3339(),
        read: value.state.read,
        saved: value.state.saved,
        later: value.state.later,
        trash: value.state.trashed,
        full_text: "pending".to_owned(),
        full_text_reason: None,
    }
}

fn html_text_paragraphs(value: &str) -> Vec<String> {
    let normalized = value
        .replace("</p>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("</li>", "\n");
    normalized
        .lines()
        .filter_map(|line| {
            let mut text = String::new();
            let mut tag = false;
            for ch in line.chars() {
                match ch {
                    '<' => tag = true,
                    '>' => tag = false,
                    _ if !tag => text.push(ch),
                    _ => {}
                }
            }
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        })
        .collect()
}

#[derive(Debug)]
enum ApiFailure {
    Command(CommandError),
    Auth(AuthError),
    Repository(RepositoryError),
    Unauthorized,
    Forbidden,
    Csrf,
    Validation(&'static str),
    ExistingSubscription(bool),
    Discovery(String),
    NoWorkspace,
    RateLimited,
    Internal,
}
impl From<CommandError> for ApiFailure {
    fn from(v: CommandError) -> Self {
        Self::Command(v)
    }
}
impl From<AuthError> for ApiFailure {
    fn from(v: AuthError) -> Self {
        Self::Auth(v)
    }
}
impl From<RepositoryError> for ApiFailure {
    fn from(v: RepositoryError) -> Self {
        Self::Repository(v)
    }
}
impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required".into(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "resource belongs to another account".into(),
            ),
            Self::Csrf => (
                StatusCode::FORBIDDEN,
                "csrf_rejected",
                "request origin is not allowed".into(),
            ),
            Self::Validation(v) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                v.into(),
            ),
            Self::ExistingSubscription(true) => (
                StatusCode::CONFLICT,
                "subscription_archived",
                "This source is already archived in this workspace. Restore it in Settings.".into(),
            ),
            Self::ExistingSubscription(false) => (
                StatusCode::CONFLICT,
                "subscription_exists",
                "This source is already subscribed in this workspace.".into(),
            ),
            Self::Discovery(v) => (StatusCode::UNPROCESSABLE_ENTITY, "discovery_failed", v),
            Self::NoWorkspace => (
                StatusCode::CONFLICT,
                "workspace_required",
                "account has no workspace".into(),
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many sign-in attempts".into(),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "internal response error".into(),
            ),
            Self::Auth(e) => {
                let (status, code) = match e {
                    AuthError::InvalidCredentials | AuthError::InvalidToken => {
                        (StatusCode::UNAUTHORIZED, "invalid_credentials")
                    }
                    AuthError::AdminRequired => (StatusCode::FORBIDDEN, "admin_required"),
                    AuthError::UsernameExists => (StatusCode::CONFLICT, "username_exists"),
                    AuthError::Repository(RepositoryError::NotFound) => {
                        (StatusCode::NOT_FOUND, "not_found")
                    }
                    AuthError::Repository(RepositoryError::Conflict) => {
                        (StatusCode::CONFLICT, "conflict")
                    }
                    AuthError::Repository(RepositoryError::Storage(_)) | AuthError::Hashing => {
                        (StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
                    }
                    _ => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_request"),
                };
                (status, code, e.to_string())
            }
            Self::Repository(e) => repository_failure(e),
            Self::Command(e) => match e {
                CommandError::InvalidReason(_) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_reason",
                    e.to_string(),
                ),
                CommandError::WorkspaceArchived => {
                    (StatusCode::CONFLICT, "workspace_archived", e.to_string())
                }
                CommandError::Repository(r) => repository_failure(r),
            },
        };
        (status, Json(ApiError { code, message })).into_response()
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
