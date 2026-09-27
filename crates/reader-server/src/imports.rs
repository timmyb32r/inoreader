use super::*;

pub(super) async fn import_opml<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<OpmlImportRequest>,
) -> Result<Json<OpmlImportResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(body.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let preview_id = Uuid::new_v5(&body.workspace_id, body.opml.as_bytes());
    if body.apply && body.preview_id != Some(preview_id) {
        return Err(ApiFailure::Validation(
            "OPML must be previewed unchanged before apply",
        ));
    }
    let outlines = opml::outlines(&body.opml).map_err(ApiFailure::from)?;
    let existing = s.repository.subscriptions_by_workspace(workspace).await?;
    let mut values = Vec::new();
    let mut warnings = Vec::new();
    if opml::has_folders(&body.opml).map_err(ApiFailure::from)? {
        warnings.push("OPML folders are flattened into the target workspace".to_owned())
    }
    let mut seen = std::collections::HashSet::new();
    for (url, title) in outlines {
        if !seen.insert(url.as_str().to_owned()) || existing.iter().any(|v| v.source_url() == &url)
        {
            warnings.push(format!("duplicate: {url}"));
            continue;
        }
        values.push(Subscription::new(
            SubscriptionId::new(),
            workspace,
            url,
            title,
        ))
    }
    let added = values.len();
    if body.apply {
        s.repository
            .import_subscriptions_atomic(workspace, values)
            .await?
    }
    Ok(Json(OpmlImportResponse {
        preview_id,
        subscriptions: added,
        warnings,
    }))
}
pub(super) async fn export_opml<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<OpmlExportQuery>,
) -> Result<Json<String>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let subscriptions = s.repository.subscriptions_by_workspace(workspace).await?;
    let mut result=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><opml version=\"2.0\"><head><title>Subscriptions</title></head><body>");
    for value in subscriptions {
        result.push_str("<outline type=\"rss\" text=\"");
        result.push_str(&opml::escape_xml(value.title()));
        result.push_str("\" xmlUrl=\"");
        result.push_str(&opml::escape_xml(value.source_url().as_str()));
        result.push_str("\"/>")
    }
    result.push_str("</body></opml>");
    Ok(Json(result))
}
