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
