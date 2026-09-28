use sqlx::{PgPool, Postgres, Transaction};

mod upgrade;

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

CREATE TABLE IF NOT EXISTS removed_subscriptions (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);
CREATE OR REPLACE VIEW article_subscription_provenance AS
    SELECT id,document,true AS present FROM subscriptions
    UNION ALL SELECT id,document,false AS present FROM removed_subscriptions;

CREATE TABLE IF NOT EXISTS articles (
    id TEXT PRIMARY KEY,
    revision BIGINT NOT NULL CHECK (revision >= 0),
    document TEXT NOT NULL
);
-- Materialized, rebuildable read projection; exact source documents stay TEXT.
ALTER TABLE articles ADD COLUMN IF NOT EXISTS workspace_key TEXT
    GENERATED ALWAYS AS (split_part(id,'/',1)) STORED;
ALTER TABLE articles ADD COLUMN IF NOT EXISTS article_key TEXT
    GENERATED ALWAYS AS (split_part(id,'/',2)) STORED;
ALTER TABLE articles ADD COLUMN IF NOT EXISTS is_read BOOLEAN
    GENERATED ALWAYS AS ((document::jsonb #>> '{state,read}')::boolean) STORED;
ALTER TABLE articles ADD COLUMN IF NOT EXISTS is_later BOOLEAN
    GENERATED ALWAYS AS ((document::jsonb #>> '{state,later}')::boolean) STORED;
-- Ordering uses an exact decimal calendar key including nanoseconds. Timestamp precision alone
-- would collapse distinct source instants; source bytes are never rewritten.
CREATE OR REPLACE FUNCTION reader_arrival_order(value TEXT) RETURNS NUMERIC
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
DECLARE parts TEXT[];
BEGIN
    parts := regexp_match(value, '^([-+]?[0-9]{4,6})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\.([0-9]{1,9}))?Z$');
    IF parts IS NULL THEN
        RAISE EXCEPTION 'unsupported article timestamp representation';
    END IF;
    -- This key retains leap seconds, year zero and signed extended years too;
    -- no cast through PostgreSQL's narrower timestamp representation occurs.
    RETURN (((((parts[1]::numeric*100+parts[2]::numeric)*100+parts[3]::numeric)*100+parts[4]::numeric)*100+parts[5]::numeric)*100+parts[6]::numeric)*1000000000
        + CASE WHEN parts[7] IS NULL THEN 0 ELSE rpad(parts[7],9,'0')::numeric END;
END $$;
ALTER TABLE articles ADD COLUMN IF NOT EXISTS arrival_order NUMERIC
    GENERATED ALWAYS AS (reader_arrival_order(document::jsonb ->> 'first_arrived_at')) STORED;
CREATE INDEX IF NOT EXISTS articles_page ON articles(workspace_key,arrival_order DESC,id DESC);
CREATE INDEX IF NOT EXISTS articles_unread_page ON articles(workspace_key,arrival_order DESC,id DESC) WHERE NOT is_read;
CREATE INDEX IF NOT EXISTS articles_later_page ON articles(workspace_key,arrival_order DESC,id DESC) WHERE is_later;
DROP INDEX IF EXISTS articles_by_workspace_arrival;

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
    PRIMARY KEY (workspace_id, dedup_hash)
);
CREATE TABLE IF NOT EXISTS poll_backlog (
    source_id TEXT NOT NULL,
    record_id TEXT NOT NULL,
    ordinal BIGINT NOT NULL CHECK (ordinal >= 0),
    document TEXT NOT NULL,
    action TEXT NOT NULL,
    context TEXT NOT NULL,
    PRIMARY KEY (source_id, record_id),
    UNIQUE (source_id, ordinal)
);

CREATE TABLE IF NOT EXISTS library_origins (
    workspace_id TEXT NOT NULL,
    article_id TEXT NOT NULL,
    subscription_id TEXT NOT NULL,
    source_record_id TEXT NOT NULL,
    PRIMARY KEY (workspace_id, article_id, subscription_id, source_record_id)
);
-- Associations with immutable AI history survive genuine article regrouping.
-- An ancestor may remain a live sibling after a split; no cross-workspace edges.
CREATE TABLE IF NOT EXISTS article_history_links (
    workspace_id TEXT NOT NULL,
    article_id TEXT NOT NULL,
    history_article_id TEXT NOT NULL,
    PRIMARY KEY (workspace_id, article_id, history_article_id)
);
CREATE INDEX IF NOT EXISTS by_subscription
    ON library_origins (subscription_id, workspace_id);
CREATE INDEX IF NOT EXISTS origins_by_workspace_article
    ON library_origins (workspace_id, article_id);

CREATE TABLE IF NOT EXISTS staged_content_chunks (
    record_id TEXT NOT NULL,
    refresh_id TEXT NOT NULL,
    representation TEXT NOT NULL,
    ordinal BIGINT NOT NULL CHECK (ordinal >= 0),
    bytes BYTEA NOT NULL,
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

CREATE TABLE IF NOT EXISTS subscription_icons (
    subscription_id TEXT PRIMARY KEY,
    data_url TEXT NOT NULL,
    fetched_at_ms BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS source_health (
    source_id TEXT PRIMARY KEY,
    failure_since_ms BIGINT,
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

/// Initialize an empty database explicitly. Existing installations must already
/// match this release; initialization never upgrades an unknown schema.
pub async fn prepare_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema=current_schema() AND table_name='schema_releases')").fetch_one(pool).await?;
    if present {
        return verify_schema(pool).await;
    }
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(492871020)")
        .execute(&mut *transaction)
        .await?;
    let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema=current_schema())").fetch_one(&mut *transaction).await?;
    if occupied {
        return Err(sqlx::Error::Protocol(
            "unversioned database: run explicit upgrade-schema after verified backup".into(),
        ));
    }
    execute_schema(&mut transaction).await?;
    sqlx::raw_sql(RELEASE_TABLE)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("INSERT INTO schema_releases(version,release) VALUES($1,$2)")
        .bind(VERSION)
        .bind(RELEASE)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await
}

pub const VERSION: i64 = 5;
pub const RELEASE: &str = "private-wiki-2026-09-28";
const RELEASE_TABLE: &str = "CREATE TABLE schema_releases(version BIGINT PRIMARY KEY CHECK(version>0),release TEXT NOT NULL,applied_at TIMESTAMPTZ NOT NULL DEFAULT now())";

/// Read-only startup preflight. No listener or worker may start on a different
/// schema. The journal owns deployment identity; physical types detect drift.
pub async fn verify_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema=current_schema() AND table_name='schema_releases')").fetch_one(pool).await?;
    if !exists {
        return Err(sqlx::Error::Protocol("schema version missing: use prepare-schema for an empty database or explicit upgrade-schema for an existing installation".into()));
    }
    let version: Option<(i64, String)> =
        sqlx::query_as("SELECT version,release FROM schema_releases ORDER BY version DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;
    if version != Some((VERSION, RELEASE.into())) {
        return Err(sqlx::Error::Protocol(
            "database schema version does not match application release".into(),
        ));
    }
    let valid: bool = sqlx::query_scalar("SELECT count(*)=4 FROM information_schema.columns WHERE table_schema=current_schema() AND ((table_name='staged_content_chunks' AND column_name='bytes' AND data_type='bytea') OR (table_name='ai_chats' AND column_name IN ('inputs','public_view') AND data_type='text' AND is_nullable='NO') OR (table_name='ai_chats' AND column_name='public_revision' AND data_type='bigint'))").fetch_one(pool).await?;
    let provenance: bool = sqlx::query_scalar("SELECT to_regclass('removed_subscriptions') IS NOT NULL AND to_regclass('article_subscription_provenance') IS NOT NULL").fetch_one(pool).await?;
    let zhihu:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='zhihu_sessions' AND column_name='encrypted' AND data_type='bytea' AND is_nullable='NO')").fetch_one(pool).await?;
    let attention: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='source_health' AND column_name='failure_since_ms' AND data_type='bigint')").fetch_one(pool).await?;
    let wiki: bool = sqlx::query_scalar("SELECT count(*)=9 FROM information_schema.tables WHERE table_schema=current_schema() AND table_name IN ('wiki_namespaces','wiki_members','wiki_pages','wiki_page_names','wiki_revisions','wiki_drafts','wiki_links','wiki_operations','subscription_wiki_links')").fetch_one(pool).await?;
    if !valid || !provenance || !zhihu || !attention || !wiki {
        return Err(sqlx::Error::Protocol(
            "database schema differs from its recorded version".into(),
        ));
    }
    Ok(())
}

