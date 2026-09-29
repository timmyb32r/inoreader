use super::{record, rejected, BuiltInAdapterCollector};
use crate::{FetchError, SourceDefinition, SourceRecord};
use chrono::{DateTime, Utc};
use http::Method;
use serde::Deserialize;
use std::collections::HashSet;
use url::Url;

#[derive(Deserialize)]
struct Listing {
    err_no: i64,
    data: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    content: Content,
}
#[derive(Deserialize)]
struct Content {
    item_id: String,
    name: String,
    r#abstract: String,
    publish_time: i64,
}
impl BuiltInAdapterCollector {
    /// Collect the public latest window, not the homepage recommendation ranking.
    /// IDs remain exact decimal strings; publisher Unix seconds become UTC dates.
    /// Any malformed or duplicate entry rejects the entire collection before commit.
    pub(super) async fn volcengine(
        &self,
        source: &SourceDefinition,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let url = Url::parse(
            "https://developer.volcengine.com/api/fe/v1/articles?cursor=&category=&tag=",
        )
        .map_err(|_| rejected("volcengine_invalid_endpoint"))?;
        let body = self
            .request(Method::GET, url, Some(source.url()), None)
            .await?;
        let listing: Listing =
            serde_json::from_slice(&body).map_err(|_| rejected("volcengine_invalid_listing"))?;
        if listing.err_no != 0 {
            return Err(rejected("volcengine_unsuccessful_listing"));
        }
        let mut ids = HashSet::new();
        listing
            .data
            .into_iter()
            .map(|entry| {
                let c = entry.content;
                if c.item_id.is_empty()
                    || !c.item_id.bytes().all(|b| b.is_ascii_digit())
                    || !ids.insert(c.item_id.clone())
                {
                    return Err(rejected("volcengine_invalid_or_duplicate_identity"));
                }
                if c.name.trim().is_empty() {
                    return Err(rejected("volcengine_empty_title"));
                }
                let date = DateTime::<Utc>::from_timestamp(c.publish_time, 0)
                    .ok_or_else(|| rejected("volcengine_invalid_timestamp"))?;
                let published = reader_core::PublicationDate::parse(&date.to_rfc3339())
                    .ok_or_else(|| rejected("volcengine_invalid_publication_date"))?;
                let url = Url::parse(&format!(
                    "https://developer.volcengine.com/articles/{}",
                    c.item_id
                ))
                .map_err(|_| rejected("volcengine_invalid_article_url"))?;
                record(
                    source,
                    url,
                    c.name,
                    Some(c.r#abstract),
                    Some(published),
                    None,
                )
            })
            .collect()
    }
}
