use super::*;

pub(super) fn workspace_view(value: &Workspace) -> WorkspaceView {
    let (reason, archive_reason_at) = match value.status() {
        WorkspaceStatus::Archived(event) => {
            (Some(event.reason.as_str().to_owned()), Some(event.at))
        }
        WorkspaceStatus::Active => (None, None),
    };
    WorkspaceView {
        id: value.id().as_uuid(),
        name: value.name().to_owned(),
        archived: !value.accepts_delivery(),
        archive_reason: reason,
        archive_reason_at,
    }
}
pub(super) async fn subscription_views<R: ReaderRepository>(
    repository: &R,
    values: &[Subscription],
) -> Result<Vec<SubscriptionView>, ApiFailure> {
    if values.is_empty() {
        return Ok(vec![]);
    }
    let stats = repository
        .subscription_stats(values[0].workspace_id(), values)
        .await?;
    Ok(values
        .iter()
        .map(|value| {
            subscription_view(
                value,
                stats
                    .get(&value.id())
                    .unwrap_or(&reader_application::SubscriptionStats::default()),
            )
        })
        .collect())
}
pub(super) async fn subscription_stats_for<R: ReaderRepository>(
    repository: &R,
    value: &Subscription,
) -> Result<reader_application::SubscriptionStats, ApiFailure> {
    let stats = repository
        .subscription_stats(value.workspace_id(), std::slice::from_ref(value))
        .await?;
    Ok(stats.get(&value.id()).cloned().unwrap_or_default())
}
pub(super) fn subscription_view(
    value: &Subscription,
    stats: &reader_application::SubscriptionStats,
) -> SubscriptionView {
    let (status, reason, reason_at) = match value.status() {
        reader_core::SubscriptionStatus::Active => (SubscriptionStatusView::Active, None, None),
        reader_core::SubscriptionStatus::Paused(event) => (
            SubscriptionStatusView::Paused,
            Some(event.reason.as_str().to_owned()),
            Some(event.at),
        ),
        reader_core::SubscriptionStatus::Archived => (SubscriptionStatusView::Archived, None, None),
    };
    SubscriptionView {
        id: value.id().as_uuid(),
        name: value.title().to_owned(),
        source_title: value.source_title().to_owned(),
        custom_name: value.custom_name().map(str::to_owned),
        source_url: value.source_url_exact().to_owned(),
        icon_data_url: stats.icon_data_url.clone(),
        source_type: source_type_view(stats.source_type),
        created_at: value.created_at(),
        count: stats.article_count,
        unread_count: stats.unread_count,
        status,
        last_update: stats.last_success_at,
        last_error_at: stats.last_error_at,
        consecutive_failures: stats.consecutive_failures,
        needs_attention: stats.needs_attention,
        attention_reason: stats.needs_attention.then(|| {
            stats.error.clone().unwrap_or_else(|| {
                "Source refresh has been failing for the configured attention duration".to_owned()
            })
        }),
        incomplete: stats.incomplete,
        continuation: stats.continuation.clone(),
        error: stats.error.clone(),
        editable_web_feed: stats.editable_web_feed,
        reason,
        reason_at,
    }
}
pub(super) fn article_view(value: &reader_application::ArticlePresentation) -> ArticleView {
    let mut view = article_domain_view(&value.article);
    let date = reader_core::resolve_publication(&value.publication);
    view.publication_status = if date.is_some() {
        reader_server_contracts::PublicationStatus::Known
    } else if value.publication.is_empty() {
        reader_server_contracts::PublicationStatus::Unknown
    } else if value.publication.iter().any(|v| v.date().is_some()) {
        reader_server_contracts::PublicationStatus::Conflicting
    } else {
        reader_server_contracts::PublicationStatus::Invalid
    };
    view.published_at = date.map(|v| v.as_str().to_owned());
    view.publication_sources = value.publication.iter().map(|v| v.source.clone()).collect();
    if matches!(
        value.description_media_type.as_deref(),
        Some("text/html" | "application/xhtml+xml")
    ) {
        let rendered = reader_web_runtime::SafeRenderedContent::from_untrusted_html_with_base(
            value.article.key.description.as_deref().unwrap_or_default(),
            Some(&view.url),
        );
        view.excerpt = reader_web_runtime::plain_text(rendered.html());
        view.body = vec![view.excerpt.clone()];
        view.body_html = Some(rendered.html().to_owned());
    }
    view.sources = value.subscription_titles.clone();
    view.subscription_ids = value
        .subscription_ids
        .iter()
        .map(|id| id.as_uuid())
        .collect();
    view.source = view
        .sources
        .first()
        .cloned()
        .unwrap_or_else(|| "Unknown source".to_owned());
    if let Some(content) = &value.safe_html {
        let rendered = reader_web_runtime::SafeRenderedContent::from_untrusted_html_with_base(
            content,
            Some(&view.url),
        );
        view.body_html = Some(rendered.html().to_owned());
        view.body.clear();
    }
    view.full_text = match value.full_text_status {
        reader_application::ContentStatus::Ready => FullTextStatus::Ready,
        reader_application::ContentStatus::Pending => FullTextStatus::Pending,
        reader_application::ContentStatus::Failed => FullTextStatus::Failed,
    };
    view.full_text_reason = value.failure_reason.clone();
    view.marked_read_at = value.marked_read_at;
    view.read_method = value.read_method;
    view.video = value.video.clone();
    view
}

