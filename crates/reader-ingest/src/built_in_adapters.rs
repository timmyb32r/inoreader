use crate::{
    BrowserHttpClient, BuiltInAdapter, FetchError, SourceDefinition, SourceKind, SourceRecord,
};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};
use http::{HeaderMap, HeaderValue, Method};
use reader_collectors::ParsedRecord;
use reader_web_runtime::PreparedRequest;
use scraper::{Html, Selector};
use std::{collections::HashSet, sync::Arc};
use url::Url;

/// Source-specific public adapters retained from `personal_feed`. All requests
/// still cross the shared outbound client; adapters only own publisher schemas.
pub struct BuiltInAdapterCollector {
    http: Arc<dyn BrowserHttpClient>,
}
impl BuiltInAdapterCollector {
    pub fn new(http: Arc<dyn BrowserHttpClient>) -> Self {
        Self { http }
    }
    pub async fn collect(
        &self,
        source: &SourceDefinition,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let SourceKind::BuiltIn(adapter) = source.kind() else {
            return Err(rejected("source_is_not_builtin_adapter"));
        };
        match adapter {
            BuiltInAdapter::Cloudera { listing_url } => self.cloudera(source, listing_url).await,
            BuiltInAdapter::Digoal { listing_url } => self.digoal(source, listing_url).await,
            BuiltInAdapter::Mirrorship => self.mirrorship(source).await,
            BuiltInAdapter::Pingkai { listing_url } => self.pingkai(source, listing_url).await,
            BuiltInAdapter::ModbNews => self.modb(source).await,
            BuiltInAdapter::InfoqBigdata => self.infoq(source).await,
            BuiltInAdapter::Highgo { max_pages } => self.highgo(source, *max_pages).await,
        }
    }
    async fn request(
        &self,
        method: Method,
        url: Url,
        referer: Option<&Url>,
        body: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, FetchError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        if let Some(referer) = referer {
            headers.insert(
                http::header::REFERER,
                HeaderValue::from_str(referer.as_str())
                    .map_err(|_| rejected("invalid_adapter_referer"))?,
            );
            let origin = format!("{}://{}", referer.scheme(), referer.authority());
            headers.insert(
                http::header::ORIGIN,
                HeaderValue::from_str(&origin).map_err(|_| rejected("invalid_adapter_origin"))?,
            );
        }
        if body.is_some() {
            headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
        let response = self
            .http
            .execute(PreparedRequest {
                method,
                url,
                headers,
                body,
            })
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(rejected("publisher_api_http_error"));
        }
        Ok(response.body)
    }
    async fn cloudera(
        &self,
        source: &SourceDefinition,
        url: &Url,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let body = self.request(Method::GET, url.clone(), None, None).await?;
        let value: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| rejected("cloudera_invalid_json"))?;
        let rows = value
            .get("Articles")
            .or_else(|| value.get("articles"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| rejected("cloudera_missing_articles"))?;
        let mut out = Vec::new();
        for row in rows {
            let title = text_field(row, &["Title", "title"]);
            let href = row
                .pointer("/ArticleLink/LinkAttrs/Href")
                .or_else(|| row.pointer("/articleLink/linkAttrs/href"))
                .and_then(|v| v.as_str());
            if title.is_empty() {
                continue;
            }
            let Some(url) = href.and_then(|v| source.url().join(v).ok()) else {
                continue;
            };
            let published = text_field(row, &["PublishedDateFormatted", "publishedDateFormatted"]);
            out.push(record(
                source,
                url,
                title,
                None,
                parse_date(&published),
                None,
            )?)
        }
        nonempty(out, "cloudera_no_articles")
    }
    async fn infoq(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        let endpoint = Url::parse("https://www.infoq.cn/public/v1/article/getList")
            .map_err(|_| rejected("invalid_builtin_url"))?;
        let body = serde_json::to_vec(&serde_json::json!({"id":15,"type":1,"ptype":0,"size":30}))
            .map_err(|_| rejected("infoq_request_encode"))?;
        let body = self
            .request(Method::POST, endpoint, Some(source.url()), Some(body))
            .await?;
        let root: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| rejected("infoq_invalid_json"))?;
        if root.get("code").and_then(|v| v.as_i64()) != Some(0) {
            return Err(rejected("infoq_unsuccessful"));
        }
        let rows = root
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or_else(|| rejected("infoq_missing_data"))?;
        let slug = regex::Regex::new(r"^[A-Za-z0-9_-]+$").unwrap();
        let mut out = Vec::new();
        for row in rows {
            let Some(id) = row
                .get("uuid")
                .and_then(|v| v.as_str())
                .filter(|v| slug.is_match(v))
            else {
                continue;
            };
            let title = text_field(row, &["article_title"]);
            let Some(ms) = row
                .get("publish_time")
                .and_then(|v| v.as_i64())
                .filter(|v| *v > 0)
            else {
                continue;
            };
            if title.is_empty() {
                continue;
            }
            let url = Url::parse(&format!("https://www.infoq.cn/article/{id}"))
                .map_err(|_| rejected("infoq_invalid_article_url"))?;
            out.push(record(
                source,
                url,
                title,
                string_field(row, "article_summary"),
                DateTime::from_timestamp_millis(ms),
                None,
            )?)
        }
        nonempty(out, "infoq_no_articles")
    }
    async fn modb(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        let endpoint =
            Url::parse("https://www.modb.pro/api/knowledges/find/v2?type=3&pageSize=30&pageNum=1")
                .map_err(|_| rejected("invalid_builtin_url"))?;
        let body = self
            .request(Method::GET, endpoint, Some(source.url()), None)
            .await?;
        let root: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| rejected("modb_invalid_json"))?;
        let rows = root
            .get("list")
            .and_then(|v| v.as_array())
            .ok_or_else(|| rejected("modb_missing_list"))?;
        let mut out = Vec::new();
        for row in rows {
            let Some(id) = row
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|v| v.bytes().all(|b| b.is_ascii_digit()))
            else {
                continue;
            };
            if row
                .get("encryptLevel")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.is_empty() && v != "PUBLIC")
            {
                continue;
            }
            let title = text_field(row, &["title"]);
            let published = string_field(row, "createdTime")
                .and_then(|v| NaiveDateTime::parse_from_str(&v, "%Y-%m-%d %H:%M:%S").ok())
                .and_then(|v| {
                    FixedOffset::east_opt(8 * 3600)?
                        .from_local_datetime(&v)
                        .single()
                })
                .map(|v| v.with_timezone(&Utc));
            if title.is_empty() || published.is_none() {
                continue;
            }
            let url = Url::parse(&format!("https://www.modb.pro/db/{id}"))
                .map_err(|_| rejected("modb_invalid_article_url"))?;
            out.push(record(
                source,
                url,
                title,
                string_field(row, "brief"),
                published,
                None,
            )?)
        }
        nonempty(out, "modb_no_articles")
    }
    async fn highgo(
        &self,
        source: &SourceDefinition,
        max_pages: usize,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        if !(1..=20).contains(&max_pages) {
            return Err(rejected("highgo_page_limit"));
        }
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for page in 1..=max_pages {
            let endpoint = Url::parse(&format!(
                "https://www.highgo.com/wsapi/news/list?pageNum={page}&pageSize=20"
            ))
            .map_err(|_| rejected("invalid_builtin_url"))?;
            let body = self.request(Method::GET, endpoint, None, None).await?;
            let root: serde_json::Value =
                serde_json::from_slice(&body).map_err(|_| rejected("highgo_invalid_json"))?;
            let total = root
                .get("total")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| rejected("highgo_missing_total"))?;
            let rows = root
                .get("rows")
                .and_then(|v| v.as_array())
                .ok_or_else(|| rejected("highgo_missing_rows"))?;
            for row in rows {
                let Some(id) = row.get("id").and_then(|v| v.as_i64()).filter(|v| *v > 0) else {
                    continue;
                };
                let kind = row.get("types").and_then(|v| v.as_i64()).unwrap_or(0);
                if !matches!(kind, 0 | 1) {
                    continue;
                }
                let title = text_field(row, &["title"]);
                let published = string_field(row, "publishDate")
                    .and_then(|v| NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok())
                    .and_then(|v| v.and_hms_opt(0, 0, 0))
                    .and_then(|v| {
                        FixedOffset::east_opt(8 * 3600)?
                            .from_local_datetime(&v)
                            .single()
                    })
                    .map(|v| v.with_timezone(&Utc));
                let url = if kind == 1 {
                    string_field(row, "outUrl").and_then(|v| Url::parse(&v).ok())
                } else {
                    Url::parse(&format!("https://www.highgo.com/about/news/{id}")).ok()
                };
                let Some(url) = url
                    .filter(|v| matches!(v.scheme(), "http" | "https") && v.host_str().is_some())
                else {
                    continue;
                };
                if title.is_empty() || published.is_none() || !seen.insert(url.to_string()) {
                    continue;
                }
                out.push(record(
                    source,
                    url,
                    title,
                    string_field(row, "intro"),
                    published,
                    None,
                )?)
            }
            if page as u64 * 20 >= total || out.len() >= 100 {
                break;
            }
        }
        out.truncate(100);
        nonempty(out, "highgo_no_articles")
    }
    async fn pingkai(
        &self,
        source: &SourceDefinition,
        listing: &Url,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let mut listing = listing.clone();
        listing.query_pairs_mut().append_pair("latest", "true");
        let body = self.request(Method::GET, listing, None, None).await?;
        let html = std::str::from_utf8(&body).map_err(|_| rejected("pingkai_invalid_utf8"))?;
        let document = Html::parse_document(html);
        let selector = Selector::parse("script#__NEXT_DATA__").unwrap();
        let raw = document
            .select(&selector)
            .next()
            .map(|v| v.text().collect::<String>())
            .ok_or_else(|| rejected("pingkai_missing_next_data"))?;
        let root: serde_json::Value =
            serde_json::from_str(&raw).map_err(|_| rejected("pingkai_invalid_next_data"))?;
        let rows = root
            .pointer("/props/pageProps/blogs/content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| rejected("pingkai_missing_articles"))?;
        let mut out = Vec::new();
        for row in rows {
            if row.get("status").and_then(|v| v.as_str()) != Some("PUBLISHED") {
                continue;
            }
            let Some(slug) = row
                .get("slug")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty() && v.len() <= 200 && !v.contains(['/', '?', '#']))
            else {
                continue;
            };
            let title = text_field(row, &["title"]);
            let published = string_field(row, "publishedAt").and_then(|v| parse_date(&v));
            if title.is_empty() || published.is_none() {
                continue;
            }
            let url = source
                .url()
                .join(slug)
                .map_err(|_| rejected("pingkai_invalid_slug"))?;
            out.push(record(
                source,
                url,
                title,
                string_field(row, "summary"),
                published,
                None,
            )?)
        }
        nonempty(out, "pingkai_no_articles")
    }
    async fn digoal(
        &self,
        source: &SourceDefinition,
        listing: &Url,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let body = self
            .request(Method::GET, listing.clone(), None, None)
            .await?;
        let text = std::str::from_utf8(&body).map_err(|_| rejected("digoal_invalid_utf8"))?;
        let pattern = regex::Regex::new(r"\[([^\]]+)\]\((\d{6}/\d{8}_\d{2}\.md)\)").unwrap();
        let mut out = Vec::new();
        for captures in pattern.captures_iter(text).take(100) {
            let title = captures.get(1).unwrap().as_str().trim().to_owned();
            let path = captures.get(2).unwrap().as_str();
            let date = NaiveDate::parse_from_str(&path[7..15], "%Y%m%d")
                .ok()
                .and_then(|v| v.and_hms_opt(0, 0, 0))
                .map(|v| DateTime::from_naive_utc_and_offset(v, Utc));
            let url = Url::parse(&format!(
                "https://github.com/digoal/blog/blob/master/{path}"
            ))
            .map_err(|_| rejected("digoal_invalid_path"))?;
            out.push(record(source, url, title, None, date, None)?)
        }
        nonempty(out, "digoal_no_articles")
    }
    async fn mirrorship(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        let mut cards = Vec::new();
        let selector = Selector::parse("a.blog[href]").unwrap();
        let title_selector = Selector::parse(".title").unwrap();
        for suffix in ["", "Comparison", "technical", "product"] {
            let url = source
                .url()
                .join(&format!("{}/", suffix))
                .map_err(|_| rejected("mirrorship_invalid_category"))?;
            let body = self.request(Method::GET, url.clone(), None, None).await?;
            let html =
                std::str::from_utf8(&body).map_err(|_| rejected("mirrorship_invalid_utf8"))?;
            let document = Html::parse_document(html);
            for node in document.select(&selector) {
                let Some(href) = node.value().attr("href") else {
                    continue;
                };
                let Some(url) = url.join(href).ok().filter(|v| {
                    v.host_str() == source.url().host_str()
                        && v.path().starts_with("/zh-CN/blog/d/")
                }) else {
                    continue;
                };
                let title = node
                    .select(&title_selector)
                    .next()
                    .map(|v| v.text().collect::<Vec<_>>().join(" ").trim().to_owned())
                    .unwrap_or_default();
                if !title.is_empty() {
                    cards.push((url, title))
                }
            }
        }
        let mut seen = HashSet::new();
        cards.retain(|(url, _)| seen.insert(url.to_string()));
        if cards.len() > 120 {
            return Err(rejected("mirrorship_article_limit"));
        }
        let mut out = Vec::new();
        let h1 = Selector::parse("h1.title").unwrap();
        let content = Selector::parse(".ck-content.content").unwrap();
        let date_pattern = regex::Regex::new(r"new Date\(\s*(\d{13})\s*\)").unwrap();
        for (url, listed_title) in cards {
            let body = self.request(Method::GET, url.clone(), None, None).await?;
            let html =
                std::str::from_utf8(&body).map_err(|_| rejected("mirrorship_invalid_utf8"))?;
            let document = Html::parse_document(html);
            let title = document
                .select(&h1)
                .next()
                .map(|v| v.text().collect::<Vec<_>>().join(" ").trim().to_owned())
                .unwrap_or_default();
            if title != listed_title {
                return Err(rejected("mirrorship_title_mismatch"));
            }
            let Some(content) = document.select(&content).next() else {
                return Err(rejected("mirrorship_missing_content"));
            };
            let raw_date = content.text().collect::<String>();
            let published = date_pattern
                .captures(&raw_date)
                .and_then(|v| v.get(1))
                .and_then(|v| v.as_str().parse::<i64>().ok())
                .and_then(DateTime::from_timestamp_millis);
            out.push(record(
                source,
                url,
                title,
                None,
                published,
                Some(content.html()),
            )?)
        }
        nonempty(out, "mirrorship_no_articles")
    }
}

