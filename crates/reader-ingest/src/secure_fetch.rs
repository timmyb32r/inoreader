use async_trait::async_trait;
use http::{header, HeaderMap, Method};
use reader_web_runtime::{
    DnsResolver, ExternalRequestObserver, OutboundHttpClient, OutboundTransport, PreparedRequest,
};
use url::Url;

use crate::{CacheValidators, FeedFetcher, FetchError, FetchedPage, FullTextExtractor};

pub struct SecureWebFetcher<R, T, O> {
    client: OutboundHttpClient<R, T, O>,
    user_agent: Option<http::HeaderValue>,
}
impl<R, T, O> SecureWebFetcher<R, T, O> {
    pub fn new(client: OutboundHttpClient<R, T, O>) -> Self {
        Self {
            client,
            user_agent: None,
        }
    }
    pub fn with_user_agent(mut self, value: &str) -> Result<Self, FetchError> {
        self.user_agent = Some(
            value
                .parse()
                .map_err(|_| FetchError::Rejected("invalid_user_agent".into()))?,
        );
        Ok(self)
    }
}

#[async_trait]
impl<R, T, O> FeedFetcher for SecureWebFetcher<R, T, O>
where
    R: DnsResolver,
    T: OutboundTransport,
    O: ExternalRequestObserver,
    R: Send + Sync,
    T: Send + Sync,
    O: Send + Sync,
{
    async fn fetch(
        &self,
        url: &Url,
        validators: &CacheValidators,
    ) -> Result<FetchedPage, FetchError> {
        fetch(
            &self.client,
            url,
            Some(validators),
            self.user_agent.as_ref(),
        )
        .await
    }
}

#[async_trait]
impl<R, T, O> FullTextExtractor for SecureWebFetcher<R, T, O>
where
    R: DnsResolver,
    T: OutboundTransport,
    O: ExternalRequestObserver,
    R: Send + Sync,
    T: Send + Sync,
    O: Send + Sync,
{
    async fn extract(&self, url: &Url) -> Result<FetchedPage, FetchError> {
        fetch(&self.client, url, None, self.user_agent.as_ref()).await
    }
}

async fn fetch<R: DnsResolver, T: OutboundTransport, O: ExternalRequestObserver>(
    client: &OutboundHttpClient<R, T, O>,
    url: &Url,
    validators: Option<&CacheValidators>,
    user_agent: Option<&http::HeaderValue>,
) -> Result<FetchedPage, FetchError> {
    let mut headers = HeaderMap::new();
    if let Some(value) = user_agent {
        headers.insert(header::USER_AGENT, value.clone());
    }
    if let Some(validators) = validators {
        if let Some(value) = &validators.etag {
            headers.insert(
                header::IF_NONE_MATCH,
                value
                    .parse()
                    .map_err(|_| FetchError::Rejected("invalid_etag".into()))?,
            );
        }
        if let Some(value) = &validators.last_modified {
            headers.insert(
                header::IF_MODIFIED_SINCE,
                value
                    .parse()
                    .map_err(|_| FetchError::Rejected("invalid_last_modified".into()))?,
            );
        }
    }
    let response = client
        .execute(PreparedRequest {
            method: Method::GET,
            url: url.clone(),
            headers,
            body: None,
        })
        .await
        .map_err(|error| FetchError::Rejected(error.to_string()))?;
    let not_modified = response.status == http::StatusCode::NOT_MODIFIED;
    if !response.status.is_success() && !not_modified {
        let status = response.status.as_u16();
        if matches!(status, 429 | 503) {
            if let Some(value) = response.headers.get(header::RETRY_AFTER) {
                let value = value
                    .to_str()
                    .map_err(|_| FetchError::Rejected("invalid_retry_after".into()))?;
                return Err(FetchError::RetryAfter {
                    status,
                    not_before: retry_after(value, chrono::Utc::now())?,
                });
            }
        }
        return Err(FetchError::Http(status));
    }
    let content_type = response
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let validators = CacheValidators {
        etag: response
            .headers
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        last_modified: response
            .headers
            .get(header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
    };
    Ok(FetchedPage {
        final_url: response.final_url,
        content_type,
        body: response.body,
        validators,
        not_modified,
    })
}

/// A valid server cooldown is never shortened, including across attempt exhaustion.
/// Reject unrepresentable values rather than overflowing or silently capping them.
fn retry_after(
    value: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<chrono::DateTime<chrono::Utc>, FetchError> {
    let invalid = || FetchError::Rejected("invalid_retry_after".into());
    if value.bytes().all(|v| v.is_ascii_digit()) && !value.is_empty() {
        let seconds = value.parse::<i64>().map_err(|_| invalid())?;
        let delay = chrono::Duration::try_seconds(seconds).ok_or_else(invalid)?;
        now.checked_add_signed(delay).ok_or_else(invalid)
    } else {
        httpdate::parse_http_date(value)
            .map(chrono::DateTime::<chrono::Utc>::from)
            .map_err(|_| invalid())
    }
}

#[cfg(test)]
#[path = "tests/retry_after.rs"]
mod retry_after_tests;
