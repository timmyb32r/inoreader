use std::time::Duration;

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutboundLimits {
    pub(crate) connect_timeout: Duration,
    pub(crate) request_deadline: Duration,
    pub(crate) max_redirect_hops: usize,
    pub(crate) max_response_body_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawOutboundLimits {
    pub connect_timeout_ms: u64,
    pub request_deadline_ms: u64,
    pub max_redirect_hops: usize,
    pub max_response_body_bytes: usize,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LimitsError {
    #[error("{field} must be greater than zero")]
    Zero { field: &'static str },
    #[error("connect timeout must not exceed the overall request deadline")]
    ConnectExceedsDeadline,
    #[error("{field} must be zero")]
    MustBeZero { field: &'static str },
    #[error("browser navigation deadline must not exceed the preview deadline")]
    NavigationExceedsPreview,
}

impl OutboundLimits {
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }
    pub fn request_deadline(&self) -> Duration {
        self.request_deadline
    }
    pub fn max_redirect_hops(&self) -> usize {
        self.max_redirect_hops
    }
    pub fn max_response_body_bytes(&self) -> usize {
        self.max_response_body_bytes
    }
}

impl TryFrom<RawOutboundLimits> for OutboundLimits {
    type Error = LimitsError;

    fn try_from(raw: RawOutboundLimits) -> Result<Self, Self::Error> {
        if raw.connect_timeout_ms == 0 {
            return Err(LimitsError::Zero {
                field: "http.connect_timeout_ms",
            });
        }
        if raw.request_deadline_ms == 0 {
            return Err(LimitsError::Zero {
                field: "http.request_deadline_ms",
            });
        }
        if raw.max_redirect_hops == 0 {
            return Err(LimitsError::Zero {
                field: "http.max_redirect_hops",
            });
        }
        if raw.max_response_body_bytes == 0 {
            return Err(LimitsError::Zero {
                field: "http.max_response_body_bytes",
            });
        }
        if raw.connect_timeout_ms > raw.request_deadline_ms {
            return Err(LimitsError::ConnectExceedsDeadline);
        }
        Ok(Self {
            connect_timeout: Duration::from_millis(raw.connect_timeout_ms),
            request_deadline: Duration::from_millis(raw.request_deadline_ms),
            max_redirect_hops: raw.max_redirect_hops,
            max_response_body_bytes: raw.max_response_body_bytes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserLimits {
    max_contexts: usize,
    max_pages_per_context: usize,
    preview_deadline: Duration,
    navigation_deadline: Duration,
    max_actions: usize,
    max_downloads: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawBrowserLimits {
    pub max_contexts: usize,
    pub max_pages_per_context: usize,
    pub preview_deadline_ms: u64,
    pub navigation_deadline_ms: u64,
    pub max_actions: usize,
    pub max_downloads: usize,
}

impl TryFrom<RawBrowserLimits> for BrowserLimits {
    type Error = LimitsError;

    fn try_from(raw: RawBrowserLimits) -> Result<Self, Self::Error> {
        for (field, value) in [
            ("browser.max_contexts", raw.max_contexts),
            ("browser.max_pages_per_context", raw.max_pages_per_context),
            ("browser.max_actions", raw.max_actions),
        ] {
            if value == 0 {
                return Err(LimitsError::Zero { field });
            }
        }
        if raw.preview_deadline_ms == 0 {
            return Err(LimitsError::Zero {
                field: "browser.preview_deadline_ms",
            });
        }
        if raw.navigation_deadline_ms == 0 {
            return Err(LimitsError::Zero {
                field: "browser.navigation_deadline_ms",
            });
        }
        if raw.navigation_deadline_ms > raw.preview_deadline_ms {
            return Err(LimitsError::NavigationExceedsPreview);
        }
        // Downloads are disabled in v1. Requiring the explicit zero makes a
        // future relaxation a deliberate contract change.
        if raw.max_downloads != 0 {
            return Err(LimitsError::MustBeZero {
                field: "browser.max_downloads",
            });
        }
        Ok(Self {
            max_contexts: raw.max_contexts,
            max_pages_per_context: raw.max_pages_per_context,
            preview_deadline: Duration::from_millis(raw.preview_deadline_ms),
            navigation_deadline: Duration::from_millis(raw.navigation_deadline_ms),
            max_actions: raw.max_actions,
            max_downloads: 0,
        })
    }
}

impl BrowserLimits {
    pub fn max_contexts(&self) -> usize {
        self.max_contexts
    }
    pub fn max_pages_per_context(&self) -> usize {
        self.max_pages_per_context
    }
    pub fn preview_deadline(&self) -> Duration {
        self.preview_deadline
    }
    pub fn navigation_deadline(&self) -> Duration {
        self.navigation_deadline
    }
    pub fn max_actions(&self) -> usize {
        self.max_actions
    }
    pub fn max_downloads(&self) -> usize {
        self.max_downloads
    }
}

#[cfg(test)]
mod tests;
