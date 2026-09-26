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
