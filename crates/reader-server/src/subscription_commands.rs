use super::*;

pub(super) async fn pause<R: ReaderRepository + 'static>(
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
pub(super) async fn resume<R: ReaderRepository + 'static>(
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
pub(super) async fn restore_subscription<R: ReaderRepository + 'static>(
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
pub(super) async fn refresh_subscription<R: ReaderRepository + 'static>(
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
