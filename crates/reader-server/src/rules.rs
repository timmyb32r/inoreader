use super::*;

pub(super) async fn list_rules<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<Vec<RuleDraft>>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    Ok(Json(
        s.repository
            .rules_by_workspace(workspace)
            .await?
            .into_iter()
            .map(rule_draft)
            .collect(),
    ))
}
pub(super) async fn preview_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<RuleListQuery>,
    Json(body): Json<RuleDraft>,
) -> Result<Json<RulePreviewResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let subscription = SubscriptionId::from_uuid(body.subscription_id);
    let sub = owned_subscription(&s, subscription, actor.account.id).await?;
    if sub.workspace_id() != workspace {
        return Err(ApiFailure::Forbidden);
    }
    let rule = parse_rule(None, &body)?;
    let values = s
        .repository
        .article_presentations_by_workspace(workspace)
        .await?;
    let mut matched = 0usize;
    let mut shared = 0usize;
    let mut total = 0usize;
    let mut samples = Vec::new();
    for value in values
        .into_iter()
        .filter(|value| value.subscription_ids.contains(&subscription))
    {
        total += 1;
        let text = value
            .safe_html
            .as_deref()
            .map(html_text_paragraphs)
            .map(|parts| parts.join("\n"));
        if rule.matches(&value.article.key.title, text.as_deref()) {
            matched += 1;
            if value.subscription_ids.len() > 1 {
                shared += 1
            }
            if samples.len() < 20 {
                samples.push(value.article.id.as_uuid())
            }
        }
    }
    Ok(Json(RulePreviewResponse {
        matched_articles: matched,
        shared_articles: shared,
        total_subscription_articles: total,
        sample_article_ids: samples,
    }))
}
pub(super) async fn create_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<RuleListQuery>,
    Json(body): Json<RuleDraft>,
) -> Result<Json<RuleDraft>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let sub = owned_subscription(
        &s,
        SubscriptionId::from_uuid(body.subscription_id),
        actor.account.id,
    )
    .await?;
    if sub.workspace_id() != workspace {
        return Err(ApiFailure::Forbidden);
    }
    let value = parse_rule(None, &body)?;
    s.repository
        .save_rule(workspace, None, value.clone())
        .await?;
    Ok(Json(rule_draft(value)))
}
pub(super) async fn update_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
    Json(body): Json<RuleDraft>,
) -> Result<Json<RuleDraft>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let current = s.repository.rule(workspace, RuleId::from_uuid(id)).await?;
    let sub = owned_subscription(
        &s,
        SubscriptionId::from_uuid(body.subscription_id),
        actor.account.id,
    )
    .await?;
    if sub.workspace_id() != workspace {
        return Err(ApiFailure::Forbidden);
    }
    let value = parse_rule(Some((RuleId::from_uuid(id), current.version + 1)), &body)?;
    s.repository
        .save_rule(workspace, Some(current.version), value.clone())
        .await?;
    Ok(Json(rule_draft(value)))
}
pub(super) async fn delete_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
) -> Result<StatusCode, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let rule = s.repository.rule(workspace, RuleId::from_uuid(id)).await?;
    s.repository
        .delete_rule(workspace, rule.id, rule.version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn apply_rule<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<RuleApplicationAccepted>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let rule = s.repository.rule(workspace, RuleId::from_uuid(id)).await?;
    let operation_id = s
        .repository
        .enqueue_rule_application(workspace, rule)
        .await?;
    Ok(Json(RuleApplicationAccepted {
        operation_id,
        status: "queued".into(),
    }))
}
pub(super) async fn rule_application_status<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<RuleListQuery>,
) -> Result<Json<RuleApplicationStatus>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let value = s
        .repository
        .rule_application_progress(workspace, id)
        .await?;
    Ok(Json(RuleApplicationStatus {
        operation_id: value.operation_id,
        status: value.status,
        evaluated: value.evaluated,
        cancel_reason: value.cancel_reason,
    }))
}
