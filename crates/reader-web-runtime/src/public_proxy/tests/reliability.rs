use super::*;
use std::sync::Mutex;

fn trusted_transport(endpoints: Vec<SocketAddr>, timeout: Duration) -> ProxyTransport {
    // White-box local wire fixture: production constructors still reject private
    // proxies. Only socket coordinates are replaced; public target authorization,
    // original-host TLS validation, CONNECT and HTTP code are unchanged.
    let mut route = PublicProxyRoute::new(
        vec!["example.com".into()],
        endpoints
            .iter()
            .enumerate()
            .map(|(i, _)| SocketAddr::from(([8, 8, 8, 8], 80 + u16::try_from(i).unwrap())))
            .collect(),
        1024,
        timeout,
        Duration::from_secs(60),
    )
    .unwrap();
    route.endpoints = endpoints;
    let mut transport = ProxyTransport::new(vec![route]).unwrap();
    let mut roots = RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            include_bytes!("../fixtures/certificate.der").to_vec(),
        ))
        .unwrap();
    transport.tls = Arc::new(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    );
    transport
}
fn request() -> (PreparedRequest, ConnectionAuthorization) {
    let url = url::Url::parse("https://example.com/feed").unwrap();
    let policy = crate::OutboundPolicy::new(
        false,
        crate::OutboundLimits::try_from(crate::RawOutboundLimits {
            connect_timeout_ms: 1000,
            request_deadline_ms: 3000,
            max_redirect_hops: 2,
            max_response_body_bytes: 1024,
        })
        .unwrap(),
    );
    let target: std::net::IpAddr = "8.8.8.8".parse().unwrap();
    let authorization = policy
        .authorize_resolution(&url, vec![target])
        .unwrap()
        .connection(target)
        .unwrap();
    (
        PreparedRequest {
            method: Method::GET,
            url,
            headers: HeaderMap::new(),
            body: None,
        },
        authorization,
    )
}
#[derive(Clone, Default)]
struct Observer(Arc<Mutex<Vec<crate::ExternalRequestCompletion>>>);
impl ExternalRequestObserver for Observer {
    fn completed(&self, value: crate::ExternalRequestCompletion) {
        self.0.lock().unwrap().push(value);
    }
}

#[test]
fn invalid_health_intervals_and_duplicate_endpoints_fail_at_construction() {
    for (timeout, quarantine) in [
        (Duration::ZERO, Duration::from_secs(1)),
        (Duration::from_secs(1), Duration::ZERO),
    ] {
        assert_eq!(
            PublicProxyRoute::new(
                vec!["example.com".into()],
                vec!["8.8.8.8:80".parse().unwrap()],
                1024,
                timeout,
                quarantine
            )
            .unwrap_err(),
            PublicProxyConfigError::InvalidHealthIntervals
        );
    }
    assert_eq!(
        PublicProxyRoute::new(
            vec!["example.com".into()],
            vec!["8.8.8.8:80".parse().unwrap(); 2],
            1024,
            Duration::from_secs(1),
            Duration::from_secs(1)
        )
        .unwrap_err(),
        PublicProxyConfigError::InvalidEndpoints
    );
}

#[tokio::test]
async fn failed_endpoint_budget_leaves_time_for_next_and_quarantines_only_failed_pair() {
    let stalled = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let first = stalled.local_addr().unwrap();
    let stalled_server = tokio::spawn(async move {
        let (socket, _) = stalled.accept().await.unwrap();
        std::future::pending::<()>().await;
        drop(socket);
    });
    let (second, healthy_server) = tls_proxy_fixture().await;
    let observer = Observer::default();
    let transport = trusted_transport(vec![first, second], Duration::from_millis(200))
        .with_observer(observer.clone());
    let (request, auth) = request();
    let mut response = transport
        .execute(
            &request,
            &auth,
            Duration::from_secs(1),
            Duration::from_secs(3),
        )
        .await
        .unwrap();
    let mut received = vec![];
    while let Some(chunk) = response.body.next_chunk().await.unwrap() {
        received.extend(chunk);
    }
    assert_eq!(received, b"feed");
    let next = transport.routes[0]
        .health
        .claim("example.com", &[false, false], Instant::now())
        .unwrap()
        .unwrap();
    assert_eq!(next.index, 1);
    {
        let events = observer.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].system, "public_proxy");
        assert_eq!(events[0].operation, "endpoint_quarantined");
        assert_eq!(events[1].operation, "endpoint_attempt");
    }
    stalled_server.abort();
    let _ = stalled_server.await;
    healthy_server.await.unwrap();
}

#[tokio::test]
async fn origin_403_429_and_503_do_not_rotate_or_quarantine_even_when_body_is_unread() {
    for reply in [b"HTTP/1.1 403 Forbidden\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfeed".as_slice(),b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 60\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfeed".as_slice(),b"HTTP/1.1 503 Unavailable\r\nRetry-After: 60\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfeed".as_slice()] {
        let (first, server)=tls_proxy_fixture_response(reply).await;
        let spare=TcpListener::bind("127.0.0.1:0").await.unwrap();
        let transport=trusted_transport(vec![first,spare.local_addr().unwrap()],Duration::from_secs(1));
        let (request,auth)=request();
        let response=transport.execute(&request,&auth,Duration::from_secs(1),Duration::from_secs(2)).await.unwrap();
        assert!(matches!(response.status.as_u16(),403|429|503));
        if response.status.as_u16()!=403 {assert_eq!(response.headers[header::RETRY_AFTER],"60");}
        drop(response);
        assert!(tokio::time::timeout(Duration::from_millis(10),spare.accept()).await.is_err());
        assert_eq!(transport.routes[0].health.claim("example.com",&[false,false],Instant::now()).unwrap().unwrap().index,0);
        server.await.unwrap();
    }
}

