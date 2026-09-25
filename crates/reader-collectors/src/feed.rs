use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedRecord {
    pub upstream_id: String,
    pub original_url: String,
    pub absolute_url: Option<Url>,
    pub title: String,
    pub description: Option<String>,
    pub content_html: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error)]
pub enum FeedError {
    #[error("feed cannot be parsed: {0}")]
    Invalid(String),
    #[error("entry has neither stable id nor URL")]
    MissingIdentity,
    #[error("entry URL is invalid: {0}")]
    InvalidUrl(String),
}

pub fn parse_xml(bytes: &[u8], feed_url: &Url) -> Result<Vec<ParsedRecord>, FeedError> {
    let feed = feed_rs::parser::parse(bytes).map_err(|e| FeedError::Invalid(e.to_string()))?;
    feed.entries
        .into_iter()
        .map(|entry| {
            let link = entry
                .links
                .first()
                .map(|v| v.href.clone())
                .unwrap_or_default();
            let upstream_id = if entry.id.is_empty() {
                link.clone()
            } else {
                entry.id
            };
            if upstream_id.is_empty() {
                return Err(FeedError::MissingIdentity);
            }
            let absolute_url = if link.is_empty() {
                None
            } else {
                Some(
                    feed_url
                        .join(&link)
                        .map_err(|_| FeedError::InvalidUrl(link.clone()))?,
                )
            };
            Ok(ParsedRecord {
                upstream_id,
                original_url: link,
                absolute_url,
                title: entry.title.map(|v| v.content).unwrap_or_default(),
                description: entry.summary.map(|v| v.content),
                content_html: entry.content.and_then(|v| v.body),
                published_at: entry.published,
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct JsonFeed {
    items: Vec<JsonItem>,
}
#[derive(Deserialize)]
struct JsonItem {
    id: Option<String>,
    url: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    date_published: Option<DateTime<Utc>>,
}

pub fn parse_json(bytes: &[u8], feed_url: &Url) -> Result<Vec<ParsedRecord>, FeedError> {
    let feed: JsonFeed =
        serde_json::from_slice(bytes).map_err(|e| FeedError::Invalid(e.to_string()))?;
    feed.items
        .into_iter()
        .map(|item| {
            let link = item.url.unwrap_or_default();
            let upstream_id = item.id.unwrap_or_else(|| link.clone());
            if upstream_id.is_empty() {
                return Err(FeedError::MissingIdentity);
            }
            let absolute_url = if link.is_empty() {
                None
            } else {
                Some(
                    feed_url
                        .join(&link)
                        .map_err(|_| FeedError::InvalidUrl(link.clone()))?,
                )
            };
            Ok(ParsedRecord {
                upstream_id,
                original_url: link,
                absolute_url,
                title: item.title.unwrap_or_default(),
                description: item.summary,
                content_html: item.content_html,
                published_at: item.date_published,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
