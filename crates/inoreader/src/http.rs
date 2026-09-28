//! Http composition responsibilities.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct RequestObserver {
    pub(super) format: LogFormat,
}
impl ExternalRequestObserver for RequestObserver {
    fn completed(&self, value: ExternalRequestCompletion) {
        log::info!(target:"reader_external", "{}", format_external_request_completion(self.format, &value))
    }
}

pub(super) type ProductionFetcher =
    SecureWebFetcher<TokioDnsResolver, ProxyTransport, RequestObserver>;
