//! Closed credentialed capability for replay-safe Telegram Bot API requests.
use super::*;

pub(super) fn telegram_headers(request: &PreparedRequest) -> Result<HeaderMap, TransportError> {
    let url = &request.url;
    let method = url.path().rsplit('/').next().unwrap_or("");
    let token = url
        .path()
        .strip_prefix("/bot")
        .and_then(|p| p.strip_suffix(&format!("/{method}")));
    if url.scheme() != "https"
        || url.host_str() != Some("api.telegram.org")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || request.method != Method::POST
        || request.body.is_none()
        || !matches!(
            method,
            "getMe" | "getWebhookInfo" | "getChat" | "getChatMember" | "getUpdates"
        )
        || !token.is_some_and(|t| !t.is_empty() && !t.contains('/'))
    {
        return Err(failure("proxy_telegram_https_read_required"));
    }
    // Token is only in the encrypted origin request path. No proxy credentials,
    // cookies, arbitrary headers or redirect-provided authorization are allowed.
    if request
        .headers
        .keys()
        .any(|name| !matches!(name.as_str(), "content-type" | "accept" | "user-agent"))
    {
        return Err(failure("proxy_telegram_headers_forbidden"));
    }
    let mut headers = request.headers.clone();
    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("identity"),
    );
    Ok(headers)
}
