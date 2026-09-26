//! PostgreSQL persistence boundary for the reader application.

mod ingest_store;
mod repository;
pub mod schema;

pub use ingest_store::PostgresIngestStore;
pub use repository::PostgresRepository;
pub use schema::prepare_schema;

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
