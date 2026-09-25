use std::time::Duration;

use super::*;
use crate::OutboundLimits;

fn policy() -> BrowserEgressPolicy {
    BrowserEgressPolicy::new(OutboundPolicy::new(
        false,
        OutboundLimits {
            connect_timeout: Duration::from_secs(1),
            request_deadline: Duration::from_secs(2),
            max_redirect_hops: 2,
            max_response_body_bytes: 100,
        },
    ))
}

#[test]
fn every_browser_request_kind_uses_the_same_url_policy() {
    let job = BrowserJobId::new();
    let mut contexts = ContextIsolation::new(1);
    let lease = contexts.acquire(job).unwrap();
    for kind in [
        BrowserRequestKind::Document,
        BrowserRequestKind::Iframe,
        BrowserRequestKind::Subresource,
        BrowserRequestKind::Fetch,
        BrowserRequestKind::Xhr,
        BrowserRequestKind::Popup,
    ] {
        let request = BrowserRequest {
            context: lease.clone(),
            kind,
            url: Url::parse("http://169.254.169.254/latest/meta-data").unwrap(),
            headers: HeaderMap::new(),
        };
        assert!(
            policy().authorize_url(&request, &contexts).is_err(),
            "{kind:?}"
        );
    }
}

#[test]
fn contexts_are_job_scoped_and_not_reused() {
    let mut contexts = ContextIsolation::new(1);
    let first = contexts.acquire(BrowserJobId::new()).unwrap();
    assert_eq!(
        contexts.acquire(BrowserJobId::new()),
        Err(BrowserPolicyError::ContextLimit)
    );
    let old_context = first.context();
    contexts.release(first).unwrap();
    let second = contexts.acquire(BrowserJobId::new()).unwrap();
    assert_ne!(old_context, second.context());
}

#[test]
fn stale_coordinate_selection_cannot_target_a_new_dom() {
    let first = DomSnapshotId::new();
    let selection = SelectedDomNode::capture(first, 42, ".article".into()).unwrap();
    assert_eq!(
        selection.apply_to(DomSnapshotId::new()),
        Err(BrowserPolicyError::StaleDomSnapshot)
    );
    assert_eq!(selection.apply_to(first), Ok(42));
}
