//! Chromium DevTools Protocol adapter for browser-backed Web feeds.

use crate::{
    extract_selected_records, BrowserCapability, BrowserCollector, FetchError, SelectorLanguage,
    SourceDefinition, SourceKind, SourceRecord, WebLoading,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::cdp::browser_protocol::{
    browser::{SetDownloadBehaviorBehavior, SetDownloadBehaviorParams},
    emulation::SetDeviceMetricsOverrideParams,
    fetch::{
        EnableParams, EventRequestPaused, FailRequestParams, FulfillRequestParams, HeaderEntry,
    },
    network::{ErrorReason, SetBypassServiceWorkerParams, SetCacheDisabledParams},
    target::{CreateBrowserContextParams, CreateTargetParams},
};
use chromiumoxide::{page::ScreenshotParams, Browser, Page};
use futures_util::StreamExt;
use http::{HeaderMap, Method};
use reader_web_runtime::{
    DnsResolver, ExternalRequestObserver, OutboundHttpClient, OutboundTransport, PreparedRequest,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::Semaphore, task::JoinHandle};
use url::Url;

#[derive(Debug)]
pub struct BrowserHttpResponse {
    pub final_url: Url,
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

/// Object-safe access to the shared outbound HTTP security boundary.
#[async_trait]
pub trait BrowserHttpClient: Send + Sync {
    async fn execute(&self, request: PreparedRequest) -> Result<BrowserHttpResponse, FetchError>;
}

#[async_trait]
impl<R, T, O> BrowserHttpClient for OutboundHttpClient<R, T, O>
where
    R: DnsResolver + Send + Sync,
    T: OutboundTransport + Send + Sync,
    O: ExternalRequestObserver + Send + Sync,
{
    async fn execute(&self, request: PreparedRequest) -> Result<BrowserHttpResponse, FetchError> {
        let response = OutboundHttpClient::execute(self, request)
            .await
            .map_err(|e| FetchError::Rejected(e.to_string()))?;
        Ok(BrowserHttpResponse {
            final_url: response.final_url,
            status: response.status.as_u16(),
            headers: response.headers,
            body: response.body,
        })
    }
}

/// Connects to remote Chromium per job. Each job owns a fresh incognito
/// context which is disposed on both success and failure.
pub struct CdpBrowserCollector {
    endpoint: String,
    http: Arc<dyn BrowserHttpClient>,
    contexts: Arc<Semaphore>,
    navigation_timeout: Duration,
    max_pages: usize,
    available: AtomicBool,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VisualRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VisualCandidateGroup {
    pub id: String,
    pub selector: String,
    pub boxes: Vec<VisualRect>,
}
#[derive(Clone, Debug)]
pub struct BrowserVisualSnapshot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub groups: Vec<VisualCandidateGroup>,
}
impl CdpBrowserCollector {
    pub fn configured(
        endpoint: String,
        http: Arc<dyn BrowserHttpClient>,
        max_contexts: usize,
        navigation_timeout: Duration,
        max_pages: usize,
    ) -> Result<Self, FetchError> {
        if max_contexts == 0 || navigation_timeout.is_zero() || max_pages == 0 {
            return Err(FetchError::Rejected("invalid_browser_limits".into()));
        }
        Ok(Self {
            endpoint,
            http,
            contexts: Arc::new(Semaphore::new(max_contexts)),
            navigation_timeout,
            max_pages,
            available: AtomicBool::new(false),
        })
    }
    pub async fn probe(
        endpoint: String,
        http: Arc<dyn BrowserHttpClient>,
        max_contexts: usize,
        navigation_timeout: Duration,
        max_pages: usize,
    ) -> Result<Self, FetchError> {
        let value = Self::configured(endpoint, http, max_contexts, navigation_timeout, max_pages)?;
        value.health_check().await?;
        Ok(value)
    }
    pub async fn health_check(&self) -> Result<(), FetchError> {
        let connection = tokio::time::timeout(
            self.navigation_timeout,
            Browser::connect(self.endpoint.clone()),
        )
        .await;
        let (browser, mut handler) = match connection {
            Ok(Ok(value)) => value,
            failure => {
                eprintln!(
                    "Chromium health connection failed for {}: {failure:?}",
                    self.endpoint
                );
                self.available.store(false, Ordering::Relaxed);
                return Err(FetchError::Rejected("browser_probe_failed".into()));
            }
        };
        let driver = tokio::spawn(async move { while handler.next().await.is_some() {} });
        let probe = tokio::time::timeout(self.navigation_timeout, browser.version()).await;
        drop(browser);
        driver.abort();
        match probe {
            Ok(Ok(_)) => {
                self.available.store(true, Ordering::Relaxed);
                Ok(())
            }
            _ => {
                self.available.store(false, Ordering::Relaxed);
                Err(FetchError::Rejected("browser_probe_failed".into()))
            }
        }
    }
    async fn connect(&self) -> Result<(Browser, chromiumoxide::Handler), FetchError> {
        match tokio::time::timeout(
            self.navigation_timeout,
            Browser::connect(self.endpoint.clone()),
        )
        .await
        {
            Ok(Ok(value)) => {
                self.available.store(true, Ordering::Relaxed);
                Ok(value)
            }
            _ => {
                self.available.store(false, Ordering::Relaxed);
                Err(FetchError::Rejected("browser_degraded".into()))
            }
        }
    }
    pub async fn visual_snapshot(
        &self,
        url: &Url,
        mobile: bool,
    ) -> Result<BrowserVisualSnapshot, FetchError> {
        let permit = self
            .contexts
            .acquire()
            .await
            .map_err(|_| FetchError::Rejected("browser_shutdown".into()))?;
        let (browser, mut handler) = self.connect().await?;
        let driver = tokio::spawn(async move { while handler.next().await.is_some() {} });
        let context = browser
            .create_browser_context(
                CreateBrowserContextParams::builder()
                    .dispose_on_detach(true)
                    .proxy_server("http://127.0.0.1:9")
                    .proxy_bypass_list("<-loopback>")
                    .build(),
            )
            .await
            .map_err(|_| FetchError::Rejected("browser_context_create_failed".into()))?;
        let result=async{
   browser.execute(SetDownloadBehaviorParams::builder().behavior(SetDownloadBehaviorBehavior::Deny).browser_context_id(context.clone()).events_enabled(false).build().map_err(FetchError::Rejected)?).await.map_err(|_|FetchError::Rejected("browser_download_policy_failed".into()))?;
   let page=Arc::new(browser.new_page(CreateTargetParams::builder().url("about:blank").browser_context_id(context.clone()).background(true).build().map_err(FetchError::Rejected)?).await.map_err(|_|FetchError::Rejected("browser_page_create_failed".into()))?);self.configure_page(&page).await?;let interceptor=intercept(page.clone(),self.http.clone()).await?;
   let(width,height)=if mobile{(390,844)}else{(1440,900)};page.execute(SetDeviceMetricsOverrideParams::new(width,height,1.0,mobile)).await.map_err(|_|FetchError::Rejected("browser_viewport_failed".into()))?;
   match tokio::time::timeout(self.navigation_timeout,page.goto(url.as_str())).await{Ok(Ok(_))=>{},Ok(Err(_))=>return Err(FetchError::Rejected("browser_navigation_failed".into())),Err(_)=>return Err(FetchError::Rejected("browser_navigation_timeout".into()))}
   let script=r#"(()=>{const groups=new Map();const esc=s=>CSS.escape(s);for(const n of document.querySelectorAll('body *')){const r=n.getBoundingClientRect();if(r.width<=1||r.height<=1||r.bottom<0||r.right<0||r.top>innerHeight||r.left>innerWidth)continue;const classes=[...n.classList].sort();const selector=n.localName+(classes.length?'.'+classes.map(esc).join('.'):'');if(!groups.has(selector))groups.set(selector,[]);groups.get(selector).push({x:r.x,y:r.y,width:r.width,height:r.height})}return [...groups].filter(([,boxes])=>boxes.length>1).map(([selector,boxes])=>({id:selector,selector,boxes}))})()"#;
   let groups:Vec<VisualCandidateGroup>=page.evaluate(script).await.map_err(|_|FetchError::Rejected("browser_geometry_capture_failed".into()))?.into_value().map_err(|_|FetchError::Rejected("browser_geometry_capture_failed".into()))?;
   let png=page.screenshot(ScreenshotParams::builder().format(CaptureScreenshotFormat::Png).build()).await.map_err(|_|FetchError::Rejected("browser_screenshot_failed".into()))?;interceptor.abort();Ok(BrowserVisualSnapshot{png,width:width as u32,height:height as u32,groups})
  }.await;
        let disposed = browser.dispose_browser_context(context).await;
        drop(browser);
        driver.abort();
        drop(permit);
        if disposed.is_err() && result.is_ok() {
            return Err(FetchError::Rejected(
                "browser_context_cleanup_failed".into(),
            ));
        }
        result
    }
    async fn collect_browser(
        &self,
        source: &SourceDefinition,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        let permit = self
            .contexts
            .acquire()
            .await
            .map_err(|_| FetchError::Rejected("browser_shutdown".into()))?;
        let (browser, mut handler) = self.connect().await?;
        let driver = tokio::spawn(async move { while handler.next().await.is_some() {} });
        let result = match tokio::time::timeout(
            self.navigation_timeout,
            self.collect_in_context(&browser, source),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(FetchError::Rejected("browser_navigation_timeout".into())),
        };
        drop(browser);
        driver.abort();
        drop(permit);
        result
    }
    async fn collect_in_context(
        &self,
        browser: &Browser,
        source: &SourceDefinition,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        // The black-hole proxy is a second, process-level egress barrier. Fetch
        // fulfillment never reaches it, while an uninstrumented popup/worker cannot
        // reach the public network if Chromium creates a new target unexpectedly.
        let context = browser
            .create_browser_context(
                CreateBrowserContextParams::builder()
                    .dispose_on_detach(true)
                    .proxy_server("http://127.0.0.1:9")
                    .proxy_bypass_list("<-loopback>")
                    .build(),
            )
            .await
            .map_err(|_| FetchError::Rejected("browser_context_create_failed".into()))?;
        let result = async {
            browser
                .execute(
                    SetDownloadBehaviorParams::builder()
                        .behavior(SetDownloadBehaviorBehavior::Deny)
                        .browser_context_id(context.clone())
                        .events_enabled(false)
                        .build()
                        .map_err(FetchError::Rejected)?,
                )
                .await
                .map_err(|_| FetchError::Rejected("browser_download_policy_failed".into()))?;
            let page = Arc::new(
                browser
                    .new_page(
                        CreateTargetParams::builder()
                            .url("about:blank")
                            .browser_context_id(context.clone())
                            .background(true)
                            .build()
                            .map_err(FetchError::Rejected)?,
                    )
                    .await
                    .map_err(|_| FetchError::Rejected("browser_page_create_failed".into()))?,
            );
            self.configure_page(&page).await?;
            let interceptor = intercept(page.clone(), self.http.clone()).await?;
            if let SourceKind::WebPage(recipe) = source.kind() {
                let (width, height, mobile) = match recipe.actions().viewport() {
                    crate::WebViewport::Desktop => (1440, 900, false),
                    crate::WebViewport::Mobile => (390, 844, true),
                };
                page.execute(SetDeviceMetricsOverrideParams::new(
                    width, height, 1.0, mobile,
                ))
                .await
                .map_err(|_| FetchError::Rejected("browser_viewport_failed".into()))?;
            }
            match tokio::time::timeout(self.navigation_timeout, page.goto(source.url().as_str()))
                .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(_)) => return Err(FetchError::Rejected("browser_navigation_failed".into())),
                Err(_) => return Err(FetchError::Rejected("browser_navigation_timeout".into())),
            }
            if let SourceKind::WebPage(recipe) = source.kind() {
                wait_for_legacy_selector(&page, recipe).await?;
                run_actions(&page, recipe).await?;
            }
            let page_limit = match source.kind() {
                SourceKind::WebPage(recipe) => self.max_pages.min(recipe.max_pages()),
                _ => 1,
            };
            let mut records = Vec::new();
            for index in 0..page_limit {
                records.extend(capture_records(&page, source).await?);
                let SourceKind::WebPage(recipe) = source.kind() else {
                    break;
                };
                let Some(next) = recipe.actions().next_page() else {
                    break;
                };
                if index + 1 >= page_limit {
                    break;
                }
                if !click_selector(&page, next).await? {
                    break;
                }
                page.evaluate("new Promise(requestAnimationFrame)")
                    .await
                    .map_err(|_| FetchError::Rejected("browser_next_page_failed".into()))?;
            }
            interceptor.abort();
            Ok(records)
        }
        .await;
        let disposed = browser.dispose_browser_context(context).await;
        if disposed.is_err() && result.is_ok() {
            return Err(FetchError::Rejected(
                "browser_context_cleanup_failed".into(),
            ));
        }
        result
    }
    async fn configure_page(&self, page: &Page) -> Result<(), FetchError> {
        page.execute(SetCacheDisabledParams::new(true))
            .await
            .map_err(|_| FetchError::Rejected("browser_cache_policy_failed".into()))?;
        page.execute(SetBypassServiceWorkerParams::new(true))
            .await
            .map_err(|_| FetchError::Rejected("browser_service_worker_policy_failed".into()))?;
        page.evaluate_on_new_document("window.open=()=>null;addEventListener('click',e=>{const a=e.target.closest&&e.target.closest('a[target=_blank]');if(a)e.preventDefault()},true);").await.map_err(|_|FetchError::Rejected("browser_popup_policy_failed".into()))?;
        page.execute(EnableParams::builder().handle_auth_requests(false).build())
            .await
            .map_err(|_| FetchError::Rejected("browser_interception_enable_failed".into()))?;
        Ok(())
    }
}

