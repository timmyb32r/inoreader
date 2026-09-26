//! PostgreSQL persistence boundary for the reader application.

mod ingest_store;
mod repository;
pub mod schema;

pub use ingest_store::PostgresIngestStore;
pub use repository::PostgresRepository;
pub use schema::prepare_schema;

#[cfg(test)]
mod tests;
