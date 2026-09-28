//! PostgreSQL persistence boundary for the reader application.

mod ai;
mod search;
pub use search::PostgresSearchStore;
mod wiki;
pub use wiki::PostgresWikiStore;
mod content_snapshot;
mod glossary;
mod zhihu;
pub use zhihu::PostgresZhihuStore;
mod ingest_store;
mod publication_backfill;
mod publication_dates;
mod publication_history;
mod repository;
pub mod schema;

pub use ai::PostgresAiStore;
pub use glossary::PostgresGlossaryStore;
pub use ingest_store::PostgresIngestStore;
pub use publication_backfill::backfill_publication_dates;
pub use repository::PostgresRepository;
pub use schema::{prepare_schema, upgrade_schema, verify_schema};

use log::LevelFilter;
use sqlx::{postgres::PgConnectOptions, ConnectOptions};

pub const POSTGRES_LOG_TARGET: &str = "sqlx::query";

/// Enables SQLx's completion log for every PostgreSQL statement.
///
/// SQLx emits the stable `sqlx::query` target with the statement summary,
/// affected/returned row counts, and elapsed time. Bind values, credentials,
/// and connection strings are never included.
pub fn instrument_postgres(options: PgConnectOptions) -> PgConnectOptions {
    options
        .log_statements(LevelFilter::Info)
        .log_slow_statements(LevelFilter::Warn, std::time::Duration::from_secs(1))
}

#[cfg(test)]
mod tests;
