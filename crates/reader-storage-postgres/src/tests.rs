use crate::ingest_store::{
    article_rule_job, cleanup_job, is_recurring, origin_has_capacity, record_job,
    retry_age_exceeded, PostgresIngestStore,
};
use crate::repository::workspace_feed_url_key;
use crate::schema::SCHEMA_SQL;
use chrono::Duration;
use reader_core::{ArticleId, SourceId, SourceRecordId, SubscriptionId, WorkspaceId};
use reader_ingest::{StoreError, WorkItem};
use sqlx::postgres::PgPoolOptions;

#[test]
fn postgres_instrumentation_has_a_stable_credential_free_target() {
    assert_eq!(crate::POSTGRES_LOG_TARGET, "sqlx::query");
    assert!(!crate::POSTGRES_LOG_TARGET.contains(['@', '/', '?', '=']));
}
use uuid::Uuid;

fn lazy_pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://reader:secret@127.0.0.1:5432/reader")
        .expect("valid test connection string")
}

#[tokio::test]
async fn scheduler_limits_are_validated_before_the_store_exists() {
    assert!(matches!(
        PostgresIngestStore::new(lazy_pool(), Duration::minutes(1), 0, Duration::hours(1)),
        Err(StoreError::Unavailable(_))
    ));
    assert!(matches!(
        PostgresIngestStore::new(lazy_pool(), Duration::minutes(1), 1, Duration::zero()),
        Err(StoreError::Unavailable(_))
    ));
}

#[test]
fn retry_age_and_origin_capacity_use_strict_boundaries() {
    assert!(!retry_age_exceeded(100, 100));
    assert!(retry_age_exceeded(99, 100));
    assert!(origin_has_capacity(1, 2));
    assert!(!origin_has_capacity(2, 2));
}

#[test]
fn only_polling_work_recurs_after_completion() {
    let source_id = SourceId::new();
    assert!(is_recurring(&WorkItem::PollSource { source_id }));
    assert!(is_recurring(&WorkItem::CollectWebFeed { source_id }));
    assert!(!is_recurring(&WorkItem::RefreshSource { source_id }));
}

#[test]
fn durable_job_ids_are_stable_and_include_the_complete_identity() {
    let record = SourceRecordId::new();
    assert_eq!(
        record_job("fanout", record, 7),
        record_job("fanout", record, 7)
    );
    assert_ne!(
        record_job("fanout", record, 7),
        record_job("fanout", record, 8)
    );
    assert_ne!(
        record_job("fanout", record, 7),
        record_job("fulltext", record, 7)
    );

    let refresh = Uuid::new_v4();
    assert_eq!(cleanup_job(record, refresh), cleanup_job(record, refresh));

    let article = ArticleId::new();
    let subscription = SubscriptionId::new();
    assert_ne!(
        article_rule_job(article, subscription, record, 1),
        article_rule_job(article, subscription, record, 2)
    );
}

#[test]
fn schema_has_one_concrete_source_of_truth() {
    assert_eq!(
        SCHEMA_SQL.matches("CREATE TABLE IF NOT EXISTS ").count(),
        34
    );
    for obsolete in [
        "reader_documents",
        "reader_unique_keys",
        "reader_subscription_activity",
        "reader_web_feed_recipes",
        "reader_login_attempts",
    ] {
        assert!(!SCHEMA_SQL.contains(obsolete), "obsolete table {obsolete}");
    }
    for table in [
        "username_reservations",
        "login_attempts",
        "subscription_activity",
        "web_feed_recipes",
        "ingest_jobs",
        "rule_evaluations",
    ] {
        assert!(
            SCHEMA_SQL.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")),
            "missing concrete table {table}"
        );
    }
}

#[test]
fn workspace_feed_url_identity_is_valid_utf8_text_and_keeps_exact_url_spelling() {
    let workspace = WorkspaceId::new();
    let first = workspace_feed_url_key(workspace, "https://example.test/feed?a=1#x");
    let second = workspace_feed_url_key(workspace, "https://example.test/feed?a=1#X");
    assert!(!first.contains('\0'));
    assert!(first.ends_with("https://example.test/feed?a=1#x"));
    assert_ne!(first, second);
}