pub(super) fn article_page_request(
    view: &str,
    subscription_id: Option<SubscriptionId>,
    cursor: Option<&str>,
    direction: Option<&str>,
) -> Result<ArticlePageRequest, ApiFailure> {
    let scope = reader_application::ArticleScope::from_wire(view, subscription_id)
        .map_err(ApiFailure::Validation)?;
    let direction = match direction.unwrap_or("older") {
        "older" => ArticlePageDirection::Older,
        "newer" => ArticlePageDirection::Newer,
        _ => return Err(ApiFailure::Validation("invalid article page direction")),
    };
    let cursor = cursor.map(decode_article_cursor).transpose()?;
    ArticlePageRequest::new(
        scope,
        cursor,
        direction,
        reader_application::SelectionLimit::new(ARTICLE_PAGE_SIZE)
            .map_err(ApiFailure::Validation)?,
    )
    .map_err(ApiFailure::Validation)
}

pub(super) fn article_page_view(page: reader_application::ArticlePage) -> ArticlePageView {
    let newer_cursor = page
        .has_newer
        .then(|| page.articles.first().map(article_cursor))
        .flatten();
    let older_cursor = page
        .has_older
        .then(|| page.articles.last().map(article_cursor))
        .flatten();
    ArticlePageView {
        articles: page.articles.iter().map(article_view).collect(),
        total: page.total,
        unread_total: page.unread_total,
        newer_cursor,
        older_cursor,
    }
}

pub(super) fn article_cursor(value: &reader_application::ArticlePresentation) -> String {
    format!(
        "{}.{}",
        value
            .marked_read_at
            .unwrap_or(value.article.first_arrived_at)
            .timestamp_micros(),
        value.article.id.as_uuid()
    )
}

pub(super) fn decode_article_cursor(value: &str) -> Result<ArticlePageCursor, ApiFailure> {
    let (micros, id) = value
        .split_once('.')
        .ok_or(ApiFailure::Validation("invalid article cursor"))?;
    let micros = micros
        .parse::<i64>()
        .map_err(|_| ApiFailure::Validation("invalid article cursor"))?;
    let arrived_at = chrono::DateTime::from_timestamp_micros(micros)
        .ok_or(ApiFailure::Validation("invalid article cursor"))?;
    let article_id = Uuid::parse_str(id)
        .map(ArticleId::from_uuid)
        .map_err(|_| ApiFailure::Validation("invalid article cursor"))?;
    Ok(ArticlePageCursor {
        arrived_at,
        article_id,
    })
}
pub(super) fn article_domain_view(value: &reader_core::Article) -> ArticleView {
    let excerpt = value.key.description.clone().unwrap_or_default();
    let url = value
        .key
        .location
        .exact_url()
        .unwrap_or_default()
        .to_owned();
    ArticleView {
        video: None,
        marked_read_at: None,
        read_method: None,
        id: value.id.as_uuid(),
        url,
        source: "Unknown source".to_owned(),
        sources: Vec::new(),
        subscription_ids: Vec::new(),
        title: value.key.title.clone(),
        excerpt: excerpt.clone(),
        body: if excerpt.is_empty() {
            Vec::new()
        } else {
            vec![excerpt]
        },
        body_html: None,
        author: None,
        saved_at: value.first_arrived_at.to_rfc3339(),
        published_at: None,
        publication_status: reader_server_contracts::PublicationStatus::Unknown,
        publication_sources: Vec::new(),
        read: value.state.read,
        later: value.state.later,
        full_text: FullTextStatus::Pending,
        full_text_reason: None,
    }
}

pub(super) fn html_text_paragraphs(value: &str) -> Vec<String> {
    let normalized = value
        .replace("</p>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("</li>", "\n");
    normalized
        .lines()
        .filter_map(|line| {
            let mut text = String::new();
            let mut tag = false;
            for ch in line.chars() {
                match ch {
                    '<' => tag = true,
                    '>' => tag = false,
                    _ if !tag => text.push(ch),
                    _ => {}
                }
            }
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        })
        .collect()
}

pub(super) fn source_type_view(value: reader_application::SourceType) -> SourceTypeView {
    match value {
        reader_application::SourceType::Feed => SourceTypeView::Feed,
        reader_application::SourceType::Web => SourceTypeView::Web,
        reader_application::SourceType::BuiltIn => SourceTypeView::BuiltIn,
    }
}

pub(super) fn read_period(
    from: Option<chrono::DateTime<chrono::Utc>>,
    until: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Option<reader_application::ReadPeriod>, ApiFailure> {
    match (from, until) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) => reader_application::ReadPeriod::new(start, end)
            .map(Some)
            .map_err(ApiFailure::Validation),
        _ => Err(ApiFailure::Validation(
            "both read_from and read_until are required",
        )),
    }
}
