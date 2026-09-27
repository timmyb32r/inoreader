use super::*;
use reader_ai::{DefinitionState, DefinitionsView};
use reader_glossary::{ChannelStatus, GlossaryError, GlossaryService};
use serde::Deserialize;

pub(super) fn routes<R: ReaderRepository + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route(
            "/api/glossary/channel",
            get(status::<R>).put(configure::<R>),
        )
        .route("/api/glossary/channel/sync", post(sync::<R>))
        .route(
            "/api/articles/{id}/definitions",
            get(definitions::<R>).post(define::<R>),
        )
}
impl From<GlossaryError> for ApiFailure {
    fn from(value: GlossaryError) -> Self {
        Self::Glossary(value)
    }
}
fn service<R>(s: &AppState<R>) -> Result<&GlossaryService, ApiFailure> {
    s.glossary
        .as_deref()
        .ok_or(GlossaryError::NotConnected.into())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryScope {
    workspace_id: Uuid,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scope {
    workspace_id: Uuid,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Connection {
    workspace_id: Uuid,
    token: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Generation {
    workspace_id: Uuid,
    operation_id: Uuid,
    #[serde(default)]
    regenerate: bool,
}
async fn status<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(q): Query<QueryScope>,
) -> Result<Json<ChannelStatus>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.status(owner, q.workspace_id).await?))
}
async fn configure<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(q): Json<Connection>,
) -> Result<Json<ChannelStatus>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .configure(owner, q.workspace_id, q.token)
            .await?,
    ))
}
async fn sync<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(q): Json<Scope>,
) -> Result<Json<ChannelStatus>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(service(&s)?.sync(owner, q.workspace_id).await?))
}
async fn view<R: ReaderRepository + 'static>(
    s: &AppState<R>,
    owner: Uuid,
    workspace: Uuid,
    article: Uuid,
) -> Result<DefinitionsView, ApiFailure> {
    let channel = service(s)?.status(owner, workspace).await?;
    let ai = s.ai.as_deref().ok_or(reader_ai::AiError::Unavailable)?;
    let job = ai.definitions(owner, workspace, article).await?;
    let names = match job.as_ref().map(|j| &j.state) {
        Some(DefinitionState::Completed { result }) => result
            .entities
            .iter()
            .map(|e| e.name().to_owned())
            .collect(),
        _ => Vec::new(),
    };
    let known = service(s)?.lookup(owner, workspace, &names).await?;
    Ok(DefinitionsView {
        job,
        channel,
        known,
    })
}
async fn definitions<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(q): Query<QueryScope>,
) -> Result<Json<DefinitionsView>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(view(&s, owner, q.workspace_id, id).await?))
}
async fn define<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(q): Json<Generation>,
) -> Result<Json<DefinitionsView>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    let channel = service(&s)?.status(owner, q.workspace_id).await?;
    if !channel.index_ready || !channel.generation_allowed {
        return Err(GlossaryError::NotConnected.into());
    }
    s.ai.as_deref()
        .ok_or(reader_ai::AiError::Unavailable)?
        .define(owner, q.workspace_id, id, q.operation_id, q.regenerate)
        .await?;
    Ok(Json(view(&s, owner, q.workspace_id, id).await?))
}
