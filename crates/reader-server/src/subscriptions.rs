use super::*;

pub(super) async fn add_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Json(body): Json<AddSubscriptionRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(body.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let url =
        Url::parse(&body.url).map_err(|_| ApiFailure::Validation("invalid subscription URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ApiFailure::Validation(
            "subscription URL must use HTTP or HTTPS",
        ));
    }
    if let Some(existing) = s
        .repository
        .subscriptions_by_workspace(workspace)
        .await?
        .into_iter()
        .find(|subscription| subscription.source_url_exact() == body.url)
    {
        return Err(ApiFailure::ExistingSubscription(matches!(
            existing.status(),
            reader_core::SubscriptionStatus::Archived
        )));
    }
    let discovered = s
        .discovery
        .discover(url.clone())
        .await
        .map_err(ApiFailure::Discovery)?;
    let title = trusted_discovery_title(body.title.as_deref(), discovered.title);
    valid_name(&title)?;
    let value =
        Subscription::new_with_exact_url(SubscriptionId::new(), workspace, url, body.url, title)
            .map_err(|_| ApiFailure::Validation("source URL does not match parsed URL"))?;
    s.repository.save_subscription(None, value.clone()).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
pub(super) fn trusted_discovery_title(
    _client_title: Option<&str>,
    discovered_title: String,
) -> String {
    discovered_title
}
pub(super) async fn list_subscriptions<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<Vec<SubscriptionView>>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let workspace = WorkspaceId::from_uuid(query.workspace_id);
    owned_workspace(&s, workspace, actor.account.id).await?;
    let values = s.repository.subscriptions_by_workspace(workspace).await?;
    Ok(Json(
        subscription_views(s.repository.as_ref(), &values).await?,
    ))
}
pub(super) async fn rename_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RenameSubscriptionRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    if !body.name.is_empty() {
        valid_name(&body.name)?;
    }
    let mut value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let revision = value.revision();
    value.rename(body.name);
    s.repository
        .save_subscription(Some(revision), value.clone())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
pub(super) async fn get_subscription<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
pub(super) async fn save_subscription_note<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveSubscriptionNoteRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let mut value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let revision = value.revision();
    value.set_personal_note(body.note);
    s.repository
        .save_subscription(Some(revision), value.clone())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
