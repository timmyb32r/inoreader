use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn route(
    hosts: Vec<&str>,
    endpoints: Vec<&str>,
    limit: usize,
) -> Result<PublicProxyRoute, PublicProxyConfigError> {
    PublicProxyRoute::new(
        hosts.into_iter().map(str::to_owned).collect(),
        endpoints.into_iter().map(|v| v.parse().unwrap()).collect(),
        limit,
        Duration::from_secs(2),
        Duration::from_secs(60),
    )
}

#[test]
fn route_rejects_ambiguous_hosts_private_endpoints_and_zero_limit() {
    for host in [
        "EXAMPLE.com",
        "*.example.com",
        "example.com/path",
        "user@example.com",
        "example.com:443",
        "example.com.",
        "localhost",
        "127.0.0.1",
        "",
    ] {
        assert!(
            route(vec![host], vec!["8.8.8.8:80"], 1024).is_err(),
            "{host}"
        );
    }
    for endpoint in [
        "127.0.0.1:80",
        "10.0.0.1:80",
        "169.254.169.254:80",
        "[::1]:80",
        "8.8.8.8:0",
    ] {
        assert_eq!(
            route(vec!["example.com"], vec![endpoint], 1024).unwrap_err(),
            PublicProxyConfigError::InvalidEndpoints
        );
    }
    assert!(route(vec!["example.com"], vec![], 1024).is_err());
    assert!(route(vec![], vec!["8.8.8.8:80"], 1024).is_err());
    assert_eq!(
        route(vec!["example.com"], vec!["8.8.8.8:80"], 0).unwrap_err(),
        PublicProxyConfigError::InvalidHeaderLimit
    );
    assert!(ProxyTransport::new(vec![
        route(vec!["example.com"], vec!["8.8.8.8:80"], 1024).unwrap(),
        route(vec!["example.com"], vec!["1.1.1.1:80"], 1024).unwrap()
    ])
    .is_err());
}

#[test]
fn proxy_never_forwards_credentials_unknown_headers_or_compression_preferences() {
    for key in [
        "authorization",
        "cookie",
        "proxy-authorization",
        "x-api-key",
        "x-private",
        "referer",
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            header::HeaderValue::from_static("secret"),
        );
        assert!(public_headers(&headers).is_err(), "{key}");
    }
    let mut headers = HeaderMap::new();
    let mut value = header::HeaderValue::from_static("tag");
    value.set_sensitive(true);
    headers.insert(header::IF_NONE_MATCH, value);
    assert!(public_headers(&headers).is_err());
    let mut headers = HeaderMap::new();
    headers.insert(
        header::IF_NONE_MATCH,
        header::HeaderValue::from_static("tag"),
    );
    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("gzip"),
    );
    let result = public_headers(&headers).unwrap();
    assert_eq!(result[header::ACCEPT_ENCODING], "identity");
    assert_eq!(result[header::IF_NONE_MATCH], "tag");
}

async fn tunnel_fixture(
    response: &'static [u8],
    limit: usize,
) -> Result<TcpStream, TransportError> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).await.unwrap();
            bytes.push(byte[0]);
        }
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "CONNECT 8.8.8.8:443 HTTP/1.1\r\nHost: 8.8.8.8:443\r\n\r\n"
        );
        stream.write_all(response).await.unwrap();
    });
    let result = connect_tunnel(
        TcpStream::connect(address).await.unwrap(),
        "8.8.8.8:443".parse().unwrap(),
        limit,
    )
    .await;
    server.await.unwrap();
    result
}

