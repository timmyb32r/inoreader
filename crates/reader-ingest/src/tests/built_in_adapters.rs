use crate::*;
use crate::{BrowserHttpResponse, SourceKind};
use async_trait::async_trait;
use http::HeaderMap;
use reader_web_runtime::PreparedRequest;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use url::Url;

struct MockHttp {
    responses: Mutex<VecDeque<Vec<u8>>>,
    requests: Mutex<Vec<Url>>,
}
impl MockHttp {
    fn new(values: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(values.iter().map(|v| v.as_bytes().to_vec()).collect()),
            requests: Mutex::new(Vec::new()),
        })
    }
    fn requests(&self) -> Vec<Url> {
        self.requests.lock().unwrap().clone()
    }
}
#[async_trait]
impl BrowserHttpClient for MockHttp {
    async fn execute(&self, request: PreparedRequest) -> Result<BrowserHttpResponse, FetchError> {
        self.requests.lock().unwrap().push(request.url.clone());
        let body = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| FetchError::Rejected("unexpected_request".into()))?;
        Ok(BrowserHttpResponse {
            final_url: request.url,
            status: 200,
            headers: HeaderMap::new(),
            body,
        })
    }
}
fn source(kind: BuiltInAdapter) -> SourceDefinition {
    SourceDefinition::new(
        reader_core::SourceId::new(),
        Url::parse("https://publisher.example/blog/").unwrap(),
        SourceKind::BuiltIn(kind),
    )
    .unwrap()
}

#[tokio::test]
async fn cloudera_schema_maps_nested_article_link() {
    let http = MockHttp::new(&[
        r#"{"Articles":[{"Title":"A","PublishedDateFormatted":"September 18, 2026","ArticleLink":{"LinkAttrs":{"Href":"/a"}}}]}"#,
    ]);
    let collector = BuiltInAdapterCollector::new(http);
    let values = collector
        .collect(&source(BuiltInAdapter::Cloudera {
            listing_url: Url::parse("https://publisher.example/list.json").unwrap(),
        }))
        .await
        .unwrap();
    assert_eq!(values[0].key().title, "A");
    assert_eq!(
        values[0].key().location.exact_url(),
        Some("https://publisher.example/a")
    );
}
#[tokio::test]
async fn infoq_requires_success_envelope_and_public_identity() {
    let http = MockHttp::new(&[
        r#"{"code":0,"data":[{"uuid":"newer","article_title":"InfoQ","article_summary":"Summary","publish_time":1789108800096}]}"#,
    ]);
    let values = BuiltInAdapterCollector::new(http)
        .collect(&source(BuiltInAdapter::InfoqBigdata))
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].published_at().unwrap().timestamp_millis(),
        1789108800096
    );
}
#[tokio::test]
async fn modb_filters_non_public_rows_and_parses_shanghai_time() {
    let http = MockHttp::new(&[
        r#"{"list":[{"id":"2100803773564280832","title":"Public","brief":"B","createdTime":"2026-09-18 12:29:38","encryptLevel":"PUBLIC"},{"id":"2","title":"Private","createdTime":"2026-09-18 12:29:38","encryptLevel":"PRIVATE"}]}"#,
    ]);
    let values = BuiltInAdapterCollector::new(http)
        .collect(&source(BuiltInAdapter::ModbNews))
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].published_at().unwrap().to_rfc3339(),
        "2026-09-18T04:29:38+00:00"
    );
}
#[tokio::test]
async fn highgo_uses_rows_not_promoted_tops_and_validates_external_url() {
    let http = MockHttp::new(&[
        r#"{"total":2,"tops":[{"id":99,"title":"Ignore"}],"rows":[{"id":943,"title":"Internal","intro":"S","types":0,"publishDate":"2026-08-25"},{"id":944,"title":"External","types":1,"outUrl":"https://mp.weixin.qq.com/s/article","publishDate":"2026-08-24"}]}"#,
    ]);
    let values = BuiltInAdapterCollector::new(http)
        .collect(&source(BuiltInAdapter::Highgo { max_pages: 5 }))
        .await
        .unwrap();
    assert_eq!(values.len(), 2);
    assert!(values
        .iter()
        .any(|v| v.upstream_id() == "https://mp.weixin.qq.com/s/article"));
}
#[tokio::test]
async fn pingkai_uses_chronological_next_data_only() {
    let http = MockHttp::new(&[
        r#"<script id="__NEXT_DATA__">{"props":{"pageProps":{"blogs":{"content":[{"slug":"post","status":"PUBLISHED","title":"Post","summary":"S","publishedAt":"2026-09-18T12:00:00Z"},{"slug":"draft","status":"DRAFT","title":"Draft","publishedAt":"2026-09-18T12:00:00Z"}]}}}}</script>"#,
    ]);
    let values = BuiltInAdapterCollector::new(http)
        .collect(&source(BuiltInAdapter::Pingkai {
            listing_url: Url::parse("https://publisher.example/blog").unwrap(),
        }))
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].key().title, "Post");
}
#[tokio::test]
async fn digoal_accepts_only_dated_markdown_paths() {
    let http = MockHttp::new(&["[Valid](202609/20260916_01.md)\n[Other](README.md)"]);
    let values = BuiltInAdapterCollector::new(http)
        .collect(&source(BuiltInAdapter::Digoal {
            listing_url: Url::parse(
                "https://raw.githubusercontent.com/digoal/blog/master/README.md",
            )
            .unwrap(),
        }))
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].published_at().unwrap().date_naive().to_string(),
        "2026-09-16"
    );
}
#[tokio::test]
async fn mirrorship_requires_listing_detail_title_identity() {
    let listing =
        r#"<a class="blog" href="/zh-CN/blog/d/a"><span class="title">Article A</span></a>"#;
    let detail = r#"<h1 class="title">Article A</h1><div class="ck-content content"><span>new Date(1789516800000)</span><p>A complete article body with enough independently meaningful words for the archived content contract.</p></div>"#;
    let http = MockHttp::new(&[listing, listing, listing, listing, detail]);
    let source = SourceDefinition::new(
        reader_core::SourceId::new(),
        Url::parse("https://publisher.example/zh-CN/blog").unwrap(),
        SourceKind::BuiltIn(BuiltInAdapter::Mirrorship),
    )
    .unwrap();
    let values = BuiltInAdapterCollector::new(http.clone())
        .collect(&source)
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].key().title, "Article A");
    assert_eq!(
        http.requests()[..4]
            .iter()
            .map(Url::as_str)
            .collect::<Vec<_>>(),
        [
            "https://publisher.example/zh-CN/blog/",
            "https://publisher.example/zh-CN/blog/Comparison/",
            "https://publisher.example/zh-CN/blog/technical/",
            "https://publisher.example/zh-CN/blog/product/",
        ]
    );
}
