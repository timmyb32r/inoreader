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

#[tokio::test]
async fn paid_post_is_never_retried_on_another_address_after_transport_error() {
    let first: IpAddr = "93.184.216.34".parse().unwrap();
    let second: IpAddr = "93.184.216.35".parse().unwrap();
    let observer = Arc::new(Observer::default());
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(2),
                max_redirect_hops: 1,
                max_response_body_bytes: 100,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![first, second]])),
        },
        FirstAddressFails {
            first,
            attempts: Mutex::new(vec![]),
        },
        observer.clone(),
    );
    let request = PreparedRequest {
        method: Method::POST,
        url: Url::parse("https://model.test/chat").unwrap(),
        headers: HeaderMap::new(),
        body: Some(b"private input".to_vec()),
    };
    assert!(client
        .execute_stream(request, "deepseek", "chat_completion")
        .await
        .is_err());
    assert_eq!(*client.transport.attempts.lock().unwrap(), vec![first]);
    assert_eq!(observer.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn dropping_stream_cancels_body_and_records_completion_once() {
    let public: IpAddr = "93.184.216.34".parse().unwrap();
    let observer = Arc::new(Observer::default());
    let client = OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: Duration::from_secs(1),
                request_deadline: Duration::from_secs(2),
                max_redirect_hops: 1,
                max_response_body_bytes: 100,
            },
        ),
        Resolver {
            answers: Mutex::new(VecDeque::from([vec![public]])),
        },
        Transport {
            responses: Mutex::new(VecDeque::from([response(StatusCode::OK, None, b"partial")])),
            requests: Mutex::new(vec![]),
        },
        observer.clone(),
    );
    let mut stream = client
        .execute_stream(
            PreparedRequest {
                method: Method::GET,
                url: Url::parse("https://model.test/chat").unwrap(),
                headers: HeaderMap::new(),
                body: None,
            },
            "deepseek",
            "chat_completion",
        )
        .await
        .unwrap();
    assert_eq!(
        stream.next_chunk().await.unwrap(),
        Some(b"partial".to_vec())
    );
    drop(stream);
    let events = observer.0.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].system, "deepseek");
    assert_eq!(events[0].outcome, ExternalRequestOutcome::Failed);
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

#[tokio::test]
async fn credential_in_path_origin_restriction_checks_every_redirect_before_transport() {
    let public: IpAddr = "93.184.216.34".parse().unwrap();
    for (location, addresses, success, calls) in [
        (
            "https://api.telegram.org/next",
            vec![vec![public], vec![public]],
            true,
            2,
        ),
        (
            "https://attacker.test/bot123:private/getUpdates",
            vec![vec![public]],
            false,
            1,
        ),
        (
            "https://api.telegram.org/next",
            vec![vec![public], vec!["127.0.0.1".parse().unwrap()]],
            false,
            1,
        ),
        (
            "https://api.telegram.org/bot123:private/getUpdates",
            vec![vec![public]],
            false,
            1,
        ),
    ] {
        let observer = Arc::new(Observer::default());
        let client = OutboundHttpClient::new(
            OutboundPolicy::new(
                false,
                OutboundLimits {
                    connect_timeout: Duration::from_secs(1),
                    request_deadline: Duration::from_secs(2),
                    max_redirect_hops: 1,
                    max_response_body_bytes: 100,
                },
            ),
            Resolver {
                answers: Mutex::new(addresses.into()),
            },
            Transport {
                responses: Mutex::new(VecDeque::from([
                    response(StatusCode::TEMPORARY_REDIRECT, Some(location), b""),
                    response(StatusCode::OK, None, b"ok"),
                ])),
                requests: Mutex::new(vec![]),
            },
            observer.clone(),
        )
        .restricted_to_origin(&Url::parse("https://api.telegram.org").unwrap())
        .unwrap();
        let result = client
            .execute_stream(
                PreparedRequest {
                    method: Method::POST,
                    url: Url::parse("https://api.telegram.org/bot123:private/getUpdates").unwrap(),
                    headers: HeaderMap::new(),
                    body: Some(b"private parameters".to_vec()),
                },
                "telegram",
                "getUpdates",
            )
            .await;
        assert_eq!(result.is_ok(), success);
        if let Ok(mut stream) = result {
            while stream.next_chunk().await.unwrap().is_some() {}
        }
        assert_eq!(client.transport.requests.lock().unwrap().len(), calls);
        let observed = observer.0.lock().unwrap();
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].system, "telegram");
        assert_eq!(observed[0].operation, "getUpdates");
    }
}

mod conditional;
