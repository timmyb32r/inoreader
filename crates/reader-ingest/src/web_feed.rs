use crate::{
    BrowserCapability, BrowserCollector, CacheValidators, FeedFetcher, FetchError,
    SelectorLanguage, SourceDefinition, SourceKind, SourceRecord, WebLoading,
};
use async_trait::async_trait;
use reader_collectors::ParsedRecord;
use scraper::{ElementRef, Html, Selector};
use std::sync::Arc;

/// Decodes only an explicitly supported declared charset, or strict UTF-8 when
/// no charset is declared. Raw bytes remain persisted separately by ingestion.
pub fn decode_html(body: &[u8], content_type: Option<&str>) -> Result<String, FetchError> {
    let label = content_type.and_then(|value| {
        value.split(';').skip(1).find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            key.trim()
                .eq_ignore_ascii_case("charset")
                .then(|| value.trim_matches(['\"', '\'']).trim())
        })
    });
    match label {
        Some(label) => {
            let encoding = encoding_rs::Encoding::for_label(label.as_bytes())
                .ok_or_else(|| FetchError::Rejected("unsupported_declared_charset".into()))?;
            let (value, _, errors) = encoding.decode(body);
            if errors {
                return Err(FetchError::Rejected(
                    "invalid_declared_charset_bytes".into(),
                ));
            }
            Ok(value.into_owned())
        }
        None => std::str::from_utf8(body)
            .map(str::to_owned)
            .map_err(|_| FetchError::Rejected("ambiguous_non_utf8_without_charset".into())),
    }
}

pub fn extract_selected_records(
    source: &SourceDefinition,
    final_url: &url::Url,
    html: &str,
) -> Result<Vec<SourceRecord>, FetchError> {
    let SourceKind::WebPage(recipe) = source.kind() else {
        return Err(FetchError::Rejected("source_is_not_web_feed".into()));
    };
    if recipe.selector_kind() == SelectorLanguage::XPath {
        return Err(FetchError::Rejected("selector_requires_browser".into()));
    }
    let selector = Selector::parse(recipe.selector())
        .map_err(|_| FetchError::Rejected("invalid_css_selector".into()))?;
    let link_selector = Selector::parse("a[href]")
        .map_err(|_| FetchError::Rejected("invalid_internal_selector".into()))?;
    let document = Html::parse_document(html);
    let mut records = Vec::new();
    let mut append =
        |index: usize, node: ElementRef<'_>, card: ElementRef<'_>| -> Result<(), FetchError> {
            let extraction = recipe.extraction();
            let selected_text =
                |selector: Option<&crate::WebSelector>| -> Result<Option<String>, FetchError> {
                    let Some(selector) = selector else {
                        return Ok(None);
                    };
                    let selector = Selector::parse(selector.expression())
                        .map_err(|_| FetchError::Rejected("invalid_extraction_selector".into()))?;
                    Ok(card
                        .select(&selector)
                        .next()
                        .map(|value| value.text().collect::<Vec<_>>().join(" ").trim().to_owned())
                        .filter(|value| !value.is_empty()))
                };
            let title = selected_text(extraction.title_selector())?
                .unwrap_or_else(|| node.text().collect::<Vec<_>>().join(" ").trim().to_owned());
            let href = node
                .value()
                .attr("href")
                .or_else(|| {
                    node.select(&link_selector)
                        .next()
                        .and_then(|v| v.value().attr("href"))
                })
                .map(str::to_owned);
            let absolute = href
                .as_deref()
                .map(|v| final_url.join(v))
                .transpose()
                .map_err(|_| FetchError::Rejected("invalid_selected_url".into()))?;
            if let (Some(pattern), Some(url)) = (extraction.url_pattern(), absolute.as_ref()) {
                if !regex::Regex::new(pattern)
                    .map_err(|_| FetchError::Rejected("invalid_url_pattern".into()))?
                    .is_match(url.as_str())
                {
                    return Ok(());
                }
            }
            let published_at = if let Some(selector) = extraction.date_selector() {
                let selector = Selector::parse(selector.expression())
                    .map_err(|_| FetchError::Rejected("invalid_date_selector".into()))?;
                card.select(&selector).next().and_then(|node| {
                    let raw = node
                        .value()
                        .attr("datetime")
                        .or_else(|| node.value().attr("content"))
                        .map(str::to_owned)
                        .unwrap_or_else(|| node.text().collect::<String>());
                    parse_visible_date(&raw)
                })
            } else {
                None
            };
            let content_html = if let Some(value) = extraction.content_selector() {
                let selector = Selector::parse(value.expression())
                    .map_err(|_| FetchError::Rejected("invalid_content_selector".into()))?;
                card.select(&selector).next().map(|value| value.html())
            } else {
                Some(card.html())
            };
            let upstream = href
                .clone()
                .unwrap_or_else(|| format!("selector:{}:{index}", recipe.selector()));
            let parsed = ParsedRecord {
                categories: None,
                description_media_type: Some("text/plain".into()),
                upstream_id: upstream,
                original_url: href.unwrap_or_default(),
                absolute_url: absolute,
                title,
                description: None,
                content_html,
                published_at,
            };
            records.push(
                SourceRecord::from_parsed(reader_core::SourceRecordId::new(), source.id(), parsed)
                    .map_err(|e| FetchError::Rejected(e.to_string()))?,
            );
            Ok(())
        };
    if let Some(card_selector) = recipe.extraction().card_selector() {
        let card_selector = Selector::parse(card_selector.expression())
            .map_err(|_| FetchError::Rejected("invalid_card_selector".into()))?;
        for (index, card) in document.select(&card_selector).enumerate() {
            if let Some(node) = card.select(&selector).next() {
                append(index, node, card)?
            }
        }
    } else {
        for (index, node) in document.select(&selector).enumerate() {
            append(index, node, node)?
        }
    }
    Ok(records)
}

