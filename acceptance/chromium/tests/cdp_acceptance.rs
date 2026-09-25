use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use http::{header, HeaderMap, HeaderValue};
use reader_core::SourceId;
use reader_ingest::{
    BrowserCapability, BrowserCollector, BrowserHttpClient, BrowserHttpResponse,
    CdpBrowserCollector, FetchError, SelectorLanguage, SourceDefinition, SourceKind, WebExtraction,
    WebFeedActions, WebFeedRecipe, WebLoading, WebSelector, WebViewport,
};
use reader_web_runtime::{validate_public_address, PreparedRequest};
use url::Url;

const HTML: &str = r#"<!doctype html><html><head><style>.card{display:block;width:240px;height:48px}</style></head><body>
<div class="overlay">consent</div><main id="items"><a class="card" href="/article/first">Article first</a><a class="card" href="/article/second">Article second</a></main><button class="more" onclick="add('third')">More</button>
<script>function add(id){const a=document.createElement('a');a.className='card';a.href='/article/'+id;a.textContent='Article '+id;document.querySelector('#items').append(a)};document.body.dataset.ready='yes'</script>
</body></html>"#;

#[derive(Default)]
struct FixtureHttp {
    requested: Mutex<Vec<String>>,
    pages: HashMap<&'static str, &'static str>,
}

impl FixtureHttp {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            requested: Mutex::new(Vec::new()),
            pages: HashMap::from([("fixture.test", HTML)]),
        })
    }
}

#[async_trait]
impl BrowserHttpClient for FixtureHttp {
    async fn execute(&self, request: PreparedRequest) -> Result<BrowserHttpResponse, FetchError> {
        self.requested
            .lock()
            .unwrap()
            .push(request.url.as_str().to_owned());
        if let Some(host) = request.url.host_str() {
            if host
                .parse::<IpAddr>()
                .is_ok_and(|ip| validate_public_address(ip).is_err())
            {
                return Err(FetchError::Rejected("ssrf_destination_blocked".into()));
            }
        }
        let body = self
            .pages
            .get(request.url.host_str().unwrap_or_default())
            .ok_or_else(|| FetchError::Rejected("fixture_host_not_allowed".into()))?;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        Ok(BrowserHttpResponse {
            final_url: request.url,
            status: 200,
            headers,
            body: body.as_bytes().to_vec(),
        })
    }
}

fn recipe() -> WebFeedRecipe {
    let selector = WebSelector::new(SelectorLanguage::Css, "a.card".into()).unwrap();
    let overlay = WebSelector::new(SelectorLanguage::Css, ".overlay".into()).unwrap();
    let more = WebSelector::new(SelectorLanguage::Css, "button.more".into()).unwrap();
    let actions = WebFeedActions::new(
        WebViewport::Desktop,
        vec![overlay],
        vec![],
        None,
        Some(more),
        1,
        0,
        2,
        4,
    )
    .unwrap();
    let extraction = WebExtraction::new(
        None,
        None,
        None,
        None,
        None,
        Some("body[data-ready=yes]".into()),
        None,
    )
    .unwrap();
    WebFeedRecipe::legacy(selector, WebLoading::Browser, actions, extraction, 1).unwrap()
}

fn source(url: &str) -> SourceDefinition {
    SourceDefinition::new(
        SourceId::new(),
        Url::parse(url).unwrap(),
        SourceKind::WebPage(recipe()),
    )
    .unwrap()
}

fn endpoint() -> String {
    std::env::var("INOREADER_CHROMIUM_CDP")
        .expect("INOREADER_CHROMIUM_CDP must point at the Docker Chromium started by tools/run_chromium_acceptance.sh")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_chromium_keeps_preview_and_scheduled_collection_in_parity() {
    let http = FixtureHttp::new();
    let collector =
        CdpBrowserCollector::probe(endpoint(), http.clone(), 2, Duration::from_secs(10), 2)
            .await
            .unwrap();

    let preview = collector
        .visual_snapshot(&Url::parse("https://fixture.test/").unwrap(), false)
        .await
        .unwrap();
    assert_eq!((preview.width, preview.height), (1440, 900));
    assert!(preview.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    let cards = preview
        .groups
        .iter()
        .find(|group| group.selector == "a.card")
        .expect("preview must expose selectable card geometry");
    assert!(!cards.boxes.is_empty());

    let first = collector
        .collect(&source("https://fixture.test/"))
        .await
        .unwrap();
    let scheduled = collector
        .collect(&source("https://fixture.test/"))
        .await
        .unwrap();
    let titles = |values: &[reader_ingest::SourceRecord]| {
        values
            .iter()
            .map(|record| record.key().title.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        titles(&first),
        vec![
            "Article first".to_owned(),
            "Article second".to_owned(),
            "Article third".to_owned()
        ]
    );
    assert_eq!(
        titles(&first),
        titles(&scheduled),
        "previewed recipe and scheduled execution must use the same DOM/action semantics"
    );
    assert!(!http.requested.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn browser_requests_cannot_bypass_the_shared_ssrf_boundary() {
    let http = FixtureHttp::new();
    let collector =
        CdpBrowserCollector::probe(endpoint(), http.clone(), 1, Duration::from_secs(5), 1)
            .await
            .unwrap();
    let error = collector
        .collect(&source("http://169.254.169.254/latest/meta-data/"))
        .await
        .unwrap_err();
    assert!(matches!(error, FetchError::Rejected(_)));
    assert!(http
        .requested
        .lock()
        .unwrap()
        .iter()
        .any(|url| url.starts_with("http://169.254.169.254/")));
    assert!(validate_public_address(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn long_lived_collector_recovers_after_the_same_container_returns() {
    let http = FixtureHttp::new();
    let unavailable = CdpBrowserCollector::probe(
        "http://127.0.0.1:1".into(),
        http.clone(),
        1,
        Duration::from_millis(300),
        1,
    )
    .await;
    assert!(
        matches!(unavailable, Err(FetchError::Rejected(reason)) if reason == "browser_probe_failed")
    );

    let recovered =
        CdpBrowserCollector::configured(endpoint(), http, 1, Duration::from_secs(2), 1).unwrap();
    recovered.health_check().await.unwrap();
    assert_eq!(recovered.capability(), BrowserCapability::Available);

    let container = std::env::var("INOREADER_CHROMIUM_CONTAINER")
        .expect("acceptance harness must expose the Chromium container id");
    assert!(Command::new("docker")
        .args(["stop", "--timeout", "1", &container])
        .status()
        .unwrap()
        .success());
    let degraded = recovered.collect(&source("https://fixture.test/")).await;
    assert!(matches!(degraded, Err(FetchError::Rejected(reason)) if reason == "browser_degraded"));
    assert_eq!(recovered.capability(), BrowserCapability::Degraded);

    assert!(Command::new("docker")
        .args(["start", &container])
        .status()
        .unwrap()
        .success());
    let mut healthy = false;
    for _ in 0..30 {
        if recovered.health_check().await.is_ok() {
            healthy = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(
        healthy,
        "the same collector must recover after Chromium returns"
    );
    assert_eq!(recovered.capability(), BrowserCapability::Available);
    assert_eq!(
        recovered
            .collect(&source("https://fixture.test/"))
            .await
            .unwrap()
            .len(),
        3
    );
}
