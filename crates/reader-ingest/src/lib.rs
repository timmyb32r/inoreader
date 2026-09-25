//! Durable ingestion orchestration.
//!
//! This crate owns no database client and no HTTP implementation. Network reads
//! cross [`FeedFetcher`]; every durable transition crosses [`IngestStore`]. The
//! latter deliberately exposes semantic atomic operations instead of generic
//! queries, so a YDB adapter can enforce idempotency and fencing in one
//! transaction. There is no production in-memory fallback.

mod built_in_adapters;
mod cdp_browser;
mod engine;
mod model;
mod ports;
mod runtime;
mod secure_fetch;
mod web_feed;

pub use built_in_adapters::*;
pub use cdp_browser::*;
pub use engine::*;
pub use model::*;
pub use ports::*;
pub use runtime::*;
pub use secure_fetch::*;
pub use web_feed::*;

#[cfg(test)]
mod tests;