/// Explicit offline upgrade from release 4 or the preceding unversioned schema.
/// Release 4 gains private wiki tables without rewriting existing user rows.
/// The unversioned upgrade also validates and converts binary content and AI views.
pub async fn upgrade_schema(pool: &PgPool, batch: std::num::NonZeroU32) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let versioned: bool = sqlx::query_scalar("SELECT to_regclass('schema_releases') IS NOT NULL")
        .fetch_one(&mut *transaction)
        .await?;
    if versioned {
        let previous: (i64, String) = sqlx::query_as(
            "SELECT version,release FROM schema_releases ORDER BY version DESC LIMIT 1 FOR UPDATE",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if previous != (4, "source-attention-2026-09-28".into()) {
            return Err(sqlx::Error::Protocol(
                "upgrade requires the preceding schema release".into(),
            ));
        }
        sqlx::raw_sql(crate::wiki::SCHEMA)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO schema_releases(version,release) VALUES($1,$2)")
            .bind(VERSION)
            .bind(RELEASE)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        return verify_schema(pool).await;
    }
    sqlx::raw_sql(include_str!("../../../tools/upgrade_binary_content.sql"))
        .execute(&mut *transaction)
        .await?;
    upgrade::content_bytes(&mut transaction, batch).await?;
    crate::ai::backfill_public_views(&mut transaction, batch).await?;
    install_subscription_provenance(&mut transaction).await?;
    sqlx::raw_sql(include_str!("schema/attention.sql"))
        .execute(&mut *transaction)
        .await?;
    sqlx::raw_sql(crate::wiki::SCHEMA)
        .execute(&mut *transaction)
        .await?;
    sqlx::raw_sql(crate::zhihu::SCHEMA)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("INSERT INTO schema_releases(version,release) VALUES($1,$2)")
        .bind(VERSION)
        .bind(RELEASE)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    verify_schema(pool).await
}

