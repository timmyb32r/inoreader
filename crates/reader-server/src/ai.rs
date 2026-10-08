use super::*;
use reader_ai::{AiError, AiProfile, AiService, ArticleChat};
use serde::Deserialize;

pub(super) fn routes<R: ReaderRepository + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/api/ai/statistics", get(statistics::<R>))
        .route("/api/ai/smart-feed", get(smart_feed::<R>))
        .route("/api/ai/random-feed", get(random_feed::<R>))
        .route("/api/ai/interests", axum::routing::put(save_interests::<R>))
        .route(
            "/api/ai/profile",
            get(profile::<R>).put(save_key::<R>).delete(delete_key::<R>),
        )
        .route("/api/ai/balance", post(balance::<R>))
        .route("/api/ai/models", axum::routing::put(save_models::<R>))
        .route(
            "/api/articles/{id}/translations",
            get(translations::<R>).post(translate::<R>),
        )
        .route("/api/articles/{id}/chat", post(start::<R>))
        .route("/api/articles/{id}/chats", get(versions::<R>))
        .route("/api/ai/chats/{id}", get(chat::<R>))
        .route("/api/ai/chats/{id}/changes", get(poll_chat::<R>))
        .route("/api/ai/chats/{id}/messages", post(message::<R>))
        .route("/api/ai/chats/{id}/retry", post(retry::<R>))
        .route("/api/ai/chats/{id}/stop", post(stop::<R>))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatisticsQuery {
    from: chrono::NaiveDate,
    until: chrono::NaiveDate,
    bucket: reader_ai::StatisticsBucket,
}
async fn statistics<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(q): Query<StatisticsQuery>,
) -> Result<Response, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    if actor.account.username != "timmyb32r" {
        return Err(ApiFailure::Forbidden);
    }
    let range = reader_ai::StatisticsRange::new(q.from, q.until, q.bucket)?;
    let mut response = Json(
        service(&s)?
            .request_statistics(actor.account.id.as_uuid(), &range)
            .await?,
    )
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SmartQuery {
    workspace_id: Uuid,
    cursor: Option<String>,
    show_hidden: Option<bool>,
}
async fn smart_feed<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(q): Query<SmartQuery>,
) -> Result<Response, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id;
    owned_workspace(&s, WorkspaceId::from_uuid(q.workspace_id), owner).await?;
    let mut response = Json(
        service(&s)?
            .smart_feed(
                owner.as_uuid(),
                q.workspace_id,
                q.cursor.as_deref(),
                ARTICLE_PAGE_SIZE as u32,
                q.show_hidden.unwrap_or(false),
            )
            .await?,
    )
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RandomQuery {
    workspace_id: Uuid,
    seed: Uuid,
    cursor: Option<String>,
}
async fn random_feed<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(q): Query<RandomQuery>,
) -> Result<Response, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id;
    owned_workspace(&s, WorkspaceId::from_uuid(q.workspace_id), owner).await?;
    let mut response = Json(
        service(&s)?
            .random_feed(
                owner.as_uuid(),
                q.workspace_id,
                q.cursor.as_deref(),
                ARTICLE_PAGE_SIZE as u32,
                q.seed,
            )
            .await?,
    )
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
async fn save_interests<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(profile): Json<reader_ai::InterestProfile>,
) -> Result<Json<reader_ai::InterestProfile>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?.save_interest_profile(owner, profile).await?,
    ))
}
impl From<AiError> for ApiFailure {
    fn from(value: AiError) -> Self {
        Self::Ai(value)
    }
}
fn service<R>(state: &AppState<R>) -> Result<&AiService, ApiFailure> {
    state
        .ai
        .as_deref()
        .ok_or(ApiFailure::Ai(AiError::Unavailable))
}
async fn profile<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<AiProfile>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    let Some(ai) = s.ai.as_ref() else {
        return Ok(Json(AiProfile {
            models: reader_ai::ModelPreferences::default(),
            spending: None,
            configured: false,
            enabled: false,
            balance: None,
            error: None,
            availability_reason: Some("DeepSeek server configuration is not available".into()),
        }));
    };
    Ok(Json(ai.profile(owner).await?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyInput {
    api_key: String,
}
async fn save_key<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<KeyInput>,
) -> Result<Json<AiProfile>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.save_key(owner, body.api_key).await?))
}
async fn save_models<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(models): Json<reader_ai::ModelPreferences>,
) -> Result<Json<AiProfile>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.save_models(owner, models).await?))
}
async fn delete_key<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<AiProfile>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.delete_key(owner).await?))
}
async fn balance<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<AiProfile>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.balance(owner).await?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartInput {
    workspace_id: Uuid,
    operation_id: Uuid,
    #[serde(default)]
    regenerate: bool,
}
async fn start<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<StartInput>,
) -> Result<Json<ArticleChat>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .start(
                owner,
                body.workspace_id,
                id,
                body.operation_id,
                body.regenerate,
            )
            .await?,
    ))
}
async fn versions<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<Vec<ArticleChat>>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?.chats(owner, query.workspace_id, id).await?,
    ))
}
async fn chat<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ArticleChat>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.chat(owner, id).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PollQuery {
    after: Option<String>,
}
async fn poll_chat<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<PollQuery>,
) -> Result<Json<reader_ai::ChatPoll>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .poll_chat(owner, id, query.after.as_deref())
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MessageInput {
    operation_id: Uuid,
    content: String,
}
async fn message<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<MessageInput>,
) -> Result<Json<ArticleChat>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .message(owner, id, body.operation_id, body.content)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetryInput {
    operation_id: Uuid,
}
async fn retry<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RetryInput>,
) -> Result<Json<ArticleChat>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?.retry(owner, id, body.operation_id).await?,
    ))
}
async fn stop<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ArticleChat>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.stop(owner, id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranslationRequest {
    workspace_id: Uuid,
    operation_id: Uuid,
    source: String,
}
async fn translate<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<TranslationRequest>,
) -> Result<Json<reader_ai::ParagraphJob>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .translate(owner, body.workspace_id, id, body.operation_id, body.source)
            .await?,
    ))
}
async fn translations<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<Vec<reader_ai::ParagraphJob>>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .translations(owner, query.workspace_id, id)
            .await?,
    ))
}
