use super::*;

struct ScriptedTransport {
    responses: Mutex<VecDeque<Result<TransportResponse, TransportError>>>,
    requests: Mutex<Vec<PreparedRequest>>,
    budgets: Mutex<Vec<Duration>>,
    delay: Duration,
}
#[async_trait]
impl OutboundTransport for ScriptedTransport {
    async fn execute(
        &self,
        request: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        _: Duration,
        remaining: Duration,
    ) -> Result<TransportResponse, TransportError> {
        self.requests.lock().unwrap().push(request.clone());
        self.budgets.lock().unwrap().push(remaining);
        tokio::time::sleep(self.delay).await;
        let mut result = self.responses.lock().unwrap().pop_front().unwrap()?;
        result.connected_peer = authorization.address();
        Ok(result)
    }
}

fn incomplete() -> Result<TransportResponse, TransportError> {
    Err(TransportError {
        kind: "response_incomplete_before_headers",
    })
}
fn request() -> PreparedRequest {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_static("\"original\""),
    );
    headers.insert(
        header::IF_MODIFIED_SINCE,
        HeaderValue::from_static("Fri, 25 Sep 2026 12:39:39 GMT"),
    );
    PreparedRequest {
        method: Method::GET,
        url: Url::parse("https://feed.test/feed/").unwrap(),
        headers,
        body: None,
    }
}
fn make_client(
    responses: Vec<Result<TransportResponse, TransportError>>,
    addresses: Vec<Vec<IpAddr>>,
    deadline: Duration,
    delay: Duration,
) -> OutboundHttpClient<Resolver, ScriptedTransport, Arc<Observer>> {
    OutboundHttpClient::new(
        OutboundPolicy::new(
            false,
            OutboundLimits {
                connect_timeout: deadline,
                request_deadline: deadline,
                max_redirect_hops: 2,
                max_response_body_bytes: 100,
            },
        ),
        Resolver {
            answers: Mutex::new(addresses.into()),
        },
        ScriptedTransport {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(vec![]),
            budgets: Mutex::new(vec![]),
            delay,
        },
        Arc::new(Observer::default()),
    )
}
fn public() -> IpAddr {
    "93.184.216.34".parse().unwrap()
}

#[tokio::test]
async fn incomplete_conditional_get_recovers_once_without_changing_bytes_or_stored_validators() {
    for absent in [
        None,
        Some(header::IF_NONE_MATCH),
        Some(header::IF_MODIFIED_SINCE),
    ] {
        let mut input = request();
        if let Some(name) = absent {
            input.headers.remove(name);
        }
        let original = input.clone();
        let body = b"\n<?xml?><rss>\x00\xff unchanged</rss>";
        let mut success = response(StatusCode::OK, None, body);
        success
            .headers
            .insert(header::ETAG, HeaderValue::from_static("\"new\""));
        let client = make_client(
            vec![incomplete(), Ok(success)],
            vec![vec![public()], vec![public()]],
            Duration::from_secs(1),
            Duration::from_millis(2),
        );
        let result = client.execute(input.clone()).await.unwrap();
        assert_eq!(result.body, body);
        assert_eq!(result.headers[header::ETAG], "\"new\"");
        assert_request_eq(&input, &original);
        let requests = client.transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_request_eq(&requests[0], &original);
        assert!(!requests[1].headers.contains_key(header::IF_NONE_MATCH));
        assert!(!requests[1].headers.contains_key(header::IF_MODIFIED_SINCE));
        let budgets = client.transport.budgets.lock().unwrap();
        assert!(budgets[1] < budgets[0]);
        let events = client.observer.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].system, "public_web");
        assert_eq!(events[0].operation, "conditional_cache_recovery");
        assert_eq!(events[0].outcome, ExternalRequestOutcome::Failed);
        assert_eq!(events[1].outcome, ExternalRequestOutcome::Success);
    }
}