#[async_trait]
impl BrowserCollector for CdpBrowserCollector {
    fn capability(&self) -> BrowserCapability {
        if self.available.load(Ordering::Relaxed) {
            BrowserCapability::Available
        } else {
            BrowserCapability::Degraded
        }
    }
    async fn collect(&self, source: &SourceDefinition) -> Result<Vec<SourceRecord>, FetchError> {
        let SourceKind::WebPage(recipe) = source.kind() else {
            return Err(FetchError::Rejected("source_is_not_web_feed".into()));
        };
        if recipe.loading() == WebLoading::Static {
            return Err(FetchError::Rejected(
                "source_does_not_require_browser".into(),
            ));
        }
        let primary = match recipe.extraction().listing_url() {
            Some(url) => source
                .with_url(url.clone())
                .map_err(|error| FetchError::Rejected(error.to_string()))?,
            None => source.clone(),
        };
        let mut records = self.collect_browser(&primary).await?;
        for page in recipe.actions().start_pages() {
            let page_source = source
                .with_url(page.clone())
                .map_err(|error| FetchError::Rejected(error.to_string()))?;
            records.extend(self.collect_browser(&page_source).await?)
        }
        let mut seen = std::collections::HashSet::new();
        records.retain(|record| seen.insert(record.upstream_id().to_owned()));
        Ok(records)
    }
}

