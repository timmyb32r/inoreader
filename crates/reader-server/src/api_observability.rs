use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use std::time::Instant;

pub(super) async fn observe(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let operation = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched")
        .to_owned();
    let started = Instant::now();
    let response = next.run(request).await;
    let completion = Completion {
        method: method.as_str(),
        operation: &operation,
        status: response.status().as_u16(),
        elapsed_ms: started.elapsed().as_millis(),
    };
    log::info!(target: "reader_server::api_request", "{completion}");
    response
}

pub(super) struct Completion<'a> {
    pub(super) method: &'a str,
    pub(super) operation: &'a str,
    pub(super) status: u16,
    pub(super) elapsed_ms: u128,
}

impl std::fmt::Display for Completion<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "api_request method={} operation={} status={} elapsed_ms={}",
            self.method, self.operation, self.status, self.elapsed_ms
        )
    }
}
