use super::*;

pub(super) async fn discover_feed<R: ReaderRepository + 'static>(
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
pub(super) async fn web_feed_recipe<R: ReaderRepository + 'static>(
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
pub(super) async fn get_web_feed_recipe<R: ReaderRepository + 'static>(
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
pub(super) async fn update_web_feed_recipe<R: ReaderRepository + 'static>(
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
pub(super) async fn visual_preview<R: ReaderRepository + 'static>(
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
pub(super) async fn visual_select<R: ReaderRepository + 'static>(
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