#[tokio::test]
async fn connect_uses_the_pinned_ip_without_destination_hostname_or_headers() {
    assert!(tunnel_fixture(
        b"HTTP/1.1 200 Connection established\r\nProxy: fixture\r\n\r\n",
        1024
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn connect_rejects_failure_oversize_incomplete_and_unsolicited_body() {
    for response in [
        b"HTTP/1.1 403 Forbidden\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nmissing end".as_slice(),
        b"HTTP/1.1 200 OK\r\n\r\nspoofed".as_slice(),
    ] {
        assert!(tunnel_fixture(response, 1024).await.is_err());
    }
    assert_eq!(
        tunnel_fixture(b"HTTP/1.1 200 OK\r\n\r\n", 10)
            .await
            .unwrap_err()
            .kind,
        "proxy_connect_headers_too_large"
    );
}

#[tokio::test]
async fn protected_requests_are_rejected_before_any_proxy_connection() {
    let transport = ProxyTransport::new(vec![
        route(vec!["example.com"], vec!["8.8.8.8:1"], 1024).unwrap()
    ])
    .unwrap();
    let limits = crate::OutboundLimits::try_from(crate::RawOutboundLimits {
        connect_timeout_ms: 1000,
        request_deadline_ms: 1000,
        max_redirect_hops: 2,
        max_response_body_bytes: 1024,
    })
    .unwrap();
    let policy = crate::OutboundPolicy::new(false, limits);
    let url = url::Url::parse("https://example.com/feed").unwrap();
    let authorization = policy
        .authorize_resolution(&url, vec!["1.1.1.1".parse().unwrap()])
        .unwrap()
        .connection("1.1.1.1".parse().unwrap())
        .unwrap();
    for (method, body) in [(Method::POST, None), (Method::GET, Some(vec![]))] {
        let request = PreparedRequest {
            method,
            url: url.clone(),
            headers: HeaderMap::new(),
            body,
        };
        assert_eq!(
            transport
                .execute(
                    &request,
                    &authorization,
                    Duration::from_secs(1),
                    Duration::from_secs(1)
                )
                .await
                .err()
                .unwrap()
                .kind,
            "proxy_public_https_get_required"
        );
    }
}

#[tokio::test]
async fn proxy_decodes_compressed_content_before_shared_size_limits() {
    let gzip = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\xff\xb3\xc9\x28\xc9\xcd\xb1\x4b\x49\x4d\xce\x4f\x49\x4d\xb1\xd1\x07\xf3\x00\x60\xe3\xcc\x44\x14\x00\x00\x00";
    let mut reader = decoded_reader(std::io::Cursor::new(gzip), "gzip").unwrap();
    let mut output = Vec::new();
    reader.read_to_end(&mut output).await.unwrap();
    assert_eq!(output, b"<html>decoded</html>");
    assert!(decoded_reader(std::io::Cursor::new(b"x"), "unknown").is_err());
}

async fn tls_proxy_fixture() -> (SocketAddr, JoinHandle<()>) {
    tls_proxy_fixture_response(
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfeed",
    )
    .await
}

async fn tls_proxy_fixture_response(response: &'static [u8]) -> (SocketAddr, JoinHandle<()>) {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    let certificate = CertificateDer::from(include_bytes!("fixtures/certificate.der").to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        include_bytes!("fixtures/key.der").to_vec(),
    ));
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![certificate], key)
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            stream.read_exact(&mut b).await.unwrap();
            request.push(b[0]);
        }
        assert_eq!(
            String::from_utf8(request).unwrap(),
            "CONNECT 8.8.8.8:443 HTTP/1.1\r\nHost: 8.8.8.8:443\r\n\r\n"
        );
        stream
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .unwrap();
        let Ok(mut tls) = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
            .accept(stream)
            .await
        else {
            return;
        };
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            if tls.read_exact(&mut b).await.is_err() {
                return;
            }
            request.push(b[0]);
        }
        let request = String::from_utf8(request).unwrap().to_lowercase();
        assert!(request.contains("host: example.com\r\n"));
        assert!(request.starts_with("get /feed http/1.1\r\n"));
        tls.write_all(response).await.unwrap();
    });
    (address, task)
}

