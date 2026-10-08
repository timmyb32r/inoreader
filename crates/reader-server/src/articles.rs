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
            )?
            .with_read_period(read_period(query.read_from, query.read_until)?)
            .map_err(ApiFailure::Validation)?,
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
    let owned = reader_application::article_commands::OwnedWorkspace::resolve(
        s.repository.as_ref(),
        actor.account.id,
        workspace,
    )
    .await?;
    let scope = reader_application::ArticleScope::from_wire(
        &body.view,
        body.subscription_id.map(SubscriptionId::from_uuid),
    )
    .map_err(ApiFailure::Validation)?;
    let batch = s.bulk_mutation_batch;
    reader_application::article_commands::mark_articles_read(
        s.repository.as_ref(),
        &owned,
        scope,
        batch,
    )
    .await
    .map_err(ApiFailure::Repository)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub(super) struct ReadingActivityQuery {
    timezone: String,
}
pub(super) async fn reading_activity<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<ReadingActivityQuery>,
) -> Result<Response, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let days = s
        .repository
        .reading_activity(workspace, &query.timezone)
        .await?
        .ok_or(ApiFailure::Validation("unsupported activity timezone"))?;
    let mut response = Json(ReadingActivityView { days }).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceActivityQuery {
    timezone: String,
    from: chrono::NaiveDate,
    until: chrono::NaiveDate,
}
pub(super) async fn source_activity<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<SourceActivityQuery>,
) -> Result<Response, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let period = reader_application::SourceActivityPeriod::new(query.from, query.until)
        .map_err(ApiFailure::Validation)?;
    let days = s
        .repository
        .source_activity(workspace, &query.timezone, period)
        .await?
        .ok_or(ApiFailure::Validation("unsupported activity timezone"))?;
    let mut response = Json(SourceActivityView { days }).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
