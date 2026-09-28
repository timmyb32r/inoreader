use super::{record, rejected, BuiltInAdapterCollector};
use crate::{FetchError, SourceDefinition, SourceRecord};
use http::Method;
use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

impl BuiltInAdapterCollector {
    /// The publisher's all-stories page contains the complete static archive;
    /// homepage Latest cards are only a six-item window. Identity remains the
    /// exact authored absolute article URL used by Dropbox's previous RSS.
    pub(super) async fn dropbox(
        &self,
        source: &SourceDefinition,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let url = Url::parse("https://dropbox.tech/all-stories")
            .map_err(|_| rejected("dropbox_invalid_listing_url"))?;
        let body = self.request(Method::GET, url, None, None).await?;
        parse(source, &body)
    }
}

pub(crate) fn parse(
    source: &SourceDefinition,
    body: &[u8],
) -> Result<Vec<SourceRecord>, FetchError> {
    let text = std::str::from_utf8(body).map_err(|_| rejected("dropbox_invalid_utf8"))?;
    let html = Html::parse_document(text);
    let cards = Selector::parse("li.dr-article-section__list-item").unwrap();
    let links = Selector::parse("a[data-element-id=article-link]").unwrap();
    let titles = Selector::parse("[data-element-id=article-title]").unwrap();
    let dates = Selector::parse("[data-element-id=article-date]").unwrap();
    let mut identities = HashSet::new();
    let mut records = Vec::new();
    for card in html.select(&cards) {
        let mut found = card.select(&links);
        let link = found
            .next()
            .ok_or_else(|| rejected("dropbox_missing_article_link"))?;
        if found.next().is_some() {
            return Err(rejected("dropbox_ambiguous_article_link"));
        }
        let raw = link
            .value()
            .attr("href")
            .ok_or_else(|| rejected("dropbox_missing_article_url"))?;
        let url = Url::parse(raw).map_err(|_| rejected("dropbox_invalid_article_url"))?;
        if url.scheme() != "https" || url.host_str() != Some("dropbox.tech") || url.as_str() != raw
        {
            return Err(rejected("dropbox_unexpected_article_url"));
        }
        if !identities.insert(raw.to_owned()) {
            return Err(rejected("dropbox_duplicate_article_identity"));
        }
        let title = card
            .select(&titles)
            .next()
            .ok_or_else(|| rejected("dropbox_missing_title"))?
            .text()
            .collect::<String>()
            .trim()
            .to_owned();
        if title.is_empty() {
            return Err(rejected("dropbox_empty_title"));
        }
        let date = card
            .select(&dates)
            .next()
            .ok_or_else(|| rejected("dropbox_missing_date"))?
            .text()
            .collect::<String>();
        let published = reader_core::PublicationDate::parse(&date)
            .ok_or_else(|| rejected("dropbox_invalid_date"))?;
        records.push(record(source, url, title, None, Some(published), None)?);
    }
    super::nonempty(records, "dropbox_no_articles")
}
