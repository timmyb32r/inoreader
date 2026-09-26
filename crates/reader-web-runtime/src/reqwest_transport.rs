use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    time::Duration,
};

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};

use crate::{
    ConnectionAuthorization, DnsResolver, OutboundTransport, PreparedRequest, ResolveError,
    ResponseBody, TransportError, TransportResponse,
};

/// Production DNS adapter. Resolution is performed before policy authorization;
/// the resulting address is then pinned into the concrete HTTP client.
#[derive(Clone, Copy, Debug, Default)]
pub struct TokioDnsResolver;

#[async_trait]
impl DnsResolver for TokioDnsResolver {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, ResolveError> {
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| ResolveError)?;
        let mut result = Vec::new();
        for address in addresses {
            if !result.contains(&address.ip()) {
                result.push(address.ip());
            }
        }
        Ok(result)
    }
}

/// Concrete transport with automatic redirects disabled. A fresh client is
/// intentionally built for each authorized hop so DNS pinning cannot leak from
/// one origin or resolution to another. The shared `OutboundHttpClient` remains
/// the only public high-level request API.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReqwestPinnedTransport;

fn pinned_client(
    host: &str,
    socket: SocketAddr,
    connect_timeout: Duration,
    remaining_deadline: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(connect_timeout)
        .timeout(remaining_deadline)
        .resolve(host, socket)
        .build()
}

type ByteStream =
    Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static>>;

struct ReqwestBody {
    stream: ByteStream,
}

#[async_trait]
impl ResponseBody for ReqwestBody {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        match self.stream.next().await {
            Some(Ok(chunk)) => Ok(Some(chunk.to_vec())),
            Some(Err(_)) => Err(TransportError {
                kind: "response_body",
            }),
            None => Ok(None),
        }
    }
}

#[async_trait]
impl OutboundTransport for ReqwestPinnedTransport {
    async fn execute(
        &self,
        request: &PreparedRequest,
        authorization: &ConnectionAuthorization,
        connect_timeout: Duration,
        remaining_deadline: Duration,
    ) -> Result<TransportResponse, TransportError> {
        let host = authorization.url().host_str().ok_or(TransportError {
            kind: "missing_host",
        })?;
        let port = authorization
            .url()
            .port_or_known_default()
            .ok_or(TransportError {
                kind: "missing_port",
            })?;
        let socket = SocketAddr::new(authorization.address(), port);
        let client =
            pinned_client(host, socket, connect_timeout, remaining_deadline).map_err(|_| {
                TransportError {
                    kind: "client_build",
                }
            })?;
        let mut builder = client.request(request.method.clone(), request.url.clone());
        builder = builder.headers(request.headers.clone());
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        let response = builder.send().await.map_err(|error| TransportError {
            kind: if error.is_timeout() {
                "request_timeout"
            } else if error.is_connect() {
                "connect"
            } else if error.is_body() {
                "request_body"
            } else {
                "request"
            },
        })?;
        let connected_peer = response
            .remote_addr()
            .ok_or(TransportError {
                kind: "missing_peer_address",
            })?
            .ip();
        authorization
            .verify_connected_peer(connected_peer)
            .map_err(|_| TransportError {
                kind: "peer_mismatch",
            })?;
        let status = response.status();
        let headers = response.headers().clone();
        Ok(TransportResponse {
            status,
            headers,
            connected_peer,
            body: Box::new(ReqwestBody {
                stream: Box::pin(response.bytes_stream()),
            }),
        })
    }
}

#[cfg(test)]
mod tests;