pub(super) async fn subscription_activity<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<SubscriptionActivityView>>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let since = Utc::now() - chrono::Duration::days(30);
    let events = s
        .repository
        .subscription_activity(actor.account.id, subscription.id(), since)
        .await?;
    Ok(Json(
        events
            .into_iter()
            .map(|event| SubscriptionActivityView {
                id: event.id,
                occurred_at: event.occurred_at,
                successful: event.successful,
                duration_ms: event.duration_ms,
                discovered_items: event.discovered_items,
                diagnostic: event.diagnostic,
            })
            .collect(),
    ))
}
pub(super) async fn publication_history<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PublicationHistoryView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let history = s
        .repository
        .publication_history(actor.account.id, SubscriptionId::from_uuid(id))
        .await?;
    Ok(Json(PublicationHistoryView {
        days: history
            .days
            .into_iter()
            .map(|day| PublicationDayView {
                date: day.date.to_string(),
                count: day.count,
            })
            .collect(),
        undated: history.undated,
        conflicting: history.conflicting,
    }))
}
pub(super) async fn subscription_extraction<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionExtractionView>, ApiFailure> {
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    let (feed_urls, recipe_version, recipe_summary) = if stats.editable_web_feed {
        let (version, document) = s.repository.web_feed_recipe(subscription.id()).await?;
        let draft: WebFeedRecipeDraft =
            serde_json::from_str(&document).map_err(|_| ApiFailure::Internal)?;
        (
            Vec::new(),
            Some(version),
            Some(web_feed_recipe_summary(&draft)),
        )
    } else if stats.source_type == reader_application::SourceType::Feed {
        (vec![subscription.source_url_exact().to_owned()], None, None)
    } else {
        (Vec::new(), None, Some("Built-in adapter".to_owned()))
    };
    Ok(Json(SubscriptionExtractionView {
        source_type: source_type_view(stats.source_type),
        feed_urls,
        recipe_version,
        recipe_summary,
        last_preview: None,
    }))
}
pub(super) fn web_feed_recipe_summary(draft: &WebFeedRecipeDraft) -> String {
    format!(
        "Cards: {}; title: {}; content: {}",
        draft.card_selector.as_deref().unwrap_or(&draft.selector),
        draft.title_selector.as_deref().unwrap_or("not configured"),
        draft
            .content_selector
            .as_deref()
            .unwrap_or("not configured")
    )
}
pub(super) fn ensure_source_url_change_allowed(
    stats: &reader_application::SubscriptionStats,
) -> Result<(), ApiFailure> {
    if stats.editable_web_feed {
        Err(ApiFailure::Validation(
            "web feed source URL must be changed through its extraction recipe",
        ))
    } else {
        Ok(())
    }
}
pub(super) async fn preview_subscription_source_url<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PreviewSourceUrlRequest>,
) -> Result<Json<SourceUrlPreviewResponse>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    ensure_source_url_change_allowed(&stats)?;
    let url = Url::parse(&body.url).map_err(|_| ApiFailure::Validation("invalid feed URL"))?;
    let preview = s
        .discovery
        .discover(url.clone())
        .await
        .map_err(ApiFailure::Discovery)?;
    let token = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::minutes(10);
    s.repository
        .save_source_url_preview(reader_application::SourceUrlPreviewRecord {
            id: token,
            subscription_id: subscription.id(),
            url: body.url.clone(),
            source_title: preview.title.clone(),
            expires_at,
            revision: 0,
        })
        .await?;
    Ok(Json(SourceUrlPreviewResponse {
        token,
        url: body.url,
        title: preview.title,
        expires_at,
    }))
}
pub(super) async fn commit_subscription_source_url<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<CommitSourceUrlRequest>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let mut subscription =
        owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    ensure_source_url_change_allowed(&stats)?;
    let preview = s.repository.source_url_preview(body.preview_token).await?;
    if preview.subscription_id != subscription.id() {
        return Err(ApiFailure::Repository(RepositoryError::NotFound));
    }
    if preview.expires_at <= Utc::now() {
        return Err(ApiFailure::Validation("source URL preview expired"));
    }
    let url = Url::parse(&preview.url)
        .map_err(|_| ApiFailure::Validation("stored source URL preview is invalid"))?;
    if s.repository
        .subscriptions_by_workspace(subscription.workspace_id())
        .await?
        .iter()
        .any(|candidate| {
            candidate.id() != subscription.id() && candidate.source_url_exact() == preview.url
        })
    {
        return Err(ApiFailure::ExistingSubscription(false));
    }
    let verified = s
        .discovery
        .discover(url.clone())
        .await
        .map_err(ApiFailure::Discovery)?;
    if verified.title != preview.source_title {
        return Err(ApiFailure::Validation("source changed since preview"));
    }
    let revision = subscription.revision();
    subscription
        .replace_source(url, preview.url, preview.source_title)
        .map_err(|_| ApiFailure::Validation("source URL preview is inconsistent"))?;
    s.repository
        .replace_subscription_source(revision, subscription.clone())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &subscription).await?;
    Ok(Json(subscription_view(&subscription, &stats)))
}
pub(super) async fn unsubscribe<R: ReaderRepository + 'static>(
    State(s): State<AppState<R>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<SubscriptionView>, ApiFailure> {
    csrf(&s, &headers)?;
    let actor = auth(&s, &headers).await?;
    let value = owned_subscription(&s, SubscriptionId::from_uuid(id), actor.account.id).await?;
    let value = ReaderService::new(s.repository.clone(), s.reason_policy)
        .archive_subscription(value.id())
        .await?;
    let stats = subscription_stats_for(s.repository.as_ref(), &value).await?;
    Ok(Json(subscription_view(&value, &stats)))
}
