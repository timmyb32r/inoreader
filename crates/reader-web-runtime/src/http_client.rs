use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use http::{header, HeaderMap, Method, StatusCode};
use thiserror::Error;
use url::Url;

use crate::{
    ConnectionAuthorization, Deadline, OutboundError, OutboundPolicy, PreparedRequest,
    RedirectChain,
};

#[async_trait]
pub trait DnsResolver: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, ResolveError>;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("DNS resolution failed")]
pub struct ResolveError;

#[async_trait]
pub trait ResponseBody: Send {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError>;
}

pub struct TransportResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// Authorized origin endpoint. Direct transports verify the socket peer;
    /// an explicitly configured CONNECT transport verifies its proxy socket,
    /// tunnels to this literal IP and verifies the original origin's TLS name.
    /// A proxy hostname or a proxy-resolved destination is never sufficient.
    pub connected_peer: IpAddr,
    pub body: Box<dyn ResponseBody>,
}

/// Low-level adapter contract. Implementations must disable automatic redirects,
/// connect to `authorization.address()` directly or via an explicitly trusted
/// public CONNECT route, retain the URL host for Host/SNI, apply
/// `connect_timeout`, and stream the response body without an unbounded buffer.
#[async_trait]
pub trait OutboundTransport: Send + Sync {
    async fn execute(
        &self,
        request: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        connect_timeout: Duration,
        remaining_deadline: Duration,
    ) -> Result<TransportResponse, TransportError>;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("transport failed: {kind}")]
