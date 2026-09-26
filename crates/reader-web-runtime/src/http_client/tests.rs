use std::{
    collections::VecDeque,
    net::Ipv4Addr,
    sync::{Arc, Mutex},
    time::Duration,
};

use http::{HeaderValue, Method};

use super::*;
use crate::OutboundLimits;

struct Resolver {
    answers: Mutex<VecDeque<Vec<IpAddr>>>,
}
#[async_trait]
impl DnsResolver for Resolver {
    async fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, ResolveError> {
        Ok(self.answers.lock().unwrap().pop_front().unwrap())
    }
}

struct Body(VecDeque<Vec<u8>>);
#[async_trait]
impl ResponseBody for Body {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(self.0.pop_front())
    }
}

struct Transport {
    responses: Mutex<VecDeque<TransportResponse>>,
    requests: Mutex<Vec<PreparedRequest>>,
}

struct FirstAddressFails {
    first: IpAddr,
    attempts: Mutex<Vec<IpAddr>>,
}
#[async_trait]
impl OutboundTransport for FirstAddressFails {
    async fn execute(
        &self,
        _: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        _: Duration,
        _: Duration,
    ) -> Result<TransportResponse, TransportError> {
        self.attempts.lock().unwrap().push(authorization.address());
        if authorization.address() == self.first {
            return Err(TransportError { kind: "request" });
        }
        let mut value = response(StatusCode::OK, None, b"ok");
        value.connected_peer = authorization.address();
        Ok(value)
    }
}
#[async_trait]
impl OutboundTransport for Transport {
    async fn execute(
        &self,
        request: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        _: Duration,
        _: Duration,
    ) -> Result<TransportResponse, TransportError> {
        self.requests.lock().unwrap().push(request.clone());
        let mut response = self.responses.lock().unwrap().pop_front().unwrap();
        response.connected_peer = authorization.address();
        Ok(response)
    }
}

#[derive(Default)]
struct Observer(Mutex<Vec<ExternalRequestCompletion>>);
impl ExternalRequestObserver for Arc<Observer> {
    fn completed(&self, event: ExternalRequestCompletion) {
        self.0.lock().unwrap().push(event);
    }
}

fn response(status: StatusCode, location: Option<&'static str>, body: &[u8]) -> TransportResponse {
    let mut headers = HeaderMap::new();
    if let Some(location) = location {
        headers.insert(header::LOCATION, HeaderValue::from_static(location));
    }
    TransportResponse {
        status,
        headers,
        connected_peer: Ipv4Addr::UNSPECIFIED.into(),
        body: Box::new(Body(VecDeque::from([body.to_vec()]))),
    }
}

#[tokio::test]
async fn redirect_is_resolved_again_and_cross_origin_secrets_are_removed() {
    let public: IpAddr = "93.184.216.34".parse().unwrap();
    let observer = Arc::new(Observer::default());
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(5),
                max_redirect_hops: 3,
                max_response_body_bytes: 10,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![public], vec![public]])),
        },
        Transport {
            responses: Mutex::new(VecDeque::from([
                response(StatusCode::FOUND, Some("https://b.test/final"), b""),
                response(StatusCode::OK, None, b"ok"),
            ])),
            requests: Mutex::new(vec![]),
        },
        observer.clone(),
    );
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, HeaderValue::from_static("secret"));
    let result = client
        .execute(PreparedRequest {
            method: Method::POST,
            url: Url::parse("https://a.test/start").unwrap(),
            headers,
            body: Some(vec![1]),
        })
        .await
        .unwrap();
    assert_eq!(result.body, b"ok");
    assert_eq!(
        observer.0.lock().unwrap()[0].outcome,
        ExternalRequestOutcome::Success
    );
}

#[tokio::test]
async fn redirect_to_private_dns_is_rejected_and_observed() {
    let observer = Arc::new(Observer::default());
    let public: IpAddr = "93.184.216.34".parse().unwrap();
    let private: IpAddr = "127.0.0.1".parse().unwrap();
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(5),
                max_redirect_hops: 3,
                max_response_body_bytes: 10,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![public], vec![private]])),
        },
        Transport {
            responses: Mutex::new(VecDeque::from([response(
                StatusCode::FOUND,
                Some("https://internal.test/"),
                b"",
            )])),
            requests: Mutex::new(vec![]),
        },
        observer.clone(),
    );
    let error = client
        .execute(PreparedRequest {
            method: Method::GET,
            url: Url::parse("https://a.test/").unwrap(),
            headers: HeaderMap::new(),
            body: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(error, OutboundError::ForbiddenAddress { .. }));
    assert_eq!(
        observer.0.lock().unwrap()[0].outcome,
        ExternalRequestOutcome::Rejected
    );
}

#[tokio::test]
async fn not_modified_is_returned_without_requiring_a_location_header() {
    let public: IpAddr = "93.184.216.34".parse().unwrap();
    let observer = Arc::new(Observer::default());
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(5),
                max_redirect_hops: 3,
                max_response_body_bytes: 10,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![public]])),
        },
        Transport {
            responses: Mutex::new(VecDeque::from([response(
                StatusCode::NOT_MODIFIED,
                None,
                b"",
            )])),
            requests: Mutex::new(vec![]),
        },
        observer.clone(),
    );

    let result = client
        .execute(PreparedRequest {
            method: Method::GET,
            url: Url::parse("https://feed.test/rss").unwrap(),
            headers: HeaderMap::new(),
            body: None,
        })
        .await
        .expect("304 is a terminal cache response");

    assert_eq!(result.status, StatusCode::NOT_MODIFIED);
    assert_eq!(
        observer.0.lock().unwrap()[0].outcome,
        ExternalRequestOutcome::Success
    );
}

#[tokio::test]
async fn configured_user_agent_is_applied_at_the_shared_outbound_boundary() {
    let public: IpAddr = "93.184.216.34".parse().unwrap();
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(5),
                max_redirect_hops: 1,
                max_response_body_bytes: 10,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![public]])),
        },
        Transport {
            responses: Mutex::new(VecDeque::from([response(StatusCode::OK, None, b"ok")])),
            requests: Mutex::new(vec![]),
        },
        Arc::new(Observer::default()),
    )
    .with_user_agent("inoreader/0.1")
    .unwrap();

    let result = client
        .execute(PreparedRequest {
            method: Method::GET,
            url: Url::parse("https://publisher.test/feed").unwrap(),
            headers: HeaderMap::new(),
            body: None,
        })
        .await
        .unwrap();
    assert_eq!(result.body, b"ok");
    assert_eq!(
        client.transport.requests.lock().unwrap()[0]
            .headers
            .get(header::USER_AGENT),
        Some(&HeaderValue::from_static("inoreader/0.1"))
    );
}

#[tokio::test]
async fn connection_retries_each_authorized_dns_address_within_the_deadline() {
    let first: IpAddr = "2001:4860:4860::8888".parse().unwrap();
    let second: IpAddr = "93.184.216.34".parse().unwrap();
    let transport = FirstAddressFails {
        first,
        attempts: Mutex::new(vec![]),
    };
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(5),
                max_redirect_hops: 1,
                max_response_body_bytes: 10,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![first, second]])),
        },
        transport,
        Arc::new(Observer::default()),
    );

    let result = client
        .execute(PreparedRequest {
            method: Method::GET,
            url: Url::parse("https://publisher.test/feed").unwrap(),
            headers: HeaderMap::new(),
            body: None,
        })
        .await
        .unwrap();

    assert_eq!(result.body, b"ok");
    assert_eq!(
        *client.transport.attempts.lock().unwrap(),
        vec![first, second]
    );
}
