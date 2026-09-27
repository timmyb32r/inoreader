use super::*;
use reader_ai::{AiError, AiProfile, AiService, ArticleChat};
use serde::Deserialize;

pub(super) fn routes<R: ReaderRepository + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route(
            "/api/ai/profile",
            get(profile::<R>).put(save_key::<R>).delete(delete_key::<R>),
        )
        .route("/api/ai/balance", post(balance::<R>))
        .route(
            "/api/articles/{id}/translations",
            get(translations::<R>).post(translate::<R>),
        )
        .route("/api/articles/{id}/chat", post(start::<R>))
        .route("/api/articles/{id}/chats", get(versions::<R>))
        .route("/api/ai/chats/{id}", get(chat::<R>))
        .route("/api/ai/chats/{id}/messages", post(message::<R>))
        .route("/api/ai/chats/{id}/retry", post(retry::<R>))
        .route("/api/ai/chats/{id}/stop", post(stop::<R>))
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