pub struct TransportError {
    /// Stable credential-free classification. Never put a URL, header, body, or
    /// raw SDK error in this value.
    pub kind: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalRequestOutcome {
    Success,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalRequestCompletion {
    pub system: &'static str,
    pub operation: &'static str,
    pub outcome: ExternalRequestOutcome,
    pub elapsed: Duration,
}

pub trait ExternalRequestObserver: Send + Sync {
    fn completed(&self, completion: ExternalRequestCompletion);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboundResponse {
    pub final_url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

pub struct OutboundHttpClient<R, T, O> {
    policy: OutboundPolicy,
    resolver: R,
    transport: T,
    observer: O,
    user_agent: Option<header::HeaderValue>,
}

impl<R, T, O> OutboundHttpClient<R, T, O>
where
    R: DnsResolver,
    T: OutboundTransport,
    O: ExternalRequestObserver,
{
    pub fn new(policy: OutboundPolicy, resolver: R, transport: T, observer: O) -> Self {
        Self {
            policy,
            resolver,
            transport,
            observer,
            user_agent: None,
        }
    }

    pub fn with_user_agent(mut self, value: &str) -> Result<Self, header::InvalidHeaderValue> {
        self.user_agent = Some(header::HeaderValue::from_str(value)?);
        Ok(self)
    }

    pub fn restricted_to_origin(mut self, url: &Url) -> Result<Self, OutboundError> {
        self.policy = self.policy.restricted_to_origin(url)?;
        Ok(self)
    }

    pub async fn execute(
        &self,
        request: PreparedRequest,
    ) -> Result<OutboundResponse, OutboundError> {
        let mut stream = self
            .execute_stream(request, "public_web", "http_request")
            .await?;
        let mut body = Vec::new();
        while let Some(chunk) = stream.next_chunk().await? {
            body.extend(chunk);
        }
        Ok(OutboundResponse {
            final_url: stream.final_url.clone(),
            status: stream.status,
            headers: stream.headers.clone(),
            body,
        })
    }

    /// Opens a bounded validated stream. Dropping it cancels reception and records
    /// an interrupted external request. Non-idempotent requests are never retried
    /// against another IP after an ambiguous transport failure.
    pub async fn execute_stream(
        &self,
        mut request: PreparedRequest,
        system: &'static str,
        operation: &'static str,
    ) -> Result<OutboundResponseStream<'_>, OutboundError> {
        if let Some(user_agent) = &self.user_agent {
            request
                .headers
                .entry(header::USER_AGENT)
                .or_insert_with(|| user_agent.clone());
        }
        let started = Instant::now();
        let mut opening = OpeningObservation {
            observer: &self.observer,
            started,
            system,
            operation,
            active: true,
        };
        let deadline = Deadline::new(self.policy.limits().request_deadline);
        let result = self.open_stream(&mut request, &deadline).await;
        opening.active = false;
        match result {
            Ok(response) => Ok(OutboundResponseStream {
                final_url: request.url,
                status: response.status,
                headers: response.headers,
                body: response.body,
                deadline,
                received: 0,
                limit: self.policy.limits().max_response_body_bytes,
                observer: &self.observer,
                started,
                system,
                operation,
                finished: false,
            }),
            Err(error) => {
                self.observer.completed(ExternalRequestCompletion {
                    system,
                    operation,
                    outcome: if matches!(error, OutboundError::Transport { .. }) {
                        ExternalRequestOutcome::Failed
                    } else {
                        ExternalRequestOutcome::Rejected
                    },
                    elapsed: started.elapsed(),
                });
                Err(error)
            }
        }
    }

    async fn open_stream(
        &self,
        request: &mut PreparedRequest,
        deadline: &Deadline,
    ) -> Result<TransportResponse, OutboundError> {
        self.policy.validate_url(&request.url)?;
        let mut redirects =
            RedirectChain::new(&request.url, self.policy.limits().max_redirect_hops);
        let mut recovered_cache_validation = false;

        'request: loop {
            self.policy.validate_url(&request.url)?;
            let host = request.url.host_str().ok_or(OutboundError::MissingHost)?;
            let port = request
                .url
                .port_or_known_default()
                .ok_or(OutboundError::MissingHost)?;
            let addresses =
                tokio::time::timeout(deadline.remaining()?, self.resolver.resolve(host, port))
                    .await
                    .map_err(|_| OutboundError::Transport { kind: "deadline" })?
                    .map_err(|_| OutboundError::Transport { kind: "dns" })?;
            let resolved = self.policy.authorize_resolution(&request.url, addresses)?;
            // Try every address from the authorized DNS answer. Hosts commonly
            // publish IPv6 before IPv4 even when the caller has no IPv6 route.
            // The overall deadline remains shared across all attempts.
            let mut response = None;
            let mut last_transport_error = None;
            for address in resolved.addresses() {
                let authorization = resolved.connection(*address)?;
                let remaining = deadline.remaining()?;
                let attempt_started = Instant::now();
                match self
                    .transport
                    .execute(
                        request,
                        &authorization,
                        self.policy.limits().connect_timeout.min(remaining),
                        remaining,
                    )
                    .await
                {
                    Ok(value) => {
                        response = Some((value, authorization));
                        break;
                    }
                    Err(error) => {
                        // Some origins close conditional requests before sending
                        // headers. Cache validators are an optimization: retry a
                        // body-free GET once without them, within this same
                        // deadline/worker attempt. Re-enter URL/DNS validation;
                        // never replay a response body or mutation precondition.
                        if error.kind == "response_incomplete_before_headers"
                            && !recovered_cache_validation
                            && can_retry_cache_validation(request)
                        {
                            self.observer.completed(ExternalRequestCompletion {
                                system: "public_web",
                                operation: "conditional_cache_recovery",
                                outcome: ExternalRequestOutcome::Failed,
                                elapsed: attempt_started.elapsed(),
                            });
                            request.headers.remove(header::IF_NONE_MATCH);
                            request.headers.remove(header::IF_MODIFIED_SINCE);
                            recovered_cache_validation = true;
                            continue 'request;
                        }
                        if !matches!(request.method, Method::GET | Method::HEAD) {
                            return Err(OutboundError::Transport { kind: error.kind });
                        }
                        last_transport_error = Some(error.kind);
                    }
                }
            }
            let (response, authorization) = response.ok_or(OutboundError::Transport {
                kind: last_transport_error.unwrap_or("no_authorized_address"),
            })?;
            authorization.verify_connected_peer(response.connected_peer)?;

            if follows_location(response.status) {
                let location = response
                    .headers
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or(OutboundError::InvalidRedirectLocation)?;
                let next = redirects.follow(&request.url, location)?;
                self.policy.validate_url(&next)?;
                *request = request.for_redirect(next);
                continue;
            }

            return Ok(response);
        }
    }
}

fn can_retry_cache_validation(request: &PreparedRequest) -> bool {
    request.method == Method::GET
        && request.body.is_none()
        && (request.headers.contains_key(header::IF_NONE_MATCH)
            || request.headers.contains_key(header::IF_MODIFIED_SINCE))
        && ![
            header::RANGE,
            header::IF_RANGE,
            header::IF_MATCH,
            header::IF_UNMODIFIED_SINCE,
        ]
        .iter()
        .any(|name| request.headers.contains_key(name))
}

pub struct OutboundResponseStream<'a> {
    pub final_url: Url,
    pub status: StatusCode,
    pub headers: HeaderMap,
    body: Box<dyn ResponseBody>,
    deadline: Deadline,
    received: usize,
    limit: usize,
    observer: &'a dyn ExternalRequestObserver,
    started: Instant,
    system: &'static str,
    operation: &'static str,
    finished: bool,
}
struct OpeningObservation<'a> {
    observer: &'a dyn ExternalRequestObserver,
    started: Instant,
    system: &'static str,
    operation: &'static str,
    active: bool,
}
impl Drop for OpeningObservation<'_> {
    fn drop(&mut self) {
        if self.active {
            self.observer.completed(ExternalRequestCompletion {
                system: self.system,
                operation: self.operation,
                outcome: ExternalRequestOutcome::Failed,
                elapsed: self.started.elapsed(),
            });
        }
    }
}
impl OutboundResponseStream<'_> {
    pub async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, OutboundError> {
        let result = self.read_chunk().await;
        if let Err(error) = &result {
            self.finish(if matches!(error, OutboundError::Transport { .. }) {
                ExternalRequestOutcome::Failed
            } else {
                ExternalRequestOutcome::Rejected
            });
        } else if matches!(result, Ok(None)) {
            self.finish(
                if self.status.is_success() || self.status == StatusCode::NOT_MODIFIED {
                    ExternalRequestOutcome::Success
                } else {
                    ExternalRequestOutcome::Failed
                },
            );
        }
        result
    }
    async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, OutboundError> {
        if self.finished {
            return Ok(None);
        }
        let chunk = tokio::time::timeout(self.deadline.remaining()?, self.body.next_chunk())
            .await
            .map_err(|_| OutboundError::Transport { kind: "deadline" })?
            .map_err(|e| OutboundError::Transport { kind: e.kind })?;
        if let Some(value) = &chunk {
            self.received = self
                .received
                .checked_add(value.len())
                .ok_or(OutboundError::ResponseBodyTooLarge { limit: self.limit })?;
            if self.received > self.limit {
                return Err(OutboundError::ResponseBodyTooLarge { limit: self.limit });
            }
        }
        Ok(chunk)
    }
    fn finish(&mut self, outcome: ExternalRequestOutcome) {
        if !self.finished {
            self.finished = true;
            self.observer.completed(ExternalRequestCompletion {
                system: self.system,
                operation: self.operation,
                outcome,
                elapsed: self.started.elapsed(),
            });
        }
    }
}
impl Drop for OutboundResponseStream<'_> {
    fn drop(&mut self) {
        self.finish(ExternalRequestOutcome::Failed);
    }
}

fn follows_location(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

#[cfg(test)]
mod tests;
