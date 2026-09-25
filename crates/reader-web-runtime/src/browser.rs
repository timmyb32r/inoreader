use std::collections::HashMap;

use http::HeaderMap;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use crate::{OutboundError, OutboundPolicy};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BrowserJobId(Uuid);
impl BrowserJobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for BrowserJobId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BrowserContextId(Uuid);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserContextLease {
    job: BrowserJobId,
    context: BrowserContextId,
}
impl BrowserContextLease {
    pub fn context(&self) -> BrowserContextId {
        self.context
    }
    pub fn job(&self) -> BrowserJobId {
        self.job
    }
}

/// Pure ownership model used by the CDP adapter. A context belongs to exactly
/// one job and must be destroyed rather than returned to a shared cookie jar.
#[derive(Debug)]
pub struct ContextIsolation {
    max_contexts: usize,
    active: HashMap<BrowserContextId, BrowserJobId>,
}

impl ContextIsolation {
    pub fn new(max_contexts: usize) -> Self {
        Self {
            max_contexts,
            active: HashMap::new(),
        }
    }
    pub fn acquire(
        &mut self,
        job: BrowserJobId,
    ) -> Result<BrowserContextLease, BrowserPolicyError> {
        if self.active.len() >= self.max_contexts {
            return Err(BrowserPolicyError::ContextLimit);
        }
        let context = BrowserContextId(Uuid::new_v4());
        self.active.insert(context, job);
        Ok(BrowserContextLease { job, context })
    }
    pub fn authorize(&self, lease: &BrowserContextLease) -> Result<(), BrowserPolicyError> {
        match self.active.get(&lease.context) {
            Some(job) if *job == lease.job => Ok(()),
            _ => Err(BrowserPolicyError::ContextMismatch),
        }
    }
    pub fn release(&mut self, lease: BrowserContextLease) -> Result<(), BrowserPolicyError> {
        self.authorize(&lease)?;
        self.active.remove(&lease.context);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserRequestKind {
    Document,
    Iframe,
    Subresource,
    Fetch,
    Xhr,
    WebSocket,
    Popup,
    Download,
}

#[derive(Clone, Debug)]
pub struct BrowserRequest {
    pub context: BrowserContextLease,
    pub kind: BrowserRequestKind,
    pub url: Url,
    pub headers: HeaderMap,
}

pub struct BrowserEgressPolicy {
    outbound: OutboundPolicy,
}

impl BrowserEgressPolicy {
    pub fn new(outbound: OutboundPolicy) -> Self {
        Self { outbound }
    }

    /// CDP interception must call this for every request kind. Successful URL
    /// authorization is only the first half: DNS and peer pinning are performed
    /// by the same outbound client used by non-browser fetches.
    pub fn authorize_url(
        &self,
        request: &BrowserRequest,
        contexts: &ContextIsolation,
    ) -> Result<(), BrowserPolicyError> {
        contexts.authorize(&request.context)?;
        match request.kind {
            BrowserRequestKind::WebSocket => return Err(BrowserPolicyError::WebSocketForbidden),
            BrowserRequestKind::Download => return Err(BrowserPolicyError::DownloadForbidden),
            _ => {}
        }
        self.outbound
            .validate_url(&request.url)
            .map_err(BrowserPolicyError::Outbound)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomSnapshotId(Uuid);
impl DomSnapshotId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for DomSnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedDomNode {
    snapshot: DomSnapshotId,
    backend_node_id: u64,
    selector: String,
}

impl SelectedDomNode {
    pub fn capture(
        snapshot: DomSnapshotId,
        backend_node_id: u64,
        selector: String,
    ) -> Result<Self, BrowserPolicyError> {
        if backend_node_id == 0 || selector.is_empty() {
            return Err(BrowserPolicyError::InvalidSelection);
        }
        Ok(Self {
            snapshot,
            backend_node_id,
            selector,
        })
    }
    pub fn apply_to(&self, current: DomSnapshotId) -> Result<u64, BrowserPolicyError> {
        if self.snapshot != current {
            return Err(BrowserPolicyError::StaleDomSnapshot);
        }
        Ok(self.backend_node_id)
    }
    pub fn selector(&self) -> &str {
        &self.selector
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BrowserPolicyError {
    #[error("browser context limit reached")]
    ContextLimit,
    #[error("browser context does not belong to this job")]
    ContextMismatch,
    #[error("WebSocket is not supported by the controlled HTTP(S) egress")]
    WebSocketForbidden,
    #[error("downloads are disabled")]
    DownloadForbidden,
    #[error("outbound request rejected: {0}")]
    Outbound(OutboundError),
    #[error("DOM selection is invalid")]
    InvalidSelection,
    #[error("DOM snapshot changed; the element must be selected again")]
    StaleDomSnapshot,
}

#[cfg(test)]
mod tests;
