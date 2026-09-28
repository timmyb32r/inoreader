//! Explicit public-source routing. CONNECT carries the already-authorized IP;
//! the proxy never resolves the destination. TLS verifies the original URL host.

mod health;

use std::{
    collections::HashSet,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::TryStreamExt;
use http::{header, HeaderMap, Method};
use http_body_util::{BodyExt, Empty};
use hyper_util::rt::TokioIo;
use rustls::{pki_types::ServerName, ClientConfig, RootCertStore};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    task::JoinHandle,
};
use tokio_rustls::TlsConnector;

use crate::{
    validate_public_address, ConnectionAuthorization, ExternalRequestObserver,
    ExternalRequestOutcome, OutboundTransport, PreparedRequest, ReqwestPinnedTransport,
    ResponseBody, TransportError, TransportResponse,
};
use health::{HealthBody, RouteHealth};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PublicProxyConfigError {
    #[error("public proxy routes require nonempty exact lowercase DNS hosts")]
    InvalidHosts,
    #[error("a target host must belong to exactly one public proxy route")]
    DuplicateHost,
    #[error(
        "public proxy routes require a nonempty list of public IP addresses with nonzero ports"
    )]
    InvalidEndpoints,
    #[error("public proxy CONNECT response header limit must be positive")]
    InvalidHeaderLimit,
    #[error("public proxy endpoint attempt timeout and quarantine must be positive")]
    InvalidHealthIntervals,
    #[error("public proxy TLS provider has no supported protocol versions")]
    InvalidTlsProvider,
}

/// An explicit operator trust decision for public, unauthenticated HTTPS only.
/// Hosts are exact, canonical DNS names, without ports or wildcards. Endpoints
/// are public numeric IPs: neither endpoint nor destination uses proxy-side DNS.
/// The finite endpoint list is tried once per authorized destination address;
/// the enclosing outbound deadline bounds all attempts and redirects together.
#[derive(Clone, Debug)]
pub struct PublicProxyRoute {
    hosts: HashSet<String>,

    endpoints: Vec<SocketAddr>,

    max_connect_response_header_bytes: usize,

    endpoint_attempt_timeout: Duration,

    health: Arc<RouteHealth>,
    direct_first: bool,
}

impl PublicProxyRoute {
    pub fn new(
        hosts: Vec<String>,
        endpoints: Vec<SocketAddr>,
        max_connect_response_header_bytes: usize,
        endpoint_attempt_timeout: Duration,
        quarantine: Duration,
    ) -> Result<Self, PublicProxyConfigError> {
        if hosts.is_empty() || hosts.iter().any(|host| {
            let parsed = url::Url::parse(&format!("https://{host}/"));
            !matches!(parsed, Ok(ref url) if url.host_str() == Some(host.as_str()) && url.port().is_none() && url.username().is_empty() && url.password().is_none() && url.path() == "/" && url.query().is_none() && url.fragment().is_none())
                || host.parse::<std::net::IpAddr>().is_ok() || host.contains('*') || host.ends_with('.') || !host.contains('.')
        }) {
            return Err(PublicProxyConfigError::InvalidHosts);
        }
        let count = hosts.len();
        let unique: HashSet<_> = hosts.into_iter().collect();
        if unique.len() != count {
            return Err(PublicProxyConfigError::DuplicateHost);
        }
        if endpoints.is_empty()
            || endpoints.iter().collect::<HashSet<_>>().len() != endpoints.len()
            || endpoints.iter().any(|endpoint| {
                endpoint.port() == 0 || validate_public_address(endpoint.ip()).is_err()
            })
        {
            return Err(PublicProxyConfigError::InvalidEndpoints);
        }
        if max_connect_response_header_bytes == 0 {
            return Err(PublicProxyConfigError::InvalidHeaderLimit);
        }
        if endpoint_attempt_timeout.is_zero() || quarantine.is_zero() {
            return Err(PublicProxyConfigError::InvalidHealthIntervals);
        }
        let health = Arc::new(RouteHealth::new(
            unique.iter().cloned(),
            endpoints.len(),
            quarantine,
        ));
        Ok(Self {
            hosts: unique,
            endpoints,
            max_connect_response_header_bytes,
            endpoint_attempt_timeout,
            health,
            direct_first: false,
        })
    }
}

impl PublicProxyRoute {
    /// Try the shared pinned direct transport before the proxy pool. Only a
    /// transport failure falls back; HTTP errors and Retry-After stay authoritative.
    pub fn with_direct_first(mut self, enabled: bool) -> Self {
        self.direct_first = enabled;
        self
    }
}