async fn intercept(
    page: Arc<Page>,
    http: Arc<dyn BrowserHttpClient>,
) -> Result<JoinHandle<()>, FetchError> {
    let mut events = page
        .event_listener::<EventRequestPaused>()
        .await
        .map_err(|_| FetchError::Rejected("browser_interception_listener_failed".into()))?;
    Ok(tokio::spawn(async move {
        while let Some(event) = events.next().await {
            let id = event.request_id.clone();
            match fulfill_paused(&event, http.as_ref()).await {
                Ok(response) => {
                    let _ = page.execute(response).await;
                }
                Err(_) => {
                    let _ = page
                        .execute(FailRequestParams::new(id, ErrorReason::BlockedByClient))
                        .await;
                }
            }
        }
    }))
}

async fn run_actions(page: &Page, recipe: &crate::WebFeedRecipe) -> Result<(), FetchError> {
    fn selector(value: &crate::WebSelector) -> serde_json::Value {
        serde_json::json!({"kind":match value.language(){SelectorLanguage::Css=>"css",SelectorLanguage::XPath=>"xpath"},"value":value.expression()})
    }
    let overlays = recipe
        .actions()
        .hide_overlays()
        .iter()
        .map(selector)
        .collect::<Vec<_>>();
    let load_more = recipe.actions().load_more().map(selector);
    let payload=serde_json::to_string(&serde_json::json!({"overlays":overlays,"loadMore":load_more,"clicks":recipe.actions().load_more_clicks(),"scrolls":recipe.actions().scrolls()})).map_err(|_|FetchError::Rejected("invalid_browser_actions".into()))?;
    let script = format!(
        r#"(async()=>{{const p={payload};const one=s=>s.kind==='css'?document.querySelector(s.value):document.evaluate(s.value,document,null,XPathResult.FIRST_ORDERED_NODE_TYPE,null).singleNodeValue;for(const s of p.overlays)one(s)?.remove();for(let i=0;i<p.clicks;i++){{one(p.loadMore)?.click();await new Promise(requestAnimationFrame)}}for(let i=0;i<p.scrolls;i++){{scrollTo(0,document.body.scrollHeight);await new Promise(requestAnimationFrame)}}}})()"#
    );
    page.evaluate(script)
        .await
        .map_err(|_| FetchError::Rejected("browser_actions_failed".into()))?;
    Ok(())
}
async fn wait_for_legacy_selector(
    page: &Page,
    recipe: &crate::WebFeedRecipe,
) -> Result<(), FetchError> {
    let Some(selector) = recipe.extraction().wait_selector() else {
        return Ok(());
    };
    let expression = serde_json::to_string(selector.expression())
        .map_err(|_| FetchError::Rejected("invalid_wait_selector".into()))?;
    let script = format!(
        r#"new Promise((resolve,reject)=>{{if(document.querySelector({expression}))return resolve(true);const observer=new MutationObserver(()=>{{if(document.querySelector({expression})){{observer.disconnect();resolve(true)}}}});observer.observe(document,{{subtree:true,childList:true}});setTimeout(()=>{{observer.disconnect();reject(new Error('wait selector timeout'))}},10000)}})"#
    );
    page.evaluate(script)
        .await
        .map_err(|_| FetchError::Rejected("browser_wait_selector_failed".into()))?;
    Ok(())
}
async fn capture_records(
    page: &Page,
    source: &SourceDefinition,
) -> Result<Vec<SourceRecord>, FetchError> {
    let html = page
        .content()
        .await
        .map_err(|_| FetchError::Rejected("browser_dom_capture_failed".into()))?;
    let base: String = page
        .evaluate("document.baseURI")
        .await
        .map_err(|_| FetchError::Rejected("browser_url_capture_failed".into()))?
        .into_value()
        .map_err(|_| FetchError::Rejected("browser_url_capture_failed".into()))?;
    let final_url =
        Url::parse(&base).map_err(|_| FetchError::Rejected("browser_url_capture_failed".into()))?;
    if matches!(source.kind(),SourceKind::WebPage(recipe)if recipe.selector_kind()==SelectorLanguage::XPath)
    {
        extract_xpath(page, source, &final_url).await
    } else {
        extract_selected_records(source, &final_url, &html)
    }
}
async fn click_selector(page: &Page, selector: &crate::WebSelector) -> Result<bool, FetchError> {
    let payload=serde_json::to_string(&serde_json::json!({"kind":match selector.language(){SelectorLanguage::Css=>"css",SelectorLanguage::XPath=>"xpath"},"value":selector.expression()})).map_err(|_|FetchError::Rejected("invalid_next_selector".into()))?;
    let script = format!(
        r#"(()=>{{const s={payload};const n=s.kind==='css'?document.querySelector(s.value):document.evaluate(s.value,document,null,XPathResult.FIRST_ORDERED_NODE_TYPE,null).singleNodeValue;if(!n)return false;n.click();return true}})()"#
    );
    page.evaluate(script)
        .await
        .map_err(|_| FetchError::Rejected("browser_next_page_failed".into()))?
        .into_value()
        .map_err(|_| FetchError::Rejected("browser_next_page_failed".into()))
}
#[derive(serde::Deserialize)]
struct SelectedNode {
    title: String,
    href: Option<String>,
    html: String,
}
async fn extract_xpath(
    page: &Page,
    source: &SourceDefinition,
    base: &Url,
) -> Result<Vec<SourceRecord>, FetchError> {
    let SourceKind::WebPage(recipe) = source.kind() else {
        return Err(FetchError::Rejected("source_is_not_web_feed".into()));
    };
    let expression = serde_json::to_string(recipe.selector())
        .map_err(|_| FetchError::Rejected("invalid_xpath_selector".into()))?;
    let script = format!(
        r#"(()=>{{const r=document.evaluate({expression},document,null,XPathResult.ORDERED_NODE_SNAPSHOT_TYPE,null);const out=[];for(let i=0;i<r.snapshotLength;i++){{const n=r.snapshotItem(i);const a=n.matches?.('a[href]')?n:n.querySelector?.('a[href]');out.push({{title:(n.textContent||'').trim(),href:a?.getAttribute('href')||null,html:n.outerHTML||''}})}}return out}})()"#
    );
    let selected: Vec<SelectedNode> = page
        .evaluate(script)
        .await
        .map_err(|_| FetchError::Rejected("xpath_evaluation_failed".into()))?
        .into_value()
        .map_err(|_| FetchError::Rejected("xpath_evaluation_failed".into()))?;
    let mut records = Vec::new();
    for (index, node) in selected.into_iter().enumerate() {
        let absolute = node
            .href
            .as_deref()
            .map(|value| base.join(value))
            .transpose()
            .map_err(|_| FetchError::Rejected("invalid_selected_url".into()))?;
        let upstream = node
            .href
            .clone()
            .unwrap_or_else(|| format!("xpath:{}:{index}", recipe.selector()));
        let parsed = reader_collectors::ParsedRecord {
            upstream_id: upstream,
            original_url: node.href.unwrap_or_default(),
            absolute_url: absolute,
            title: node.title,
            description: None,
            content_html: Some(node.html),
            published_at: None,
        };
        records.push(
            SourceRecord::from_parsed(reader_core::SourceRecordId::new(), source.id(), parsed)
                .map_err(|error| FetchError::Rejected(error.to_string()))?,
        )
    }
    Ok(records)
}