fn rejected(value: &str) -> FetchError {
    FetchError::Rejected(value.to_owned())
}
fn string_field(value: &serde_json::Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}
fn text_field(value: &serde_json::Value, names: &[&str]) -> String {
    names
        .iter()
        .find_map(|name| string_field(value, name))
        .unwrap_or_default()
}
fn parse_date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|v| v.with_timezone(&Utc))
        .or_else(|| {
            ["%Y-%m-%d", "%m/%d/%Y", "%B %d, %Y", "%b %d, %Y"]
                .iter()
                .find_map(|format| {
                    NaiveDate::parse_from_str(value.trim(), format)
                        .ok()
                        .and_then(|v| v.and_hms_opt(0, 0, 0))
                        .map(|v| DateTime::from_naive_utc_and_offset(v, Utc))
                })
        })
}
fn record(
    source: &SourceDefinition,
    url: Url,
    title: String,
    description: Option<String>,
    published_at: Option<DateTime<Utc>>,
    content_html: Option<String>,
) -> Result<SourceRecord, FetchError> {
    let parsed = ParsedRecord {
        upstream_id: url.to_string(),
        original_url: url.to_string(),
        absolute_url: Some(url),
        title,
        description,
        content_html,
        published_at,
    };
    SourceRecord::from_parsed(reader_core::SourceRecordId::new(), source.id(), parsed)
        .map_err(|e| rejected(&e.to_string()))
}
fn nonempty(values: Vec<SourceRecord>, diagnostic: &str) -> Result<Vec<SourceRecord>, FetchError> {
    if values.is_empty() {
        Err(rejected(diagnostic))
    } else {
        Ok(values)
    }
}
