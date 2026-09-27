use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebLoading {
    Automatic,
    Static,
    Browser,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorLanguage {
    Css,
    XPath,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WebSelector {
    language: SelectorLanguage,
    expression: String,
}
impl WebSelector {
    pub fn new(language: SelectorLanguage, expression: String) -> Result<Self, WebFeedError> {
        if expression.trim().is_empty() {
            return Err(WebFeedError::EmptySelector);
        }
        if language == SelectorLanguage::Css {
            scraper::Selector::parse(&expression).map_err(|_| WebFeedError::InvalidSelector)?;
        } else if !expression.trim_start().starts_with('/')
            && !expression.trim_start().starts_with('(')
        {
            return Err(WebFeedError::InvalidSelector);
        }
        Ok(Self {
            language,
            expression,
        })
    }
    pub fn language(&self) -> SelectorLanguage {
        self.language
    }
    pub fn expression(&self) -> &str {
        &self.expression
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebViewport {
    Desktop,
    Mobile,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WebFeedActions {
    viewport: WebViewport,
    hide_overlays: Vec<WebSelector>,
    start_pages: Vec<Url>,
    next_page: Option<WebSelector>,
    load_more: Option<WebSelector>,
    load_more_clicks: usize,
    scrolls: usize,
}
impl WebFeedActions {
    // Both configured limits are part of the construction boundary: keeping
    // them here prevents callers from creating an unchecked action plan.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        viewport: WebViewport,
        hide_overlays: Vec<WebSelector>,
        start_pages: Vec<Url>,
        next_page: Option<WebSelector>,
        load_more: Option<WebSelector>,
        load_more_clicks: usize,
        scrolls: usize,
        max_pages: usize,
        max_actions: usize,
    ) -> Result<Self, WebFeedError> {
        let value = Self {
            viewport,
            hide_overlays,
            start_pages,
            next_page,
            load_more,
            load_more_clicks,
            scrolls,
        };
        if value.page_count() > max_pages {
            return Err(WebFeedError::LimitExceeded("start_pages"));
        }
        let action_count = value
            .hide_overlays
            .len()
            .checked_add(value.load_more_clicks)
            .and_then(|n| n.checked_add(value.scrolls))
            .and_then(|n| n.checked_add(usize::from(value.next_page.is_some())))
            .ok_or(WebFeedError::LimitExceeded("actions"))?;
        if action_count > max_actions {
            return Err(WebFeedError::LimitExceeded("actions"));
        }
        Ok(value)
    }
    pub fn viewport(&self) -> WebViewport {
        self.viewport
    }
    pub fn hide_overlays(&self) -> &[WebSelector] {
        &self.hide_overlays
    }
    pub fn start_pages(&self) -> &[Url] {
        &self.start_pages
    }
    pub fn next_page(&self) -> Option<&WebSelector> {
        self.next_page.as_ref()
    }
    pub fn load_more(&self) -> Option<&WebSelector> {
        self.load_more.as_ref()
    }
    pub fn load_more_clicks(&self) -> usize {
        self.load_more_clicks
    }
    pub fn scrolls(&self) -> usize {
        self.scrolls
    }
    pub fn page_count(&self) -> usize {
        self.start_pages.len().saturating_add(1)
    }
    pub fn action_count(&self) -> usize {
        self.hide_overlays.len()
            + self.load_more_clicks
            + self.scrolls
            + usize::from(self.next_page.is_some())
    }
}
impl Default for WebFeedActions {
    fn default() -> Self {
        Self {
            viewport: WebViewport::Desktop,
            hide_overlays: vec![],
            start_pages: vec![],
            next_page: None,
            load_more: None,
            load_more_clicks: 0,
            scrolls: 0,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WebExtraction {
    listing_url: Option<Url>,
    card_selector: Option<WebSelector>,
    title_selector: Option<WebSelector>,
    date_selector: Option<WebSelector>,
    content_selector: Option<WebSelector>,
    wait_selector: Option<WebSelector>,
    url_pattern: Option<String>,
}
impl WebExtraction {
    pub fn new(
        listing_url: Option<Url>,
        card_selector: Option<String>,
        title_selector: Option<String>,
        date_selector: Option<String>,
        content_selector: Option<String>,
        wait_selector: Option<String>,
        url_pattern: Option<String>,
    ) -> Result<Self, WebFeedError> {
        let css = |value: Option<String>| {
            value
                .map(|value| WebSelector::new(SelectorLanguage::Css, value))
                .transpose()
        };
        if let Some(value) = url_pattern.as_deref() {
            regex::Regex::new(value).map_err(|_| WebFeedError::InvalidUrlPattern)?;
        }
        Ok(Self {
            listing_url,
            card_selector: css(card_selector)?,
            title_selector: css(title_selector)?,
            date_selector: css(date_selector)?,
            content_selector: css(content_selector)?,
            wait_selector: css(wait_selector)?,
            url_pattern,
        })
    }
    pub fn listing_url(&self) -> Option<&Url> {
        self.listing_url.as_ref()
    }
    pub fn card_selector(&self) -> Option<&WebSelector> {
        self.card_selector.as_ref()
    }
    pub fn title_selector(&self) -> Option<&WebSelector> {
        self.title_selector.as_ref()
    }
    pub fn date_selector(&self) -> Option<&WebSelector> {
        self.date_selector.as_ref()
    }
    pub fn content_selector(&self) -> Option<&WebSelector> {
        self.content_selector.as_ref()
    }
    pub fn wait_selector(&self) -> Option<&WebSelector> {
        self.wait_selector.as_ref()
    }
    pub fn url_pattern(&self) -> Option<&str> {
        self.url_pattern.as_deref()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WebFeedRecipe {
    selector: WebSelector,
    loading: WebLoading,
    actions: WebFeedActions,
    extraction: WebExtraction,
    max_pages: usize,
}
impl WebFeedRecipe {
    pub fn new(selector: String, loading: WebLoading) -> Result<Self, WebFeedError> {
        Self::advanced(
            WebSelector::new(SelectorLanguage::Css, selector)?,
            loading,
            WebFeedActions::default(),
        )
    }
    pub fn advanced(
        selector: WebSelector,
        loading: WebLoading,
        actions: WebFeedActions,
    ) -> Result<Self, WebFeedError> {
        Self::configured(selector, loading, actions, WebExtraction::default(), 1)
    }
    pub fn configured(
        selector: WebSelector,
        loading: WebLoading,
        actions: WebFeedActions,
        extraction: WebExtraction,
        max_pages: usize,
    ) -> Result<Self, WebFeedError> {
        if loading == WebLoading::Static && selector.language() == SelectorLanguage::XPath {
            return Err(WebFeedError::XPathRequiresBrowser);
        }
        if max_pages == 0 {
            return Err(WebFeedError::ZeroLimit {
                field: "web_feed.max_pages",
            });
        }
        Ok(Self {
            selector,
            loading,
            actions,
            extraction,
            max_pages,
        })
    }
    pub fn selector(&self) -> &str {
        self.selector.expression()
    }
    pub fn selector_kind(&self) -> SelectorLanguage {
        self.selector.language()
    }
    pub fn loading(&self) -> WebLoading {
        self.loading
    }
    pub fn actions(&self) -> &WebFeedActions {
        &self.actions
    }
    pub fn extraction(&self) -> &WebExtraction {
        &self.extraction
    }
    pub fn max_pages(&self) -> usize {
        self.max_pages
    }
}

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum WebFeedError {
    #[error("Web feed selector must not be empty")]
    EmptySelector,
    #[error("selector syntax is invalid")]
    InvalidSelector,
    #[error("URL pattern syntax is invalid")]
    InvalidUrlPattern,
    #[error("XPath selection requires browser or automatic loading")]
    XPathRequiresBrowser,
    #[error("configured Web feed limit exceeded: {0}")]
    LimitExceeded(&'static str),
    #[error("configured limit {field} must be greater than zero")]
    ZeroLimit { field: &'static str },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectorDraft {
    pub language: String,
    pub expression: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebFeedRecipeDraft {
    pub workspace_id: Uuid,
    pub url: String,
    pub selector: String,
    pub loading: String,
    pub preview: Option<bool>,
    #[serde(default)]
    pub selector_language: Option<String>,
    #[serde(default)]
    pub viewport: Option<String>,
    #[serde(default)]
    pub listing_url: Option<String>,
    #[serde(default)]
    pub card_selector: Option<String>,
    #[serde(default)]
    pub title_selector: Option<String>,
    #[serde(default)]
    pub date_selector: Option<String>,
    #[serde(default)]
    pub content_selector: Option<String>,
    #[serde(default)]
    pub wait_selector: Option<String>,
    #[serde(default)]
    pub url_pattern: Option<String>,
    #[serde(default)]
    pub max_pages: Option<usize>,
    #[serde(default)]
    pub hide_overlays: Vec<SelectorDraft>,
    #[serde(default)]
    pub start_pages: Vec<String>,
    #[serde(default)]
    pub next_page: Option<SelectorDraft>,
    #[serde(default)]
    pub load_more: Option<SelectorDraft>,
    #[serde(default)]
    pub load_more_clicks: usize,
    #[serde(default)]
    pub scrolls: usize,
}

/// A checked configuration and the exact authored draft travel together. It has
/// no public mutation/deserialization path; construction validates all configured
/// limits before preview/network access. Storage receives this type, never JSON.
#[derive(Clone, Debug)]
pub struct PreparedWebFeed {
    draft: WebFeedRecipeDraft,
    recipe: WebFeedRecipe,
}
impl PreparedWebFeed {
    pub fn new(
        draft: WebFeedRecipeDraft,
        max_pages: usize,
        max_actions: usize,
    ) -> Result<Self, String> {
        if max_pages == 0 || max_actions == 0 {
            return Err("web feed configuration limits must be positive".into());
        }
        if draft.workspace_id.is_nil() {
            return Err("missing workspace identity".into());
        }
        let url = Url::parse(&draft.url).map_err(|_| "invalid web feed URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("invalid web feed URL".into());
        }
        let recipe = recipe_from_draft(&draft, max_pages, max_actions)?;
        Ok(Self { draft, recipe })
    }
    pub fn draft(&self) -> &WebFeedRecipeDraft {
        &self.draft
    }
    pub fn recipe(&self) -> &WebFeedRecipe {
        &self.recipe
    }
}
fn recipe_from_draft(
    draft: &WebFeedRecipeDraft,
    max_pages: usize,
    max_actions: usize,
) -> Result<WebFeedRecipe, String> {
    let loading = match draft.loading.as_str() {
        "automatic" => WebLoading::Automatic,
        "static" => WebLoading::Static,
        "browser" => WebLoading::Browser,
        _ => return Err("invalid web feed loading mode".into()),
    };
    let language = match draft.selector_language.as_deref().unwrap_or("css") {
        "css" => SelectorLanguage::Css,
        "xpath" => SelectorLanguage::XPath,
        _ => return Err("invalid selector language".into()),
    };
    let primary = WebSelector::new(language, draft.selector.clone()).map_err(|e| e.to_string())?;
    let viewport = match draft.viewport.as_deref().unwrap_or("desktop") {
        "desktop" => WebViewport::Desktop,
        "mobile" => WebViewport::Mobile,
        _ => return Err("invalid viewport".into()),
    };
    let parse = |value: &SelectorDraft| {
        let language = match value.language.as_str() {
            "css" => SelectorLanguage::Css,
            "xpath" => SelectorLanguage::XPath,
            _ => return Err("invalid selector language".to_owned()),
        };
        WebSelector::new(language, value.expression.clone()).map_err(|e| e.to_string())
    };
    let overlays = draft
        .hide_overlays
        .iter()
        .map(&parse)
        .collect::<Result<Vec<_>, _>>()?;
    let pages = draft
        .start_pages
        .iter()
        .map(|value| Url::parse(value).map_err(|_| "invalid start page URL".to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    let next = draft.next_page.as_ref().map(&parse).transpose()?;
    let load_more = draft.load_more.as_ref().map(&parse).transpose()?;
    let requested_pages = draft.max_pages.unwrap_or(1);
    if requested_pages == 0 || requested_pages > max_pages {
        return Err("web feed maxPages exceeds configured limit".into());
    }
    let actions = WebFeedActions::new(
        viewport,
        overlays,
        pages,
        next,
        load_more,
        draft.load_more_clicks,
        draft.scrolls,
        max_pages,
        max_actions,
    )
    .map_err(|e| e.to_string())?;
    let listing = draft
        .listing_url
        .as_deref()
        .map(Url::parse)
        .transpose()
        .map_err(|_| "invalid listing URL".to_owned())?;
    let extraction = WebExtraction::new(
        listing,
        draft.card_selector.clone(),
        draft.title_selector.clone(),
        draft.date_selector.clone(),
        draft.content_selector.clone(),
        draft.wait_selector.clone(),
        draft.url_pattern.clone(),
    )
    .map_err(|e| e.to_string())?;
    WebFeedRecipe::configured(primary, loading, actions, extraction, requested_pages)
        .map_err(|e| e.to_string())
}

// Stored execution configurations re-enter the intrinsic validation boundary.
// Deployment-specific action/page budgets are applied by PreparedWebFeed before
// preview and by the collector's runtime policy before browser execution.
impl<'de> Deserialize<'de> for WebSelector {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            language: SelectorLanguage,
            expression: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(raw.language, raw.expression).map_err(serde::de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for WebFeedActions {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            viewport: WebViewport,
            hide_overlays: Vec<WebSelector>,
            start_pages: Vec<Url>,
            next_page: Option<WebSelector>,
            load_more: Option<WebSelector>,
            load_more_clicks: usize,
            scrolls: usize,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(
            raw.viewport,
            raw.hide_overlays,
            raw.start_pages,
            raw.next_page,
            raw.load_more,
            raw.load_more_clicks,
            raw.scrolls,
            usize::MAX,
            usize::MAX,
        )
        .map_err(serde::de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for WebExtraction {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            listing_url: Option<Url>,
            card_selector: Option<WebSelector>,
            title_selector: Option<WebSelector>,
            date_selector: Option<WebSelector>,
            content_selector: Option<WebSelector>,
            wait_selector: Option<WebSelector>,
            url_pattern: Option<String>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let css = |selector: Option<WebSelector>| -> Result<Option<String>, D::Error> {
            selector
                .map(|value| {
                    if value.language != SelectorLanguage::Css {
                        return Err(serde::de::Error::custom("extraction selectors must be CSS"));
                    }
                    Ok(value.expression)
                })
                .transpose()
        };
        Self::new(
            raw.listing_url,
            css(raw.card_selector)?,
            css(raw.title_selector)?,
            css(raw.date_selector)?,
            css(raw.content_selector)?,
            css(raw.wait_selector)?,
            raw.url_pattern,
        )
        .map_err(serde::de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for WebFeedRecipe {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            selector: WebSelector,
            loading: WebLoading,
            actions: WebFeedActions,
            extraction: WebExtraction,
            max_pages: usize,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::configured(
            raw.selector,
            raw.loading,
            raw.actions,
            raw.extraction,
            raw.max_pages,
        )
        .map_err(serde::de::Error::custom)
    }
}
#[cfg(test)]
#[path = "tests/web_feed.rs"]
mod tests;
