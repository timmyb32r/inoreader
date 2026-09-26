use sqlx::{PgPool, Postgres, Transaction};

/// Current PostgreSQL schema.
///
/// Opaque IDs and serialized documents remain `TEXT`: several repository keys are
/// deliberately not UUIDs, and preserving the serialized JSON bytes avoids an
/// implicit JSONB normalization step. Revisions and unsigned counters use signed
/// PostgreSQL integers with non-negative checks because PostgreSQL has no native
/// unsigned integer types.
pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_metadata (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subscriptions (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS articles (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS outbox (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS content_manifests (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS username_reservations (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invites (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS password_resets (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS login_attempts (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS web_feed_recipes (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sources (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_records (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS delivery_origins (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS content_chunks (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS content_refresh_state (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ingest_jobs (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    run_at_ms BIGINT NOT NULL,
    first_attempt_ms BIGINT NOT NULL,
    origin_key TEXT NOT NULL,
    attempt BIGINT NOT NULL CHECK (attempt >= 0),
    lease_token TEXT,
    lease_deadline_ms BIGINT,
    item TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    diagnostic TEXT
);
CREATE INDEX IF NOT EXISTS ready_jobs
    ON ingest_jobs (status, run_at_ms);
CREATE INDEX IF NOT EXISTS leased_by_origin
    ON ingest_jobs (status, origin_key, lease_deadline_ms);

CREATE TABLE IF NOT EXISTS source_record_identity (
    source_id TEXT NOT NULL,
    upstream_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    observed_at_ms BIGINT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL,
    PRIMARY KEY (source_id, upstream_id)
);
CREATE INDEX IF NOT EXISTS recent_by_source
    ON source_record_identity (source_id, observed_at_ms);

CREATE TABLE IF NOT EXISTS library_dedup (
    workspace_id TEXT NOT NULL,
    dedup_hash TEXT NOT NULL,
    dedup_key TEXT NOT NULL,
    article_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL,
    PRIMARY KEY (workspace_id, dedup_hash)
);

CREATE TABLE IF NOT EXISTS library_origins (
    workspace_id TEXT NOT NULL,
    article_id TEXT NOT NULL,
    subscription_id TEXT NOT NULL,
    source_record_id TEXT NOT NULL,
    PRIMARY KEY (workspace_id, article_id, subscription_id, source_record_id)
);
CREATE INDEX IF NOT EXISTS by_subscription
    ON library_origins (subscription_id, workspace_id);

CREATE TABLE IF NOT EXISTS staged_content_chunks (
    record_id TEXT NOT NULL,
    refresh_id TEXT NOT NULL,
    representation TEXT NOT NULL,
    ordinal BIGINT NOT NULL CHECK (ordinal >= 0),
    bytes TEXT NOT NULL,
    PRIMARY KEY (record_id, refresh_id, representation, ordinal)
);

CREATE TABLE IF NOT EXISTS source_urls (
    url TEXT PRIMARY KEY,
    source_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subscription_sources (
    subscription_id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_health (
    source_id TEXT PRIMARY KEY,
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subscription_activity (
    subscription_id TEXT NOT NULL,
    occurred_at_ms BIGINT NOT NULL,
    id TEXT NOT NULL,
    document TEXT NOT NULL,
    PRIMARY KEY (subscription_id, occurred_at_ms, id)
);
CREATE INDEX IF NOT EXISTS expired_activity
    ON subscription_activity (occurred_at_ms);

CREATE TABLE IF NOT EXISTS source_url_previews (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workspace_feed_urls (
    id TEXT PRIMARY KEY,
    subscription_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS seed_items (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rule_evaluations (
    workspace_id TEXT NOT NULL,
    article_id TEXT NOT NULL,
    rule_id TEXT NOT NULL,
    rule_version BIGINT NOT NULL CHECK (rule_version >= 0),
    document TEXT NOT NULL,
    PRIMARY KEY (workspace_id, article_id, rule_id, rule_version)
);
"#;

/// Creates the complete schema atomically. The DDL is idempotent, so callers
/// may safely invoke this on every process startup.
pub async fn prepare_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    execute_schema(&mut transaction).await?;
    transaction.commit().await
}

async fn execute_schema(transaction: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(SCHEMA_SQL)
        .execute(&mut **transaction)
        .await?;
    migrate_library_dedup_index(transaction).await?;
    Ok(())
}

/// Moves the deduplication index away from the unbounded serialized key. The
/// complete key remains authoritative data and every lookup compares it after
/// using the compact fingerprint; a fingerprint collision therefore fails
/// closed instead of merging unrelated articles.
async fn migrate_library_dedup_index(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query("ALTER TABLE library_dedup ADD COLUMN IF NOT EXISTS dedup_hash TEXT")
        .execute(&mut **transaction)
        .await?;

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, dedup_key FROM library_dedup WHERE dedup_hash IS NULL",
    )
    .fetch_all(&mut **transaction)
    .await?;
    for (workspace_id, dedup_key) in rows {
        sqlx::query(
            "UPDATE library_dedup SET dedup_hash = $1 \
             WHERE workspace_id = $2 AND dedup_key = $3 AND dedup_hash IS NULL",
        )
        .bind(crate::ingest_store::dedup_fingerprint(&dedup_key))
        .bind(workspace_id)
        .bind(dedup_key)
        .execute(&mut **transaction)
        .await?;
    }

    let collision: Option<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, dedup_hash FROM library_dedup \
         GROUP BY workspace_id, dedup_hash HAVING count(DISTINCT dedup_key) > 1 LIMIT 1",
    )
    .fetch_optional(&mut **transaction)
    .await?;
    if let Some((workspace_id, dedup_hash)) = collision {
        return Err(sqlx::Error::Protocol(format!(
            "dedup fingerprint collision in workspace {workspace_id} for {dedup_hash}"
        )));
    }

    sqlx::query("ALTER TABLE library_dedup ALTER COLUMN dedup_hash SET NOT NULL")
        .execute(&mut **transaction)
        .await?;
    let primary_key: Option<String> = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid = 'library_dedup'::regclass AND contype = 'p'",
    )
    .fetch_optional(&mut **transaction)
    .await?;
    if primary_key.as_deref() != Some("PRIMARY KEY (workspace_id, dedup_hash)") {
        sqlx::query("ALTER TABLE library_dedup DROP CONSTRAINT IF EXISTS library_dedup_pkey")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "ALTER TABLE library_dedup ADD CONSTRAINT library_dedup_pkey \
             PRIMARY KEY (workspace_id, dedup_hash)",
        )
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}