#[tokio::test]
async fn tunnel_validates_tls_and_retains_original_hostname_after_ip_pinning() {
    let (endpoint, server) = tls_proxy_fixture().await;
    let mut transport = ProxyTransport::new(vec![]).unwrap();
    let mut roots = RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(
            include_bytes!("fixtures/certificate.der").to_vec(),
        ))
        .unwrap();
    transport.tls = Arc::new(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    );
    let limits = crate::OutboundLimits::try_from(crate::RawOutboundLimits {
        connect_timeout_ms: 2000,
        request_deadline_ms: 4000,
        max_redirect_hops: 3,
        max_response_body_bytes: 1024,
    })
    .unwrap();
    let policy = crate::OutboundPolicy::new(false, limits);
    let url = url::Url::parse("https://example.com/feed").unwrap();
    let authorization = policy
        .authorize_resolution(&url, vec!["8.8.8.8".parse().unwrap()])
        .unwrap()
        .connection("8.8.8.8".parse().unwrap())
        .unwrap();
    let request = PreparedRequest {
        method: Method::GET,
        url,
        headers: HeaderMap::new(),
        body: None,
    };
    let mut response = transport
        .via_proxy(
            &request,
            &authorization,
            public_headers(&request.headers).unwrap(),
            endpoint,
            1024,
            Duration::from_secs(2),
            Duration::from_secs(4),
        )
        .await
        .unwrap();
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next_chunk().await.unwrap() {
        body.extend(chunk);
    }
    assert_eq!(body, b"feed");
    server.await.unwrap();
}

#[tokio::test]
async fn tunnel_rejects_untrusted_certificate_instead_of_weakening_tls() {
    let (endpoint, server) = tls_proxy_fixture().await;
    let transport = ProxyTransport::new(vec![]).unwrap();
    let limits = crate::OutboundLimits::try_from(crate::RawOutboundLimits {
        connect_timeout_ms: 2000,
        request_deadline_ms: 4000,
        max_redirect_hops: 3,
        max_response_body_bytes: 1024,
    })
    .unwrap();
    let policy = crate::OutboundPolicy::new(false, limits);
    let url = url::Url::parse("https://example.com/feed").unwrap();
    let authorization = policy
        .authorize_resolution(&url, vec!["8.8.8.8".parse().unwrap()])
        .unwrap()
        .connection("8.8.8.8".parse().unwrap())
        .unwrap();
    let request = PreparedRequest {
        method: Method::GET,
        url,
        headers: HeaderMap::new(),
        body: None,
    };
    let result = transport
        .via_proxy(
            &request,
            &authorization,
            public_headers(&request.headers).unwrap(),
            endpoint,
            1024,
            Duration::from_secs(2),
            Duration::from_secs(4),
        )
        .await;
    assert_eq!(result.err().unwrap().kind, "proxy_tls");
    server.await.unwrap();
}

#[test]
fn duplicate_hosts_inside_one_route_are_rejected() {
    assert_eq!(
        route(vec!["example.com", "example.com"], vec!["8.8.8.8:80"], 1024).unwrap_err(),
        PublicProxyConfigError::DuplicateHost
    );
}
#[tokio::test]
async fn gzip_members_and_http_zlib_deflate_preserve_the_full_body() {
    let gzip = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\xff\xb3\xc9\x28\xc9\xcd\xb1\x4b\x49\x4d\xce\x4f\x49\x4d\xb1\xd1\x07\xf3\x00\x60\xe3\xcc\x44\x14\x00\x00\x00";
    let mut reader = decoded_reader(
        std::io::Cursor::new([gzip.as_slice(), gzip.as_slice()].concat()),
        "gzip",
    )
    .unwrap();
    let mut output = Vec::new();
    reader.read_to_end(&mut output).await.unwrap();
    assert_eq!(output, b"<html>decoded</html><html>decoded</html>");
    let zlib = b"\x78\x9c\xb3\xc9\x28\xc9\xcd\xb1\x4b\x49\x4d\xce\x4f\x49\x4d\xb1\xd1\x07\xf3\x00\x4d\xcd\x07\x56";

    let mut reader = decoded_reader(std::io::Cursor::new(zlib), "deflate").unwrap();
    let mut output = Vec::new();
    reader.read_to_end(&mut output).await.unwrap();
    assert_eq!(output, b"<html>decoded</html>");
}

mod health;

mod reliability;

mod telegram;
