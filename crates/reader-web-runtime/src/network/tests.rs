use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use http::{HeaderValue, Method};

use super::*;

fn policy() -> OutboundPolicy {
    OutboundPolicy::new(
        false,
        OutboundLimits {
            connect_timeout: Duration::from_secs(1),
            request_deadline: Duration::from_secs(2),
            max_redirect_hops: 2,
            max_response_body_bytes: 4,
        },
    )
}

#[test]
fn rejects_private_metadata_and_encoded_addresses() {
    for ip in [
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)),
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        "::ffff:10.0.0.1".parse().unwrap(),
        "fd00:ec2::254".parse().unwrap(),
    ] {
        assert!(validate_public_address(ip).is_err(), "{ip}");
    }
    assert!(validate_public_address("93.184.216.34".parse().unwrap()).is_ok());
}

#[test]
fn peer_must_equal_pinned_dns_address() {
    let url = Url::parse("https://example.test/a").unwrap();
    let public = "93.184.216.34".parse().unwrap();
    let target = policy().authorize_resolution(&url, vec![public]).unwrap();
    assert!(target.connection("1.1.1.1".parse().unwrap()).is_err());
    let connection = target.connection(public).unwrap();
    assert!(connection
        .verify_connected_peer("127.0.0.1".parse().unwrap())
        .is_err());
}

#[test]
fn cross_origin_redirect_strips_secrets_and_body() {
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, HeaderValue::from_static("secret"));
    headers.insert(header::COOKIE, HeaderValue::from_static("secret"));
    headers.insert("x-api-key", HeaderValue::from_static("secret"));
    headers.insert("accept", HeaderValue::from_static("text/html"));
    let request = PreparedRequest {
        method: Method::POST,
        url: Url::parse("https://a.test/x").unwrap(),
        headers,
        body: Some(vec![1]),
    };
    let redirected = request.for_redirect(Url::parse("https://b.test/y").unwrap());
    assert!(redirected.body.is_none());
    assert!(redirected.headers.get(header::AUTHORIZATION).is_none());
    assert!(redirected.headers.get(header::COOKIE).is_none());
    assert_eq!(redirected.headers.get("accept").unwrap(), "text/html");
}

#[test]
fn every_redirect_is_counted_and_loops_fail() {
    let start = Url::parse("https://a.test/1").unwrap();
    let mut chain = RedirectChain::new(&start, 2);
    let two = chain.follow(&start, "/2").unwrap();
    assert_eq!(chain.follow(&two, "/1"), Err(OutboundError::RedirectLoop));
}

#[test]
fn body_limit_fails_before_appending_chunk() {
    let mut body = BoundedBody::new(4);
    body.push(b"123").unwrap();
    assert_eq!(
        body.push(b"45"),
        Err(OutboundError::ResponseBodyTooLarge { limit: 4 })
    );
    assert_eq!(body.into_bytes(), b"123");
}
