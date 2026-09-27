use super::*;

pub(super) async fn create_workspace<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Result<Json<WorkspaceView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    valid_name(&body.name)?;
    let value = Workspace::new(WorkspaceId::new(), actor.account.id, body.name);
    s.repository.save_workspace(None, value.clone()).await?;
    Ok(Json(workspace_view(&value)))
}
pub(super) async fn rename_workspace<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RenameWorkspaceRequest>,
) -> Result<Json<WorkspaceView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    valid_name(&body.name)?;
    let mut value = owned_workspace(&s, WorkspaceId::from_uuid(id), actor.account.id).await?;
    let revision = value.revision();
    value.rename(body.name);
    s.repository
        .save_workspace(Some(revision), value.clone())
        .await?;
    Ok(Json(workspace_view(&value)))
}
pub(super) async fn archive<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ReasonCommand>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    owned_workspace(&s, WorkspaceId::from_uuid(id), actor.account.id).await?;
    ReaderService::new(s.repository, s.reason_policy)
        .archive_workspace(
            WorkspaceId::from_uuid(id),
            body.reason,
            ActorId::from_uuid(actor.account.id.as_uuid()),
            Utc::now(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn restore<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    owned_workspace(&s, WorkspaceId::from_uuid(id), actor.account.id).await?;
    ReaderService::new(s.repository, s.reason_policy)
        .restore_workspace(WorkspaceId::from_uuid(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