#[tokio::test]
async fn incomplete_success_body_is_visible_and_never_replayed_from_next_proxy() {
    let (first, server) = tls_proxy_fixture_response(
        b"HTTP/1.1 200 OK\r\nContent-Length: 50\r\nConnection: close\r\n\r\nshort",
    )
    .await;
    let spare = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let transport = trusted_transport(
        vec![first, spare.local_addr().unwrap()],
        Duration::from_secs(1),
    );
    let (request, auth) = request();
    let mut response = transport
        .execute(
            &request,
            &auth,
            Duration::from_secs(1),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    loop {
        match response.body.next_chunk().await {
            Ok(Some(_)) => {}
            Err(error) => {
                assert_eq!(error.kind, "proxy_response_body");
                break;
            }
            Ok(None) => panic!("partial response was accepted"),
        }
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(10), spare.accept())
            .await
            .is_err()
    );
    assert_eq!(
        transport.routes[0]
            .health
            .claim("example.com", &[false, false], Instant::now())
            .unwrap()
            .unwrap()
            .index,
        1
    );
    server.await.unwrap();
}

struct DirectFixture(Option<http::StatusCode>);
#[async_trait]
impl OutboundTransport for DirectFixture {
    async fn execute(
        &self,
        _: &PreparedRequest,
        _: &ConnectionAuthorization,
        _: Duration,
        _: Duration,
    ) -> Result<TransportResponse, TransportError> {
        match self.0 {
            Some(status) => Ok(TransportResponse {
                status,
                connected_peer: "8.8.8.8".parse().unwrap(),
                headers: HeaderMap::new(),
                body: Box::new(EmptyDirectBody),
            }),
            None => Err(failure("direct_fixture_unavailable")),
        }
    }
}
struct EmptyDirectBody;
#[async_trait]
impl ResponseBody for EmptyDirectBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(None)
    }
}
#[tokio::test]
async fn direct_first_preserves_http_results_without_opening_proxy() {
    for status in [
        http::StatusCode::OK,
        http::StatusCode::FORBIDDEN,
        http::StatusCode::TOO_MANY_REQUESTS,
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut transport = trusted_transport(
            vec![listener.local_addr().unwrap()],
            Duration::from_millis(200),
        );
        transport.routes[0].direct_first = true;
        transport.direct = Arc::new(DirectFixture(Some(status)));
        let (request, auth) = request();
        let response = transport
            .execute(
                &request,
                &auth,
                Duration::from_secs(1),
                Duration::from_secs(3),
            )
            .await
            .unwrap();
        assert_eq!(response.status, status);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), listener.accept())
                .await
                .is_err()
        );
    }
}
#[tokio::test]
async fn direct_transport_failure_uses_reserved_proxy() {
    let (endpoint, server) = tls_proxy_fixture().await;
    let mut transport = trusted_transport(vec![endpoint], Duration::from_secs(2));
    transport.routes[0].direct_first = true;
    transport.direct = Arc::new(DirectFixture(None));
    let (request, auth) = request();
    let mut response = transport
        .execute(
            &request,
            &auth,
            Duration::from_secs(1),
            Duration::from_secs(3),
        )
        .await
        .unwrap();
    assert_eq!(response.status, http::StatusCode::OK);
    while response.body.next_chunk().await.unwrap().is_some() {}
    server.await.unwrap();
}

#[tokio::test]
async fn secrets_and_plain_http_are_rejected_before_direct_or_proxy_io() {
    for direct_first in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut transport = trusted_transport(
            vec![listener.local_addr().unwrap()],
            Duration::from_millis(100),
        );
        transport.routes[0].direct_first = direct_first;
        transport.direct = Arc::new(DirectFixture(Some(http::StatusCode::OK)));
        for name in [
            "authorization",
            "cookie",
            "proxy-authorization",
            "x-api-key",
            "x-auth-token",
            "x-secret",
        ] {
            let (mut request, auth) = request();
            request.headers.insert(
                http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                header::HeaderValue::from_static("fixture-only"),
            );
            assert!(
                transport
                    .execute(
                        &request,
                        &auth,
                        Duration::from_secs(1),
                        Duration::from_secs(1)
                    )
                    .await
                    .is_err(),
                "{name}"
            );
        }
        let (mut request, _) = request();
        request.url.set_scheme("http").unwrap();
        let policy = crate::OutboundPolicy::new(
            true,
            crate::OutboundLimits::try_from(crate::RawOutboundLimits {
                connect_timeout_ms: 1000,
                request_deadline_ms: 1000,
                max_redirect_hops: 2,
                max_response_body_bytes: 1024,
            })
            .unwrap(),
        );
        let ip = "8.8.8.8".parse().unwrap();
        let auth = policy
            .authorize_resolution(&request.url, vec![ip])
            .unwrap()
            .connection(ip)
            .unwrap();
        assert_eq!(
            transport
                .execute(
                    &request,
                    &auth,
                    Duration::from_secs(1),
                    Duration::from_secs(1)
                )
                .await
                .err()
                .unwrap()
                .kind,
            "proxy_public_https_get_required"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), listener.accept())
                .await
                .is_err()
        );
    }
}
