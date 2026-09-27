use crate::GlossaryError;
use serde::Deserialize;

/// Operator-supplied limits. No operational object exists until all ranges and
/// deadline/lease relationships have passed GlossaryPolicy::new.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlossaryConfig {
    pub polling_timeout_seconds: u64,

    pub request_timeout_seconds: u64,

    pub connect_timeout_seconds: u64,

    pub lease_seconds: u64,

    pub batch_size: u16,

    pub max_response_bytes: usize,

    pub max_archive_line_bytes: usize,

    pub retry_initial_seconds: u64,

    pub retry_max_seconds: u64,

    pub public_page_interval_milliseconds: u64,

    pub history_reconcile_interval_seconds: u64,

    pub history_pages_per_run: usize,

    pub worker_poll_milliseconds: u64,

    pub workers: usize,
}

#[derive(Clone)]
pub struct GlossaryPolicy(GlossaryConfig);
impl GlossaryPolicy {
    pub fn new(config: GlossaryConfig) -> Result<Self, GlossaryError> {
        if config.polling_timeout_seconds == 0
            || config.polling_timeout_seconds >= config.request_timeout_seconds
            || config.connect_timeout_seconds == 0
            || config.connect_timeout_seconds >= config.request_timeout_seconds
            || config
                .request_timeout_seconds
                .checked_mul(2)
                .is_none_or(|n| config.lease_seconds <= n)
            || config.lease_seconds > i64::MAX as u64
            || config.request_timeout_seconds.checked_mul(1000).is_none()
            || config.batch_size == 0
            || config.batch_size > 100
            || config.max_response_bytes == 0
            || config.max_archive_line_bytes == 0
            || config.retry_initial_seconds == 0
            || config.retry_max_seconds < config.retry_initial_seconds
            || config.retry_max_seconds > i64::MAX as u64
            || config.public_page_interval_milliseconds == 0
            || config.history_pages_per_run == 0
            || config.history_reconcile_interval_seconds == 0
            || config.history_reconcile_interval_seconds > i64::MAX as u64
            || config.worker_poll_milliseconds == 0
            || config.workers == 0
        {
            return Err(GlossaryError::Configuration);
        }
        Ok(Self(config))
    }
    pub fn config(&self) -> &GlossaryConfig {
        &self.0
    }
}

/// Token is a provider credential, never a URL supplied by a caller. Grammar
/// excludes path/query injection. Debug intentionally never includes the value.
pub struct BotToken(String);
impl BotToken {
    pub fn new(value: String) -> Result<Self, GlossaryError> {
        let (id, secret) = value.split_once(':').ok_or(GlossaryError::Identity)?;
        if id.is_empty()
            || !id.bytes().all(|b| b.is_ascii_digit())
            || id.parse::<i64>().ok().is_none_or(|v| v <= 0)
            || secret.is_empty()
            || !secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Err(GlossaryError::Identity);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for BotToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BotToken([redacted])")
    }
}
