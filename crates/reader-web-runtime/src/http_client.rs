use std::{
    net::IpAddr,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use http::{header, HeaderMap, StatusCode};
use thiserror::Error;
use url::Url;

use crate::{
    BoundedBody, ConnectionAuthorization, Deadline, OutboundError, OutboundPolicy, PreparedRequest,
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
    pub connected_peer: IpAddr,
    pub body: Box<dyn ResponseBody>,
}

/// Low-level adapter contract. Implementations must disable automatic redirects,
/// connect to `authorization.address()`, retain the URL host for Host/SNI, apply
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
        }
    }

    pub async fn execute(
        &self,
        mut request: PreparedRequest,
    ) -> Result<OutboundResponse, OutboundError> {
        let started = Instant::now();
        let result = self.execute_inner(&mut request).await;
        let outcome = match &result {
            Ok(_) => ExternalRequestOutcome::Success,
            Err(OutboundError::Transport { .. }) => ExternalRequestOutcome::Failed,
            Err(_) => ExternalRequestOutcome::Rejected,
        };
        self.observer.completed(ExternalRequestCompletion {
            system: "public_web",
            operation: "http_request",
            outcome,
            elapsed: started.elapsed(),
        });
        result
    }

    async fn execute_inner(
        &self,
        request: &mut PreparedRequest,
    ) -> Result<OutboundResponse, OutboundError> {
        self.policy.validate_url(&request.url)?;
        let deadline = Deadline::new(self.policy.limits().request_deadline);
        let mut redirects =
            RedirectChain::new(&request.url, self.policy.limits().max_redirect_hops);

        loop {
            self.policy.validate_url(&request.url)?;
            let host = request.url.host_str().ok_or(OutboundError::MissingHost)?;
            let port = request
                .url
                .port_or_known_default()
                .ok_or(OutboundError::MissingHost)?;
            let addresses = self
                .resolver
                .resolve(host, port)
                .await
                .map_err(|_| OutboundError::Transport { kind: "dns" })?;
            let resolved = self.policy.authorize_resolution(&request.url, addresses)?;
            // A transport may choose another authorized address on a retry, but it
            // never resolves the name itself. That closes the validation/connect gap.
            let address = *resolved
                .addresses()
                .first()
                .ok_or(OutboundError::DnsNoAddresses)?;
            let authorization = resolved.connection(address)?;
            let remaining = deadline.remaining()?;
            let response = self
                .transport
                .execute(
                    request,
                    &authorization,
                    self.policy.limits().connect_timeout.min(remaining),
                    remaining,
                )
                .await
                .map_err(|error| OutboundError::Transport { kind: error.kind })?;
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

            let mut source = response.body;
            let mut body = BoundedBody::new(self.policy.limits().max_response_body_bytes);
            while let Some(chunk) = source
                .next_chunk()
                .await
                .map_err(|error| OutboundError::Transport { kind: error.kind })?
            {
                deadline.remaining()?;
                body.push(&chunk)?;
            }
            return Ok(OutboundResponse {
                final_url: request.url.clone(),
                status: response.status,
                headers: response.headers,
                body: body.into_bytes(),
            });
        }
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