async fn execute_schema(transaction: &mut Transaction<'_, Postgres>) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(SCHEMA_SQL)
        .execute(&mut **transaction)
        .await?;
    sqlx::query("CREATE UNIQUE INDEX subscriptions_by_workspace_exact_url ON subscriptions ((document::jsonb ->> 'workspace_id'), (document::jsonb ->> 'source_url'))").execute(&mut **transaction).await?;
    sqlx::raw_sql(crate::zhihu::SCHEMA)
        .execute(&mut **transaction)
        .await?;
    sqlx::raw_sql(crate::ai::SCHEMA)
        .execute(&mut **transaction)
        .await?;
    sqlx::raw_sql(crate::wiki::SCHEMA)
        .execute(&mut **transaction)
        .await?;
    sqlx::raw_sql(crate::glossary::schema::SCHEMA)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn install_subscription_provenance(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::raw_sql("CREATE TABLE removed_subscriptions (id TEXT PRIMARY KEY,revision BIGINT NOT NULL CHECK(revision>=0),document TEXT NOT NULL); CREATE VIEW article_subscription_provenance AS SELECT id,document,true AS present FROM subscriptions UNION ALL SELECT id,document,false AS present FROM removed_subscriptions;").execute(&mut **tx).await?;
    Ok(())
}