fn parse_visible_date(value: &str) -> Option<reader_core::PublicationDate> {
    reader_core::PublicationDate::parse(value)
}

/// Conservative in-process readability: prefer an explicitly authored body,
/// then the semantic article/main region with the most non-navigation text.
/// When neither exists, retain the complete sanitized document so
/// extraction failure cannot silently discard content.
pub fn readable_fragment(html: &str) -> String {
    let document = Html::parse_document(html);
    // Prefer an explicit authored body to author biographies, related cards and
    // navigation tagged <main>. Raw page bytes are retained independently.
    for selector in [
        "[itemprop='articleBody']",
        ".post-content .article-content",
        ".post-content",
        ".post__content",
        ".wrap-main-left-article",
        ".entry-content",
        ".blog-content",
        ".wrap-main-left-content",
    ] {
        if let Ok(selector) = Selector::parse(selector) {
            if let Some(node) = document
                .select(&selector)
                .max_by_key(|n| n.text().map(str::len).sum::<usize>())
            {
                return node.html();
            }
        }
    }
    let selector = Selector::parse("article, main").expect("static selector");
    if let Some(node) = document.select(&selector).max_by_key(|n| {
        let text = n.text().map(str::len).sum::<usize>();
        let links = Selector::parse("a").expect("static selector");
        let navigation = n
            .select(&links)
            .map(|a| a.text().map(str::len).sum::<usize>())
            .sum::<usize>();
        text.saturating_sub(navigation)
    }) {
        return node.html();
    }
    html.to_owned()
}

/// Production collector for static Web feeds. Browser recipes deliberately
/// return the degraded diagnostic until the CDP adapter has completed its
/// capability probe; static recipes remain available.
pub struct StaticWebFeedCollector<F> {
    fetcher: Arc<F>,
}
impl<F> StaticWebFeedCollector<F> {
    pub fn new(fetcher: Arc<F>) -> Self {
        Self { fetcher }
    }
}

#[async_trait]
impl<F: FeedFetcher> BrowserCollector for StaticWebFeedCollector<F> {
    fn capability(&self) -> BrowserCapability {
        BrowserCapability::Available
    }
    async fn collect(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        let SourceKind::WebPage(recipe) = source.kind() else {
            return Err(FetchError::Rejected("source_is_not_web_feed".into()));
        };
        if recipe.loading() == WebLoading::Browser {
            return Err(FetchError::Rejected("browser_degraded".into()));
        }
        let fetch_url = recipe.extraction().listing_url().unwrap_or(source.url());
        let page = self
            .fetcher
            .fetch(fetch_url, &CacheValidators::default())
            .await?;
        let text = decode_html(&page.body, page.content_type.as_deref())?;
        extract_selected_records(source, &page.final_url, &text)
    }
}
