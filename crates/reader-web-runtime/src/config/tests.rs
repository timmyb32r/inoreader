use super::*;

#[test]
fn rejects_implicit_or_contradictory_limits() {
    let raw = RawOutboundLimits {
        connect_timeout_ms: 2_000,
        request_deadline_ms: 1_000,
        max_redirect_hops: 5,
        max_response_body_bytes: 10,
    };
    assert_eq!(
        OutboundLimits::try_from(raw),
        Err(LimitsError::ConnectExceedsDeadline)
    );
}

#[test]
fn browser_downloads_are_explicitly_disabled() {
    let raw = RawBrowserLimits {
        max_contexts: 1,
        max_pages_per_context: 1,
        preview_deadline_ms: 1_000,
        navigation_deadline_ms: 500,
        max_actions: 1,
        max_downloads: 1,
    };
    assert!(BrowserLimits::try_from(raw).is_err());
}
