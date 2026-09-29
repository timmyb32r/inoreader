use super::{record, rejected, BuiltInAdapterCollector};
use crate::{FetchError, SourceDefinition, SourceRecord};
use chrono::{DateTime, Utc};
use http::Method;
use serde::Deserialize;
use std::collections::HashSet;
use url::Url;
#[derive(Deserialize)]
struct Listing {
    code: i64,
    result: Rows,
}
#[derive(Deserialize)]
struct Rows {
    rows: Vec<Article>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Article {
    id: u64,
    name: String,
    release_time: i64,
}
impl BuiltInAdapterCollector {
    /// Public article window without the homepage's recommendation-only tag.
    /// Publisher IDs remain decimal u64; releaseTime is Unix milliseconds.
    pub(super) async fn jdcloud(
        &self,
        source: &SourceDefinition,
        page_size: usize,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let url=Url::parse(&format!("https://developer.jdcloud.com/api/article/list?middleArticleId=&status=0&type=1&pageNum=1&pageSize={page_size}&tagName=&orderByType=2&hasReadNum=2"))
            .map_err(|_| rejected("jdcloud_invalid_endpoint"))?;
        let body = self
            .request(Method::GET, url, Some(source.url()), None)
            .await?;
        let listing: Listing =
            serde_json::from_slice(&body).map_err(|_| rejected("jdcloud_invalid_listing"))?;
        if listing.code != 0 {
            return Err(rejected("jdcloud_unsuccessful_listing"));
        }
        let mut ids = HashSet::new();
        listing
            .result
            .rows
            .into_iter()
            .map(|row| {
                if row.id == 0 || !ids.insert(row.id) {
                    return Err(rejected("jdcloud_invalid_or_duplicate_identity"));
                }
                if row.name.trim().is_empty() {
                    return Err(rejected("jdcloud_empty_title"));
                }
                let date = DateTime::<Utc>::from_timestamp_millis(row.release_time)
                    .ok_or_else(|| rejected("jdcloud_invalid_timestamp"))?;
                let published = reader_core::PublicationDate::parse(&date.to_rfc3339())
                    .ok_or_else(|| rejected("jdcloud_invalid_publication_date"))?;
                let url = Url::parse(&format!("https://developer.jdcloud.com/article/{}", row.id))
                    .map_err(|_| rejected("jdcloud_invalid_article_url"))?;
                record(source, url, row.name, None, Some(published), None)
            })
            .collect()
    }
}
