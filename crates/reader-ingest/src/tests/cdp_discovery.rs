use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn cdp_discovery_rewrites_the_advertised_loopback_authority() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 1024];
        let count = stream.read(&mut request).await.unwrap();
        let request = std::str::from_utf8(&request[..count]).unwrap();
        assert!(request.starts_with("GET /json/version HTTP/1.1\r\n"));
        assert!(request.contains(&format!("\r\nHost: 127.0.0.1:{}\r\n", address.port())));
        let body = r#"{"webSocketDebuggerUrl":"ws://127.0.0.1:9223/devtools/browser/browser-id"}"#;
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    });

    let endpoint = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        crate::cdp_browser::discover_websocket_endpoint(&format!(
            "http://127.0.0.1:{}/",
            address.port()
        )),
    )
    .await
    .expect("discovery must not wait for the server to close the connection")
    .unwrap();

    assert_eq!(
        endpoint,
        format!(
            "ws://127.0.0.1:{}/devtools/browser/browser-id",
            address.port()
        )
    );
    server.await.unwrap();
}
