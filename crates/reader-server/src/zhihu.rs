use super::*;
use reader_application::ZhihuProfilePort;
pub(super) fn routes<R: ReaderRepository + 'static>() -> Router<AppState<R>> {
    Router::new()
        .route(
            "/api/profile/zhihu",
            get(status::<R>).put(save::<R>).delete(remove::<R>),
        )
        .route("/api/profile/zhihu/check", post(check::<R>))
}
fn service<R>(s: &AppState<R>) -> Result<&dyn ZhihuProfilePort, ApiFailure> {
    s.zhihu.as_deref().ok_or(ApiFailure::Validation(
        "Zhihu session encryption is not configured",
    ))
}
fn failure(e: String) -> ApiFailure {
    ApiFailure::Discovery(e)
}
async fn status<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<ZhihuProfileView>, ApiFailure> {
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    Ok(Json(ZhihuProfileView {
        available: s.zhihu.is_some(),
        configured: match &s.zhihu {
            Some(v) => v.configured(owner).await.map_err(failure)?,
            None => false,
        },
    }))
}
async fn save<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(q): Json<ZhihuSessionCommand>,
) -> Result<Json<ZhihuProfileView>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    service(&s)?.save(owner, q.cookies).await.map_err(failure)?;
    Ok(Json(ZhihuProfileView {
        available: true,
        configured: true,
    }))
}
async fn check<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<ZhihuProfileView>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    service(&s)?.check(owner).await.map_err(failure)?;
    Ok(Json(ZhihuProfileView {
        available: true,
        configured: true,
    }))
}
async fn remove<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
) -> Result<Json<ZhihuProfileView>, ApiFailure> {
    csrf(&s, &headers)?;
    let owner = auth(&s, &headers).await?.account.id.as_uuid();
    service(&s)?.remove(owner).await.map_err(failure)?;
    Ok(Json(ZhihuProfileView {
        available: true,
        configured: false,
    }))
}
