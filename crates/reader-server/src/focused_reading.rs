use super::*;
use reader_application::{CompleteReading, ReadingCompletion, ReadingState};

pub(super) async fn state<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<ReadingState>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    Ok(Json(
        s.repository
            .reading_state(
                actor.account.id,
                WorkspaceId::from_uuid(query.workspace_id),
                ArticleId::from_uuid(id),
            )
            .await?,
    ))
}
pub(super) async fn complete<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
    Json(command): Json<CompleteReading>,
) -> Result<Json<ReadingCompletion>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    Ok(Json(
        s.repository
            .complete_reading(
                actor.account.id,
                WorkspaceId::from_uuid(query.workspace_id),
                ArticleId::from_uuid(id),
                command,
            )
            .await?,
    ))
}
pub(super) async fn undo<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path((id, operation)): Path<(Uuid, Uuid)>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<ReadingCompletion>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    Ok(Json(
        s.repository
            .undo_reading(
                actor.account.id,
                WorkspaceId::from_uuid(query.workspace_id),
                ArticleId::from_uuid(id),
                operation,
            )
            .await?,
    ))
}

/// Uses the stored article's exact arrival timestamp, never a JS date or narrowed cursor.
pub(super) async fn next<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<ArticlePageView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let article = s
        .repository
        .article(workspace, ArticleId::from_uuid(id))
        .await?;
    let request = reader_application::ArticlePageRequest::new(
        reader_application::ArticleScope::Feed,
        Some(reader_application::ArticlePageCursor {
            arrived_at: article.first_arrived_at,
            article_id: article.id,
        }),
        reader_application::ArticlePageDirection::Older,
        reader_application::SelectionLimit::new(ARTICLE_PAGE_SIZE)
            .map_err(ApiFailure::Validation)?,
    )
    .map_err(ApiFailure::Validation)?;
    Ok(Json(article_page_view(
        s.repository
            .article_summary_page(workspace, request)
            .await?,
    )))
}

/// Project is derived from an owned anchor article, never a caller-supplied owner.
pub(super) async fn commits<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<ArticleListQuery>,
) -> Result<Json<ArticlePageView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let article = s
        .repository
        .article(workspace, ArticleId::from_uuid(id))
        .await?;
    let project = reader_application::CommitProject::from_url(
        article
            .key
            .location
            .exact_url()
            .ok_or(ApiFailure::Validation("commit has no URL"))?,
    )
    .map_err(ApiFailure::Validation)?;
    let request = article_page_request(
        query.view.as_deref().unwrap_or("feed"),
        query.subscription_id.map(SubscriptionId::from_uuid),
        query.cursor.as_deref(),
        query.direction.as_deref(),
    )?
    .with_read_period(read_period(query.read_from, query.read_until)?)
    .map_err(ApiFailure::Validation)?
    .with_commit_project(project)
    .map_err(ApiFailure::Validation)?;
    Ok(Json(article_page_view(
        s.repository
            .article_summary_page(workspace, request)
            .await?,
    )))
}