async fn fulfill_paused(
    event: &EventRequestPaused,
    http: &dyn BrowserHttpClient,
) -> Result<FulfillRequestParams, FetchError> {
    let url = Url::parse(&event.request.url)
        .map_err(|_| FetchError::Rejected("invalid_browser_request_url".into()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(FetchError::Rejected(
            "browser_non_http_request_blocked".into(),
        ));
    }
    let method = Method::from_bytes(event.request.method.as_bytes())
        .map_err(|_| FetchError::Rejected("invalid_browser_request_method".into()))?;
    let headers = safe_request_headers(event.request.headers.inner())?;
    if event.request.has_post_data == Some(true) && event.request.post_data_entries.is_none() {
        return Err(FetchError::Rejected(
            "browser_request_body_unavailable".into(),
        ));
    }
    let body = event.request.post_data_entries.as_ref().map(|entries| {
        let mut body = Vec::new();
        for entry in entries {
            if let Some(bytes) = &entry.bytes {
                body.extend_from_slice(<chromiumoxide::Binary as AsRef<[u8]>>::as_ref(bytes));
            }
        }
        body
    });
    let mut response = http
        .execute(PreparedRequest {
            method,
            url: url.clone(),
            headers,
            body,
        })
        .await?;
    if response.final_url != url
        && response
            .headers
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.to_ascii_lowercase().contains("html"))
    {
        if let Ok(html) = std::str::from_utf8(&response.body) {
            let escaped = response
                .final_url
                .as_str()
                .replace('&', "&amp;")
                .replace('\"', "&quot;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            response.body = format!("<base href=\"{escaped}\">{html}").into_bytes();
        }
    }
    let response_headers = response
        .headers
        .iter()
        .filter_map(|(name, value)| {
            if name == http::header::SET_COOKIE || name.as_str().eq_ignore_ascii_case("set-cookie2")
            {
                return None;
            }
            value
                .to_str()
                .ok()
                .map(|value| HeaderEntry::new(name.as_str(), value))
        })
        .collect::<Vec<_>>();
    FulfillRequestParams::builder()
        .request_id(event.request_id.clone())
        .response_code(response.status as i64)
        .response_headers(response_headers)
        .body(BASE64.encode(response.body))
        .build()
        .map_err(FetchError::Rejected)
}

fn safe_request_headers(value: &serde_json::Value) -> Result<HeaderMap, FetchError> {
    let mut result = HeaderMap::new();
    let Some(headers) = value.as_object() else {
        return Err(FetchError::Rejected(
            "invalid_browser_request_headers".into(),
        ));
    };
    for (name, value) in headers {
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "host" | "cookie" | "cookie2" | "authorization" | "proxy-authorization"
        ) {
            continue;
        }
        let Some(value) = value.as_str() else {
            continue;
        };
        let name = http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| FetchError::Rejected("invalid_browser_request_header".into()))?;
        let value = http::HeaderValue::from_str(value)
            .map_err(|_| FetchError::Rejected("invalid_browser_request_header".into()))?;
        result.append(name, value);
    }
    Ok(result)
}
