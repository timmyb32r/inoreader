use super::*;
use reader_wiki::{Limits, Store, Write};
use serde::Deserialize;
pub(super) fn routes<R: ReaderRepository + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route(
            "/api/wiki/{namespace}/collections/{kind}",
            get(collection::<R>),
        )
        .route(
            "/api/wiki/{namespace}/pages/{page}/organization",
            get(organization::<R>),
        )
        .route(
            "/api/wiki/{namespace}/pages/{page}/favorite",
            put(favorite::<R>),
        )
        .route(
            "/api/wiki/{namespace}/subscription-root",
            post(subscription_root::<R>),
        )
        .route(
            "/api/wiki/{namespace}/pages/{page}/history/{revision}",
            get(revision::<R>),
        )
        .route("/api/wiki/{namespace}/pages/{page}/links", get(links::<R>))
        .route("/api/wiki/limits", get(limits::<R>))
        .route(
            "/api/wiki/namespaces",
            get(namespaces::<R>).post(create::<R>),
        )
        .route("/api/wiki/{namespace}", get(namespace::<R>))
        .route(
            "/api/wiki/{namespace}/pages",
            get(pages::<R>).post(write::<R>),
        )
        .route("/api/wiki/{namespace}/resolve", get(resolve::<R>))
        .route("/api/wiki/{namespace}/pages/{page}", get(page::<R>))
        .route(
            "/api/wiki/{namespace}/pages/{page}/history",
            get(history::<R>),
        )
        .route(
            "/api/wiki/{namespace}/drafts/{id}",
            get(draft::<R>).put(save_draft::<R>).delete(discard::<R>),
        )
        .route(
            "/api/wiki/{namespace}/members",
            get(members::<R>).put(member::<R>),
        )
        .route(
            "/api/subscriptions/{id}/wiki",
            get(binding::<R>).put(bind::<R>),
        )
        .layer(middleware::from_fn(no_store))
}
async fn no_store(request: axum::extract::Request, next: middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
fn service<R>(s: &AppState<R>) -> Result<(&dyn Store, &Limits), ApiFailure> {
    s.wiki
        .as_ref()
        .map(|(s, l)| (s.as_ref(), l))
        .ok_or(ApiFailure::Internal)
}
#[derive(Deserialize, Default)]
struct Listing {
    #[serde(default)]
    offset: u32,
    #[serde(default)]
    search: String,
    #[serde(default)]
    trash: bool,
    #[serde(default)]
    name: String,
}
async fn limits<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
) -> Result<Json<WikiLimits>, ApiFailure> {
    auth(&s, &h).await?;
    Ok(Json(service(&s)?.1.input().clone()))
}
async fn namespaces<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Query(q): Query<Listing>,
) -> Result<Json<WikiNamespaces>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .namespaces(a, q.offset)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn create<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Json(q): Json<WikiCreateNamespace>,
) -> Result<Json<WikiNamespace>, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .create_namespace(a, q.id, &q.name)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn namespace<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(n): Path<Uuid>,
) -> Result<Json<WikiNamespace>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .namespace(a, n)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn pages<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(n): Path<Uuid>,
    Query(q): Query<Listing>,
) -> Result<Json<WikiPages>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .pages(a, n, &q.search, q.trash, q.offset)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn page<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, p)): Path<(Uuid, Uuid)>,
) -> Result<Json<WikiPage>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .page(a, n, p)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn resolve<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(n): Path<Uuid>,
    Query(q): Query<Listing>,
) -> Result<Json<WikiPage>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .resolve(a, n, &q.name)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn write<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(n): Path<Uuid>,
    Json(q): Json<WikiWrite>,
) -> Result<Json<WikiPage>, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    let (store, limits) = service(&s)?;
    let command = Write::new(q, limits).map_err(ApiFailure::Wiki)?;
    Ok(Json(
        store.write(a, n, command).await.map_err(ApiFailure::Wiki)?,
    ))
}
async fn history<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, p)): Path<(Uuid, Uuid)>,
    Query(q): Query<Listing>,
) -> Result<Json<WikiHistory>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .history(a, n, p, q.offset)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn draft<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<WikiDraftResponse>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(WikiDraftResponse {
        draft: service(&s)?
            .0
            .draft(a, n, id)
            .await
            .map_err(ApiFailure::Wiki)?,
    }))
}
async fn save_draft<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, id)): Path<(Uuid, Uuid)>,
    Json(d): Json<WikiDraft>,
) -> Result<Json<WikiDraft>, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    if d.id != id {
        return Err(ApiFailure::Validation("draft id mismatch"));
    }
    Ok(Json(
        service(&s)?
            .0
            .save_draft(a, n, d)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn discard<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, id)): Path<(Uuid, Uuid)>,
    Json(q): Json<WikiDiscardDraft>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    service(&s)?
        .0
        .discard_draft(a, n, id, q.revision)
        .await
        .map_err(ApiFailure::Wiki)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn members<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(n): Path<Uuid>,
    Query(q): Query<Listing>,
) -> Result<Json<WikiMembers>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .members(a, n, q.offset)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn member<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(n): Path<Uuid>,
    Json(q): Json<WikiMemberCommand>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    service(&s)?
        .0
        .set_member(a, n, &q.username, q.role)
        .await
        .map_err(ApiFailure::Wiki)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn binding<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WikiBinding>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .binding(a, id)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn bind<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
    Json(q): Json<WikiBindCommand>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    service(&s)?
        .0
        .bind(a, id, q.target.map(|t| (t.namespace, t.page)))
        .await
        .map_err(ApiFailure::Wiki)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn links<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, p)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<WikiLink>>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .links(a, n, p)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}

async fn revision<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((n, p, v)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<WikiRevision>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .revision(a, n, p, v)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}

async fn collection<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((ns, kind)): Path<(Uuid, reader_wiki::Collection)>,
    Query(q): Query<Listing>,
) -> Result<Json<WikiPages>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .collection(a, ns, kind, q.offset)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn organization<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((ns, page)): Path<(Uuid, Uuid)>,
    Query(q): Query<Listing>,
) -> Result<Json<WikiOrganization>, ApiFailure> {
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .organization(a, ns, page, q.offset)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
async fn favorite<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path((ns, page)): Path<(Uuid, Uuid)>,
    Json(q): Json<WikiFavorite>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    service(&s)?
        .0
        .favorite(a, ns, page, q.favorite)
        .await
        .map_err(ApiFailure::Wiki)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn subscription_root<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    h: HeaderMap,
    Path(ns): Path<Uuid>,
) -> Result<Json<WikiPage>, ApiFailure> {
    csrf(&s, &h)?;
    let a = auth(&s, &h).await?.account.id.as_uuid();
    Ok(Json(
        service(&s)?
            .0
            .subscription_root(a, ns)
            .await
            .map_err(ApiFailure::Wiki)?,
    ))
}
