use super::*;

pub(super) async fn list_articles<R: ReaderRepository + 'static>(
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
                query.view.as_deref().unwrap_or("feed"),
                query.subscription_id.map(SubscriptionId::from_uuid),
                query.cursor.as_deref(),
                query.direction.as_deref(),
            )?,
        )
        .await?;
    Ok(Json(article_page_view(values)))
}
pub(super) async fn get_article<R: ReaderRepository + 'static>(
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
pub(super) async fn update_article<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
    Json(body): Json<ArticleStatePatch>,
) -> Result<Json<ArticleView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    let scope = reader_application::article_commands::OwnedWorkspace::resolve(
        s.repository.as_ref(),
        actor.account.id,
        workspace,
    )
    .await?;
    let article_id = ArticleId::from_uuid(id);
    reader_application::article_commands::update_article(
        s.repository.as_ref(),
        &scope,
        article_id,
        reader_application::article_commands::ArticlePatch {
            read: body.read,
            later: body.later,
        },
    )
    .await?;
    let presentation = s
        .repository
        .article_presentation(workspace, article_id)
        .await?;
    Ok(Json(article_view(&presentation)))
}
pub(super) async fn refresh_full_text<R: ReaderRepository + 'static>(
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
pub(super) async fn mark_all_read<R: ReaderRepository + 'static>(
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
    article_page_request(&body.view, subscription, None, None)?;
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
            "feed" => !value.state.read,
            "subscription" => true,
            "later" => value.state.later,
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
