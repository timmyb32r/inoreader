use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, Instant},
};

use http::{header, HeaderMap, Method};
use thiserror::Error;
use url::Url;

use crate::OutboundLimits;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboundPolicy {
    allow_plain_http: bool,
    limits: OutboundLimits,
}

impl OutboundPolicy {
    pub fn new(allow_plain_http: bool, limits: OutboundLimits) -> Self {
        Self {
            allow_plain_http,
            limits,
        }
    }

    pub fn limits(&self) -> &OutboundLimits {
        &self.limits
    }

    pub fn validate_url(&self, url: &Url) -> Result<(), OutboundError> {
        match url.scheme() {
            "https" => {}
            "http" if self.allow_plain_http => {}
            "http" => return Err(OutboundError::PlainHttpRequiresExplicitPolicy),
            scheme => return Err(OutboundError::UnsupportedScheme(scheme.to_owned())),
        }
        if url.host_str().is_none() {
            return Err(OutboundError::MissingHost);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(OutboundError::UrlCredentialsForbidden);
        }
        Ok(())
    }

    pub fn authorize_resolution(
        &self,
        url: &Url,
        addresses: Vec<IpAddr>,
    ) -> Result<ResolvedTarget, OutboundError> {
        self.validate_url(url)?;
        if addresses.is_empty() {
            return Err(OutboundError::DnsNoAddresses);
        }
        for address in &addresses {
            validate_public_address(*address)?;
        }
        Ok(ResolvedTarget {
            url: url.clone(),
            addresses,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTarget {
    url: Url,
    addresses: Vec<IpAddr>,
}

impl ResolvedTarget {
    pub fn url(&self) -> &Url {
        &self.url
    }
    pub fn addresses(&self) -> &[IpAddr] {
        &self.addresses
    }

    pub fn connection(&self, address: IpAddr) -> Result<ConnectionAuthorization, OutboundError> {
        validate_public_address(address)?;
        if !self.addresses.contains(&address) {
            return Err(OutboundError::ConnectedAddressChanged { address });
        }
        Ok(ConnectionAuthorization {
            url: self.url.clone(),
            address,
        })
    }
}

/// Opaque proof passed to the transport. The adapter must connect to `address`
/// directly while retaining the URL host for HTTP Host/SNI verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionAuthorization {
    url: Url,
    address: IpAddr,
}

impl ConnectionAuthorization {
    pub fn url(&self) -> &Url {
        &self.url
    }
    pub fn address(&self) -> IpAddr {
        self.address
    }

    pub fn verify_connected_peer(&self, peer: IpAddr) -> Result<(), OutboundError> {
        validate_public_address(peer)?;
        if peer != self.address {
            return Err(OutboundError::ConnectedAddressChanged { address: peer });
        }
        Ok(())
    }
}

pub fn validate_public_address(address: IpAddr) -> Result<(), OutboundError> {
    let forbidden = match address {
        IpAddr::V4(v4) => forbidden_v4(v4),
        IpAddr::V6(v6) => forbidden_v6(v6),
    };
    if forbidden {
        Err(OutboundError::ForbiddenAddress { address })
    } else {
        Ok(())
    }
}

fn forbidden_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, d] = ip.octets();
    a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || (a == 255 && b == 255 && c == 255 && d == 255)
}

fn forbidden_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return forbidden_v4(v4);
    }
    let s = ip.segments();
    ip.is_unspecified() || ip.is_loopback() || ip.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // unique local
        || (s[0] & 0xffc0) == 0xfe80 // link local
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
        || (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0 && s[3] == 0 && forbidden_v4(Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8)))
}

#[derive(Clone, Debug)]
pub struct PreparedRequest {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<Vec<u8>>,
}

impl PreparedRequest {
    pub fn for_redirect(&self, next: Url) -> Self {
        let cross_origin = origin(&self.url) != origin(&next);
        let mut headers = self.headers.clone();
        headers.remove(header::HOST);
        if cross_origin {
            let sensitive_names: Vec<_> = headers
                .iter()
                .filter(|(name, value)| is_sensitive_header(name.as_str()) || value.is_sensitive())
                .map(|(name, _)| name.clone())
                .collect();
            for name in sensitive_names {
                headers.remove(name);
            }
            headers.remove(header::REFERER);
        }
        Self {
            method: self.method.clone(),
            url: next,
            headers,
            body: if cross_origin {
                None
            } else {
                self.body.clone()
            },
        }
    }
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "cookie2"
            | "x-api-key"
            | "x-auth-token"
            | "x-csrf-token"
    )
}

fn origin(url: &Url) -> (String, Option<String>, Option<u16>) {
    (
        url.scheme().to_owned(),
        url.host_str().map(str::to_ascii_lowercase),
        url.port_or_known_default(),
    )
}

#[derive(Debug)]
pub struct RedirectChain {
    seen: HashSet<String>,
    hops: usize,
    max_hops: usize,
}

impl RedirectChain {
    pub fn new(initial: &Url, max_hops: usize) -> Self {
        Self {
            seen: HashSet::from([initial.as_str().to_owned()]),
            hops: 0,
            max_hops,
        }
    }

    pub fn follow(&mut self, current: &Url, location: &str) -> Result<Url, OutboundError> {
        if self.hops >= self.max_hops {
            return Err(OutboundError::RedirectLimitExceeded);
        }
        let next = current
            .join(location)
            .map_err(|_| OutboundError::InvalidRedirectLocation)?;
        if !self.seen.insert(next.as_str().to_owned()) {
            return Err(OutboundError::RedirectLoop);
        }
        self.hops += 1;
        Ok(next)
    }
}

#[derive(Debug)]
pub struct Deadline {
    started: Instant,
    duration: Duration,
}

impl Deadline {
    pub fn new(duration: Duration) -> Self {
        Self {
            started: Instant::now(),
            duration,
        }
    }
    pub fn remaining(&self) -> Result<Duration, OutboundError> {
        self.duration
            .checked_sub(self.started.elapsed())
            .ok_or(OutboundError::DeadlineExceeded)
    }
}

#[derive(Debug)]
pub struct BoundedBody {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedBody {
    pub fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), OutboundError> {
        let next = self
            .bytes
            .len()
            .checked_add(chunk.len())
            .ok_or(OutboundError::ResponseBodyTooLarge { limit: self.limit })?;
        if next > self.limit {
            return Err(OutboundError::ResponseBodyTooLarge { limit: self.limit });
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum OutboundError {
    #[error("unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("plain HTTP requires explicit operator policy")]
    PlainHttpRequiresExplicitPolicy,
    #[error("URL has no host")]
    MissingHost,
    #[error("credentials in a URL are forbidden")]
    UrlCredentialsForbidden,
    #[error("DNS returned no addresses")]
    DnsNoAddresses,
    #[error("destination address is forbidden: {address}")]
    ForbiddenAddress { address: IpAddr },
    #[error("connected peer was not the authorized DNS result: {address}")]
    ConnectedAddressChanged { address: IpAddr },
    #[error("redirect limit exceeded")]
    RedirectLimitExceeded,
    #[error("redirect loop detected")]
    RedirectLoop,
    #[error("invalid redirect location")]
    InvalidRedirectLocation,
    #[error("overall request deadline exceeded")]
    DeadlineExceeded,
    #[error("response body exceeds configured limit of {limit} bytes")]
    ResponseBodyTooLarge { limit: usize },
    #[error("transport failed: {kind}")]
    Transport { kind: &'static str },
}

#[cfg(test)]
mod tests;