/// Public collector transport only. Credentialed integrations must retain their
/// dedicated direct transport. Unknown/sensitive headers are rejected before
/// connecting; the route never forwards cookies, authorization or request bodies.
#[derive(Clone)]
pub struct PublicFetchTransport {
    routes: Vec<PublicProxyRoute>,
    direct: Arc<dyn OutboundTransport>,
    tls: Arc<ClientConfig>,
    observer: Option<Arc<dyn ExternalRequestObserver>>,
}

impl PublicFetchTransport {
    pub fn new(routes: Vec<PublicProxyRoute>) -> Result<Self, PublicProxyConfigError> {
        let mut hosts = HashSet::new();
        for route in &routes {
            for host in &route.hosts {
                if !hosts.insert(host.clone()) {
                    return Err(PublicProxyConfigError::DuplicateHost);
                }
            }
        }
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .map_err(|_| PublicProxyConfigError::InvalidTlsProvider)?
                .with_root_certificates(roots)
                .with_no_client_auth();
        Ok(Self {
            routes,
            direct: Arc::new(ReqwestPinnedTransport),
            tls: Arc::new(tls),
            observer: None,
        })
    }

    /// Uses the application's shared external-request instrumentation; no URL,
    /// headers, credentials or provider error strings enter health diagnostics.
    pub fn with_observer(mut self, observer: impl ExternalRequestObserver + 'static) -> Self {
        self.observer = Some(Arc::new(observer));
        self
    }
}

fn failure(kind: &'static str) -> TransportError {
    TransportError { kind }
}

fn public_headers(headers: &HeaderMap) -> Result<HeaderMap, TransportError> {
    let mut result = HeaderMap::new();
    for (name, value) in headers {
        if value.is_sensitive() {
            return Err(failure("proxy_credentials_forbidden"));
        }
        match name.as_str() {
            "accept" | "accept-language" | "cache-control" | "if-modified-since"
            | "if-none-match" | "pragma" | "user-agent" | "range" | "if-range" => {
                result.append(name, value.clone());
            }
            // Transport-owned or browser fingerprint fields carry no payload;
            // identity encoding is requested explicitly below.
            "host"
            | "connection"
            | "accept-encoding"
            | "upgrade-insecure-requests"
            | "sec-fetch-dest"
            | "sec-fetch-mode"
            | "sec-fetch-site"
            | "sec-fetch-user"
            | "sec-ch-ua"
            | "sec-ch-ua-mobile"
            | "sec-ch-ua-platform" => {}
            _ => return Err(failure("proxy_credentials_or_custom_headers_forbidden")),
        }
    }
    result.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("identity"),
    );
    Ok(result)
}

