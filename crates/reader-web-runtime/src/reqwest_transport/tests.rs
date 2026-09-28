use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use super::pinned_client;

#[tokio::test]
async fn pinned_client_decompresses_gzip_responses_before_ingestion() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let count = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..count]);
        assert!(request.to_ascii_lowercase().contains("accept-encoding:"));

        // gzip("<html>decoded</html>")
        let body = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\xff\xb3\xc9\x28\xc9\xcd\xb1\x4b\x49\x4d\xce\x4f\x49\x4d\xb1\xd1\x07\xf3\x00\x60\xe3\xcc\x44\x14\x00\x00\x00";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    });

    let client = pinned_client(
        "example.test",
        address,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap();
    let response = client
        .get(format!("http://example.test:{}", address.port()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.bytes().await.unwrap(), "<html>decoded</html>");
    server.join().unwrap();
}

#[tokio::test]
async fn conditional_connection_closed_before_headers_has_narrow_typed_classification() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let count = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
        assert!(request.contains("if-none-match:"));
        // Reproduce DATAREON: close after a conditional GET, before any headers.
    });
    let client = pinned_client(
        "example.test",
        address,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap();
    let error = client
        .get(format!("http://example.test:{}", address.port()))
        .header("If-None-Match", "\"known\"")
        .send()
        .await
        .unwrap_err();
    assert_eq!(
        super::classify_error(&error).kind,
        "response_incomplete_before_headers"
    );
    server.join().unwrap();
}

use std::{net::SocketAddr, process::Command};

/// The child gets proxy environment overrides without changing the test runner's
/// process-wide environment. All destinations and canary credentials are fixtures.
#[test]
fn direct_client_ignores_ambient_proxy_with_credential_canary() {
    const CHILD: &str = "READER_PROXY_ISOLATION_FIXTURE";
    if let Ok(port) = std::env::var(CHILD) {
        let port: u16 = port.parse().unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let client = pinned_client(
                    "fixture.example",
                    SocketAddr::from(([127, 0, 0, 1], port)),
                    Duration::from_secs(1),
                    Duration::from_secs(2),
                )
                .unwrap();
                let response = client
                    .get(format!("http://fixture.example:{port}/"))
                    .header("authorization", "Bearer fixture-only-canary")
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), http::StatusCode::OK);
            });
        return;
    }
    let destination = TcpListener::bind("127.0.0.1:0").unwrap();
    let target_port = destination.local_addr().unwrap().port();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    destination.set_nonblocking(true).unwrap();
    proxy.set_nonblocking(true).unwrap();
    let service = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok((mut socket, _)) = destination.accept() {
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = [0; 4096];
                let n = socket.read(&mut bytes).unwrap();
                let request = String::from_utf8_lossy(&bytes[..n]);
                assert!(request.contains("fixture-only-canary"));
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    });
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "reqwest_transport::tests::direct_client_ignores_ambient_proxy_with_credential_canary",
            "--nocapture",
        ])
        .env(CHILD, target_port.to_string())
        .env("NO_PROXY", "")
        .env("no_proxy", "");
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        child.env(name, &proxy_url);
    }
    let result = child.output().unwrap();
    assert!(
        proxy.accept().is_err(),
        "direct request connected to the ambient proxy"
    );
    assert!(
        service.join().unwrap(),
        "direct destination never received the canary"
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
