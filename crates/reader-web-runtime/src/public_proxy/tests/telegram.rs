use super::*;

fn request() -> (PreparedRequest, ConnectionAuthorization) {
    let url = url::Url::parse("https://api.telegram.org/bot123:test-secret/getUpdates").unwrap();
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
    let ip = "8.8.8.8".parse().unwrap();
    let auth = policy
        .authorize_resolution(&url, vec![ip])
        .unwrap()
        .connection(ip)
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    (
        PreparedRequest {
            url,
            method: Method::POST,
            headers,
            body: Some(br#"{"offset":42,"timeout":0}"#.to_vec()),
        },
        auth,
    )
}

#[test]
fn telegram_capability_is_closed_and_https_only() {
    let (req, _) = request();
    assert!(telegram_headers(&req).is_ok());
    for url in [
        "http://api.telegram.org/bot123:test-secret/getMe",
        "https://evil.example/bot123:test-secret/getMe",
        "https://api.telegram.org/bot123:test-secret/sendMessage",
        "https://api.telegram.org:444/bot123:test-secret/getMe",
        "https://api.telegram.org/bot123:test-secret/getMe?secret=yes",
        "https://user:secret@api.telegram.org/bot123:test-secret/getMe",
    ] {
        let (mut req, _) = request();
        req.url = url::Url::parse(url).unwrap();
        assert!(telegram_headers(&req).is_err(), "{url}");
    }
    for name in [
        "authorization",
        "cookie",
        "proxy-authorization",
        "x-api-key",
        "host",
        "content-length",
    ] {
        let (mut req, _) = request();
        req.headers.insert(
            http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            header::HeaderValue::from_static("secret"),
        );
        assert!(telegram_headers(&req).is_err());
    }
    let (mut req, _) = request();
    req.method = Method::GET;
    assert!(telegram_headers(&req).is_err());
    let (mut req, _) = request();
    req.body = None;
    assert!(telegram_headers(&req).is_err());
}

async fn fixture() -> (SocketAddr, JoinHandle<()>) {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from(
            include_bytes!("../fixtures/telegram-certificate.der").to_vec(),
        )],
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            include_bytes!("../fixtures/telegram-key.der").to_vec(),
        )),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut connect = Vec::new();
        while !connect.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            socket.read_exact(&mut b).await.unwrap();
            connect.push(b[0]);
        }
        // All cleartext bytes preceding TLS are exactly this credential-free CONNECT.
        assert_eq!(
            connect,
            b"CONNECT 8.8.8.8:443 HTTP/1.1\r\nHost: 8.8.8.8:443\r\n\r\n"
        );
        socket
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .unwrap();
        let Ok(mut tls) = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
            .accept(socket)
            .await
        else {
            return;
        };
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            if tls.read_exact(&mut b).await.is_err() {
                return;
            };
            head.push(b[0]);
        }
        let text = String::from_utf8(head).unwrap();
        assert!(text.starts_with("POST /bot123:test-secret/getUpdates HTTP/1.1\r\n"));
        assert!(text.to_lowercase().contains("host: api.telegram.org\r\n"));
        let len = text
            .lines()
            .find_map(|l| {
                l.to_lowercase()
                    .strip_prefix("content-length: ")
                    .map(|v| v.parse::<usize>().unwrap())
            })
            .unwrap();
        let mut body = vec![0; len];
        tls.read_exact(&mut body).await.unwrap();
        assert_eq!(body, br#"{"offset":42,"timeout":0}"#);
        tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await
            .unwrap();
    });
    (addr, task)
}
fn transport(endpoints: Vec<SocketAddr>, trust: bool) -> ProxyTransport {
    let mut route = PublicProxyRoute::new(
        vec!["api.telegram.org".into()],
        (0..endpoints.len())
            .map(|i| SocketAddr::from(([8, 8, 8, 8], 80 + i as u16)))
            .collect(),
        1024,
        Duration::from_millis(300),
        Duration::from_secs(60),
    )
    .unwrap();
    route.endpoints = endpoints;
    let mut t = ProxyTransport::new(vec![route]).unwrap();
    if trust {
        let mut roots = RootCertStore::empty();
        roots
            .add(rustls::pki_types::CertificateDer::from(
                include_bytes!("../fixtures/telegram-certificate.der").to_vec(),
            ))
            .unwrap();
        t.tls = Arc::new(
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        );
    }
    t
}
#[tokio::test]
async fn telegram_retries_share_health_and_write_secrets_only_inside_verified_tls() {
    let stalled = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (addr, server) = fixture().await;
    let public = transport(vec![stalled.local_addr().unwrap(), addr], true);
    let bot = public.clone().for_telegram_bot_api();
    assert!(Arc::ptr_eq(&public.routes[0].health, &bot.routes[0].health));
    let (req, auth) = request();
    assert!(public
        .execute(&req, &auth, Duration::from_secs(1), Duration::from_secs(2))
        .await
        .is_err());
    let mut r = bot
        .execute(&req, &auth, Duration::from_secs(1), Duration::from_secs(2))
        .await
        .unwrap();
    while r.body.next_chunk().await.unwrap().is_some() {}
    assert_eq!(
        public.routes[0]
            .health
            .claim("api.telegram.org", &[false, false], Instant::now())
            .unwrap()
            .unwrap()
            .index,
        1
    );
    server.await.unwrap();
}
#[tokio::test]
async fn untrusted_tls_never_receives_bot_request() {
    let (addr, server) = fixture().await;
    let t = transport(vec![addr], false).for_telegram_bot_api();
    let (req, auth) = request();
    assert!(t
        .execute(&req, &auth, Duration::from_secs(1), Duration::from_secs(2))
        .await
        .is_err());
    server.await.unwrap();
}
#[tokio::test]
async fn invalid_telegram_request_rejected_before_direct_fallback() {
    let t = ProxyTransport::new(vec![]).unwrap().for_telegram_bot_api();
    let (mut req, auth) = request();
    req.url.set_scheme("http").unwrap();
    let e = t
        .execute(&req, &auth, Duration::from_secs(1), Duration::from_secs(1))
        .await
        .err()
        .unwrap();
    assert_eq!(e.kind, "proxy_telegram_https_read_required");
}

#[tokio::test]
async fn trusted_certificate_for_wrong_hostname_never_receives_token() {
    let (addr, server) = tls_proxy_fixture().await;
    let mut t = transport(vec![addr], false).for_telegram_bot_api();
    let mut roots = RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            include_bytes!("../fixtures/certificate.der").to_vec(),
        ))
        .unwrap();
    t.tls = Arc::new(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    );
    let (req, auth) = request();
    assert!(t
        .execute(&req, &auth, Duration::from_secs(1), Duration::from_secs(2))
        .await
        .is_err());
    server.await.unwrap();
}