#[async_trait]
impl OutboundTransport for PublicFetchTransport {
    async fn execute(
        &self,
        request: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        connect_timeout: Duration,
        remaining_deadline: Duration,
    ) -> Result<TransportResponse, TransportError> {
        let route = self.routes.iter().find(|route| {
            request
                .url
                .host_str()
                .is_some_and(|host| route.hosts.contains(host))
        });
        let Some(route) = route else {
            return self
                .direct
                .execute(request, authorization, connect_timeout, remaining_deadline)
                .await;
        };
        if request.url != *authorization.url()
            || request.url.scheme() != "https"
            || !matches!(request.method, Method::GET | Method::HEAD)
            || request.body.is_some()
        {
            return Err(failure("proxy_public_https_get_required"));
        }
        let headers = public_headers(&request.headers)?;
        let started = Instant::now();
        if route.direct_first {
            let budget = remaining_deadline.min(route.endpoint_attempt_timeout);
            let response = tokio::time::timeout(
                budget,
                self.direct
                    .execute(request, authorization, connect_timeout.min(budget), budget),
            )
            .await;
            let succeeded = matches!(response, Ok(Ok(_)));
            health::observe(
                &self.observer,
                "direct_attempt",
                if succeeded {
                    ExternalRequestOutcome::Success
                } else {
                    ExternalRequestOutcome::Failed
                },
                started,
            );
            if let Ok(Ok(response)) = response {
                return Ok(response);
            }
        }
        let mut last_error = failure("proxy_pool_quarantined");
        let mut attempted = vec![false; route.endpoints.len()];
        let host = request
            .url
            .host_str()
            .ok_or_else(|| failure("missing_host"))?;
        while let Some(mut attempt) = route.health.claim(host, &attempted, Instant::now())? {
            attempted[attempt.index] = true;
            let endpoint = route.endpoints[attempt.index];
            let remaining = remaining_deadline
                .checked_sub(started.elapsed())
                .ok_or_else(|| failure("proxy_deadline"))?;
            let budget = remaining.min(route.endpoint_attempt_timeout);
            let attempt_started = Instant::now();
            let result = tokio::time::timeout(
                budget,
                self.via_proxy(
                    request,
                    authorization,
                    headers.clone(),
                    endpoint,
                    route.max_connect_response_header_bytes,
                    connect_timeout.min(budget),
                    budget,
                ),
            )
            .await;
            match result {
                Ok(Ok(mut response)) => {
                    // Any HTTP response is authoritative, including 429/503 and
                    // Retry-After. Only transport failures advance endpoints.
                    response.body = Box::new(HealthBody {
                        body: Some(response.body),
                        attempt: Some(attempt),
                        started: attempt_started,
                        budget,
                        observer: self.observer.clone(),
                        origin_authoritative: !response.status.is_success(),
                        finished: false,
                    });
                    return Ok(response);
                }
                Ok(Err(error)) => last_error = error,
                Err(_) if budget < route.endpoint_attempt_timeout => {
                    // Exhausting a deadline already spent on other endpoints
                    // does not prove that this endpoint failed its own budget.
                    health::observe(
                        &self.observer,
                        "endpoint_attempt",
                        ExternalRequestOutcome::Failed,
                        attempt_started,
                    );
                    return Err(failure("proxy_deadline"));
                }
                Err(_) => last_error = failure("proxy_endpoint_attempt_timeout"),
            }
            let operation = if attempt.is_probe() {
                "quarantine_probe"
            } else {
                "endpoint_quarantined"
            };
            attempt.finish(false, Instant::now())?;
            health::observe(
                &self.observer,
                operation,
                ExternalRequestOutcome::Failed,
                attempt_started,
            );
        }
        Err(last_error)
    }
}

impl PublicFetchTransport {
    #[allow(clippy::too_many_arguments)]
    async fn via_proxy(
        &self,
        request: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        mut headers: HeaderMap,
        endpoint: SocketAddr,
        header_limit: usize,
        connect_timeout: Duration,
        remaining: Duration,
    ) -> Result<TransportResponse, TransportError> {
        let started = Instant::now();
        let host = authorization
            .url()
            .host_str()
            .ok_or_else(|| failure("missing_host"))?;
        let target = SocketAddr::new(
            authorization.address(),
            authorization
                .url()
                .port_or_known_default()
                .ok_or_else(|| failure("missing_port"))?,
        );
        let tls = tokio::time::timeout(connect_timeout, async {
            let stream = TcpStream::connect(endpoint)
                .await
                .map_err(|_| failure("proxy_connect"))?;
            if stream.peer_addr().map_err(|_| failure("proxy_peer"))? != endpoint {
                return Err(failure("proxy_peer_mismatch"));
            }
            let stream = connect_tunnel(stream, target, header_limit).await?;
            let name =
                ServerName::try_from(host.to_owned()).map_err(|_| failure("proxy_tls_name"))?;
            TlsConnector::from(self.tls.clone())
                .connect(name, stream)
                .await
                .map_err(|_| failure("proxy_tls"))
        })
        .await
        .map_err(|_| failure("proxy_connect_timeout"))??;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(tls))
            .await
            .map_err(|_| failure("proxy_http_handshake"))?;
        let driver_remaining = remaining
            .checked_sub(started.elapsed())
            .ok_or_else(|| failure("proxy_endpoint_attempt_timeout"))?;
        let connection = tokio::spawn(async move {
            let _ = tokio::time::timeout(driver_remaining, connection).await;
        });
        let mut driver = ConnectionDriver(Some(connection));
        let authority = &request.url[url::Position::BeforeHost..url::Position::AfterPort];
        headers.insert(
            header::HOST,
            header::HeaderValue::from_str(authority).map_err(|_| failure("proxy_host_header"))?,
        );
        let path = &request.url[url::Position::BeforePath..url::Position::AfterQuery];
        let mut outgoing = http::Request::builder()
            .method(request.method.clone())
            .uri(path)
            .body(Empty::<Bytes>::new())
            .map_err(|_| failure("proxy_http_request"))?;
        *outgoing.headers_mut() = headers;
        let response = sender
            .send_request(outgoing)
            .await
            .map_err(|_| failure("proxy_http_response"))?;
        let (mut parts, body) = response.into_parts();
        let encoding = parts
            .headers
            .get(header::CONTENT_ENCODING)
            .map(|v| v.to_str().map_err(|_| failure("proxy_content_encoding")))
            .transpose()?
            .unwrap_or("identity")
            .to_owned();
        let stream = body.into_data_stream().map_err(std::io::Error::other);
        let body = decoded_reader(tokio_util::io::StreamReader::new(stream), &encoding)?;
        if encoding != "identity" {
            parts.headers.remove(header::CONTENT_ENCODING);
            parts.headers.remove(header::CONTENT_LENGTH);
        }
        Ok(TransportResponse {
            status: parts.status,
            headers: parts.headers,
            // CONNECT targets this literal IP and TLS authenticates the original
            // host. The proxy socket peer itself was checked separately above.
            connected_peer: authorization.address(),
            body: Box::new(ProxyBody {
                body,
                driver: ConnectionDriver(driver.0.take()),
            }),
        })
    }
}

