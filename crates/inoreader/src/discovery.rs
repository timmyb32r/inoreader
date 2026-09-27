//! Discovery composition responsibilities.
use super::*;

pub(super) struct ProductionWebCollector {
    pub(super) static_feeds: StaticWebFeedCollector<ProductionFetcher>,
    pub(super) adapters: BuiltInAdapterCollector,
    pub(super) cdp: CdpBrowserCollector,
    pub(super) max_pages: usize,
    pub(super) max_actions: usize,
}
#[async_trait::async_trait]
impl BrowserCollector for ProductionWebCollector {
    fn capability(&self) -> BrowserCapability {
        self.cdp.capability()
    }
    async fn collect(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        if let SourceKind::WebPage(recipe) = source.kind() {
            if recipe.actions().page_count() > self.max_pages || recipe.max_pages() > self.max_pages
            {
                return Err(FetchError::Rejected("web_feed_page_limit".into()));
            }
            if recipe.actions().action_count() > self.max_actions {
                return Err(FetchError::Rejected("web_feed_action_limit".into()));
            }
        }
        match source.kind() {
            SourceKind::BuiltIn(_) => self.adapters.collect(source).await,
            SourceKind::WebPage(recipe) if recipe.loading() == WebLoading::Browser => {
                self.cdp.collect(source).await
            }
            SourceKind::WebPage(recipe) if recipe.loading() == WebLoading::Automatic => {
                match self.static_feeds.collect(source).await {
                    Ok(records) if !records.is_empty() => return Ok(records),
                    Ok(_) => {}
                    Err(FetchError::Rejected(value)) if value == "selector_requires_browser" => {}
                    Err(error) => return Err(error),
                }
                self.cdp.collect(source).await
            }
            _ => self.static_feeds.collect(source).await,
        }
    }
}
pub(super) struct VisualSnapshotState {
    pub(super) session_id: uuid::Uuid,
    pub(super) workspace_id: uuid::Uuid,
    pub(super) expires_at: chrono::DateTime<chrono::Utc>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) groups: Vec<reader_ingest::VisualCandidateGroup>,
}
pub(super) struct ProductionDiscovery {
    pub(super) fetcher: Arc<ProductionFetcher>,
    pub(super) web_feeds: Arc<ProductionWebCollector>,
    pub(super) preview_timeout: Duration,
    pub(super) initial_items: usize,
    pub(super) visual_snapshots: tokio::sync::Mutex<HashMap<uuid::Uuid, VisualSnapshotState>>,
}
#[async_trait::async_trait]
impl FeedDiscovery for ProductionDiscovery {
    async fn discover(&self, url: Url) -> Result<FeedPreviewResponse, String> {
        let page = self
            .fetcher
            .fetch(&url, &CacheValidators::default())
            .await
            .map_err(|e| e.to_string())?;
        let (is_json, records) = match reader_collectors::parse_json(&page.body, &page.final_url) {
            Ok(v) => (true, v),
            Err(_) => (
                false,
                reader_collectors::parse_xml(&page.body, &page.final_url)
                    .map_err(|e| e.to_string())?,
            ),
        };
        let title = page.final_url.host_str().unwrap_or("Feed").to_owned();
        let available_items = records.len();
        let initial_items = available_items.min(self.initial_items);
        Ok(FeedPreviewResponse {
            title,
            kind: if is_json { "json_feed" } else { "rss" }.to_owned(),
            url: page.final_url.to_string(),
            available_items,
            initial_items,
            incomplete: available_items > initial_items,
            articles: records
                .into_iter()
                .take(initial_items)
                .map(|v| FeedPreviewArticle {
                    title: v.title,
                    published_at: v.published_at.map(|d| d.as_str().to_owned()),
                })
                .collect(),
        })
    }
    async fn preview_web_feed(
        &self,
        draft: &WebFeedRecipeDraft,
    ) -> Result<(FeedPreviewResponse, reader_core::PreparedWebFeed), String> {
        let prepared = reader_core::PreparedWebFeed::new(
            draft.clone(),
            self.web_feeds.max_pages,
            self.web_feeds.max_actions,
        )?;
        let url = Url::parse(&draft.url).map_err(|_| "invalid web feed URL".to_owned())?;
        let recipe = prepared.recipe().clone();
        let source = SourceDefinition::new(
            reader_core::SourceId::new(),
            url.clone(),
            SourceKind::WebPage(recipe),
        )
        .map_err(|e| e.to_string())?;
        let records = tokio::time::timeout(self.preview_timeout, self.web_feeds.collect(&source))
            .await
            .map_err(|_| "web feed preview timed out".to_owned())?
            .map_err(|e| e.to_string())?;
        let title = url.host_str().unwrap_or("Web feed").to_owned();
        let available_items = records.len();
        let initial_items = available_items.min(self.initial_items);
        Ok((
            FeedPreviewResponse {
                title,
                kind: "web_feed".to_owned(),
                url: url.to_string(),
                available_items,
                initial_items,
                incomplete: available_items > initial_items,
                articles: records
                    .into_iter()
                    .take(initial_items)
                    .map(|v| FeedPreviewArticle {
                        title: v.key().title.clone(),
                        published_at: v.published_at().map(|d| d.as_str().to_owned()),
                    })
                    .collect(),
            },
            prepared,
        ))
    }
    async fn visual_preview(
        &self,
        session_id: uuid::Uuid,
        request: &VisualPreviewRequest,
    ) -> Result<VisualPreviewResponse, String> {
        let url = Url::parse(&request.url).map_err(|_| "invalid visual preview URL".to_owned())?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("visual preview URL must use HTTP or HTTPS".into());
        }
        let mobile = match request.viewport.as_str() {
            "desktop" => false,
            "mobile" => true,
            _ => return Err("invalid visual preview viewport".into()),
        };
        let capture = tokio::time::timeout(
            self.preview_timeout,
            self.web_feeds.cdp.visual_snapshot(&url, mobile),
        )
        .await
        .map_err(|_| "visual preview timed out".to_owned())?
        .map_err(|e| e.to_string())?;
        let token = uuid::Uuid::new_v4();
        let expires_at = chrono::Utc::now()
            + chrono::Duration::from_std(self.preview_timeout)
                .map_err(|_| "invalid visual preview deadline".to_owned())?;
        let groups = capture
            .groups
            .iter()
            .enumerate()
            .map(|(index, group)| VisualCandidateGroupView {
                id: format!("group-{index}"),
                selector: SelectorDraft {
                    language: "css".into(),
                    expression: group.selector.clone(),
                },
                count: group.boxes.len(),
                boxes: group
                    .boxes
                    .iter()
                    .map(|rect| VisualRectView {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                    })
                    .collect(),
            })
            .collect();
        let state = VisualSnapshotState {
            session_id,
            workspace_id: request.workspace_id,
            expires_at,
            width: capture.width,
            height: capture.height,
            groups: capture.groups,
        };
        let mut snapshots = self.visual_snapshots.lock().await;
        let now = chrono::Utc::now();
        snapshots.retain(|_, value| {
            value.expires_at > now
                && (value.session_id != session_id || value.workspace_id != request.workspace_id)
        });
        snapshots.insert(token, state);
        Ok(VisualPreviewResponse {
            snapshot_token: token,
            expires_at,
            image_data_url: format!("data:image/png;base64,{}", BASE64.encode(capture.png)),
            width: capture.width,
            height: capture.height,
            groups,
        })
    }
    async fn visual_select(
        &self,
        session_id: uuid::Uuid,
        request: &VisualSelectionRequest,
    ) -> Result<VisualSelectionResponse, String> {
        if !request.x.is_finite() || !request.y.is_finite() || request.x < 0.0 || request.y < 0.0 {
            return Err("invalid visual selection coordinates".into());
        }
        let mut snapshots = self.visual_snapshots.lock().await;
        snapshots.retain(|_, value| value.expires_at > chrono::Utc::now());
        let snapshot = snapshots
            .get(&request.snapshot_token)
            .ok_or_else(|| "visual_snapshot_stale".to_owned())?;
        if snapshot.session_id != session_id || snapshot.workspace_id != request.workspace_id {
            return Err("visual_snapshot_stale".into());
        }
        if request.x > f64::from(snapshot.width) || request.y > f64::from(snapshot.height) {
            return Err("visual selection is outside the snapshot".into());
        }
        let selected = select_visual_group(snapshot, request.x, request.y)?;
        snapshots.remove(&request.snapshot_token);
        Ok(selected)
    }
}

pub(super) fn select_visual_group(
    snapshot: &VisualSnapshotState,
    x: f64,
    y: f64,
) -> Result<VisualSelectionResponse, String> {
    let group = snapshot
        .groups
        .iter()
        .filter(|group| {
            group.boxes.iter().any(|rect| {
                x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
            })
        })
        .min_by(|left, right| {
            let left_area = left
                .boxes
                .iter()
                .filter(|r| x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height)
                .map(|r| r.width * r.height)
                .fold(f64::INFINITY, f64::min);
            let right_area = right
                .boxes
                .iter()
                .filter(|r| x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height)
                .map(|r| r.width * r.height)
                .fold(f64::INFINITY, f64::min);
            left_area.total_cmp(&right_area)
        })
        .ok_or_else(|| "no repeated item group at the selected coordinates".to_owned())?;
    WebSelector::new(SelectorLanguage::Css, group.selector.clone()).map_err(|e| e.to_string())?;
    Ok(VisualSelectionResponse {
        selector: SelectorDraft {
            language: "css".into(),
            expression: group.selector.clone(),
        },
        count: group.boxes.len(),
        similar_items: group
            .boxes
            .iter()
            .map(|rect| VisualRectView {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            })
            .collect(),
    })
}