#[tokio::test]
async fn recovery_never_repeats_or_applies_to_other_transport_failures() {
    for kind in [
        "response_incomplete_before_headers",
        "request",
        "connect",
        "response_body",
        "request_timeout",
    ] {
        let first = Err(TransportError { kind });
        let client = make_client(
            vec![first, incomplete()],
            vec![vec![public()], vec![public()]],
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert!(client.execute(request()).await.is_err());
        let expected = if kind == "response_incomplete_before_headers" {
            2
        } else {
            1
        };
        assert_eq!(
            client.transport.requests.lock().unwrap().len(),
            expected,
            "{kind}"
        );
    }
}

#[tokio::test]
async fn recovery_does_not_change_methods_bodies_ranges_or_noncache_preconditions() {
    let mut cases = vec![];
    for method in [Method::POST, Method::HEAD, Method::PUT] {
        let mut value = request();
        value.method = method;
        cases.push(value);
    }
    let mut with_body = request();
    with_body.body = Some(b"unchanged".to_vec());
    cases.push(with_body);
    let mut unconditional = request();
    unconditional.headers.clear();
    cases.push(unconditional);
    for name in [
        header::RANGE,
        header::IF_RANGE,
        header::IF_MATCH,
        header::IF_UNMODIFIED_SINCE,
    ] {
        let mut value = request();
        value
            .headers
            .insert(name, HeaderValue::from_static("unchanged"));
        cases.push(value);
    }
    for value in cases {
        let client = make_client(
            vec![incomplete()],
            vec![vec![public()]],
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert!(client.execute(value.clone()).await.is_err());
        let requests = client.transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_request_eq(&requests[0], &value);
    }
}

#[tokio::test]
async fn recovery_revalidates_dns_and_keeps_original_deadline() {
    let client = make_client(
        vec![incomplete()],
        vec![vec![public()], vec!["127.0.0.1".parse().unwrap()]],
        Duration::from_secs(1),
        Duration::ZERO,
    );
    assert!(matches!(
        client.execute(request()).await,
        Err(OutboundError::ForbiddenAddress { .. })
    ));
    assert_eq!(client.transport.requests.lock().unwrap().len(), 1);

    let client = make_client(
        vec![incomplete()],
        vec![vec![public()]],
        Duration::from_millis(5),
        Duration::from_millis(15),
    );
    assert!(matches!(
        client.execute(request()).await,
        Err(OutboundError::DeadlineExceeded)
    ));
    assert_eq!(client.transport.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn recovery_preserves_redirect_validation_and_secret_removal() {
    let mut input = request();
    input
        .headers
        .insert(header::AUTHORIZATION, HeaderValue::from_static("private"));
    let client = make_client(
        vec![
            incomplete(),
            Ok(response(
                StatusCode::FOUND,
                Some("https://other.test/feed"),
                b"",
            )),
            Ok(response(StatusCode::OK, None, b"full response")),
        ],
        vec![vec![public()], vec![public()], vec![public()]],
        Duration::from_secs(1),
        Duration::ZERO,
    );
    assert_eq!(client.execute(input).await.unwrap().body, b"full response");
    let requests = client.transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1].headers[header::AUTHORIZATION], "private");
    assert!(!requests[2].headers.contains_key(header::AUTHORIZATION));
}

#[tokio::test]
async fn valid_conditional_http_responses_do_not_trigger_recovery() {
    for status in [
        StatusCode::NOT_MODIFIED,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let client = make_client(
            vec![Ok(response(status, None, b""))],
            vec![vec![public()]],
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert_eq!(client.execute(request()).await.unwrap().status, status);
        assert_eq!(client.transport.requests.lock().unwrap().len(), 1);
    }
}

struct IncompleteBody;
#[async_trait]
impl ResponseBody for IncompleteBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        Err(TransportError {
            kind: "response_incomplete_before_headers",
        })
    }
}
#[tokio::test]
async fn response_body_failure_cannot_trigger_cache_recovery() {
    let mut response = response(StatusCode::OK, None, b"");
    response.body = Box::new(IncompleteBody);
    let client = make_client(
        vec![Ok(response)],
        vec![vec![public()]],
        Duration::from_secs(1),
        Duration::ZERO,
    );
    assert!(client.execute(request()).await.is_err());
    assert_eq!(client.transport.requests.lock().unwrap().len(), 1);
    assert_eq!(client.observer.0.lock().unwrap().len(), 1);
}

fn assert_request_eq(actual: &PreparedRequest, expected: &PreparedRequest) {
    assert_eq!(actual.method, expected.method);
    assert_eq!(actual.url, expected.url);
    assert_eq!(actual.headers, expected.headers);
    assert_eq!(actual.body, expected.body);
}