struct ConnectionDriver(Option<JoinHandle<()>>);
impl Drop for ConnectionDriver {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}
type DecodedReader = Pin<Box<dyn AsyncRead + Send>>;
fn decoded_reader(
    reader: impl AsyncRead + Send + 'static,
    encoding: &str,
) -> Result<DecodedReader, TransportError> {
    use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
    let reader = BufReader::new(reader);
    Ok(match encoding {
        "identity" => Box::pin(reader),
        "gzip" => {
            let mut decoder = GzipDecoder::new(reader);
            decoder.multiple_members(true);
            Box::pin(decoder)
        }
        "br" => Box::pin(BrotliDecoder::new(reader)),
        "deflate" => Box::pin(ZlibDecoder::new(reader)),
        "zstd" => {
            let mut decoder = ZstdDecoder::new(reader);
            decoder.multiple_members(true);
            Box::pin(decoder)
        }
        _ => return Err(failure("proxy_unsupported_content_encoding")),
    })
}
struct ProxyBody {
    body: DecodedReader,
    driver: ConnectionDriver,
}
#[async_trait]
impl ResponseBody for ProxyBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        let mut chunk = vec![0; 8192];
        let count = self
            .body
            .read(&mut chunk)
            .await
            .map_err(|_| failure("proxy_response_body"))?;
        if count == 0 {
            if let Some(task) = self.driver.0.take() {
                task.abort();
            }
            return Ok(None);
        }
        chunk.truncate(count);
        Ok(Some(chunk))
    }
}

async fn connect_tunnel(
    mut stream: TcpStream,
    target: SocketAddr,
    header_limit: usize,
) -> Result<TcpStream, TransportError> {
    stream
        .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
        .await
        .map_err(|_| failure("proxy_connect_write"))?;
    let mut reader = BufReader::new(stream);
    let mut total = 0usize;
    let mut status = None;
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|_| failure("proxy_connect_response"))?;
        if available.is_empty() {
            return Err(failure("proxy_connect_eof"));
        }
        let take = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |i| i + 1);
        total = total
            .checked_add(take)
            .filter(|n| *n <= header_limit)
            .ok_or_else(|| failure("proxy_connect_headers_too_large"))?;
        // Accumulate at most the explicitly configured limit; read_until alone
        // could allocate an unbounded line before applying that limit.
        let mut line = Vec::new();
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        while !line.ends_with(b"\n") {
            let available = reader
                .fill_buf()
                .await
                .map_err(|_| failure("proxy_connect_response"))?;
            if available.is_empty() {
                return Err(failure("proxy_connect_eof"));
            }
            let take = available
                .iter()
                .position(|b| *b == b'\n')
                .map_or(available.len(), |i| i + 1);
            total = total
                .checked_add(take)
                .filter(|n| *n <= header_limit)
                .ok_or_else(|| failure("proxy_connect_headers_too_large"))?;
            line.extend_from_slice(&available[..take]);
            reader.consume(take);
        }
        if status.is_none() {
            let text = std::str::from_utf8(&line).map_err(|_| failure("proxy_connect_status"))?;
            let mut fields = text.split_whitespace();
            if !matches!(fields.next(), Some("HTTP/1.0" | "HTTP/1.1"))
                || fields.next() != Some("200")
            {
                return Err(failure("proxy_connect_rejected"));
            }
            status = Some(());
        } else if line == b"\r\n" {
            if !reader.buffer().is_empty() {
                return Err(failure("proxy_connect_unexpected_body"));
            }
            return Ok(reader.into_inner());
        }
    }
}

#[cfg(test)]
mod tests;
