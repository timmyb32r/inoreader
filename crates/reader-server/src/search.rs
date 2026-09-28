use super::*;
use reader_application::{SearchKind, SearchRequest};
use serde::Deserialize;
pub(super) fn routes<R: ReaderRepository + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route("/api/search", get(search::<R>))
        .route("/api/search/limits", get(limits::<R>))
        .layer(middleware::from_fn(no_store))
}
async fn no_store(request: axum::extract::Request, next: middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryInput {
    q: String,
    kind: SearchKind,
    namespace: Option<Uuid>,
    subscription: Option<Uuid>,
    #[serde(default)]
    offset: u32,
}
async fn limits<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
) -> Result<Json<SearchLimitsView>, ApiFailure> {
    auth(&s, &h).await?;
    Ok(Json(
        s.search
            .as_ref()
            .ok_or(ApiFailure::Internal)?
            .1
            .input()
            .clone(),
    ))
}
async fn search<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Query(q): Query<QueryInput>,
) -> Result<Json<SearchPage>, ApiFailure> {
    let actor = auth(&s, &h).await?.account.id.as_uuid();
    let (store, limits) = s.search.as_ref().ok_or(ApiFailure::Internal)?;
    let request = SearchRequest::new(limits, q.q, q.kind, q.namespace, q.subscription, q.offset)
        .map_err(ApiFailure::Validation)?;
    Ok(Json(store.search(actor, request).await?))
}
