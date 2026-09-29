use super::*;

pub async fn verify(pool: &PgPool) {
    for (poll, concurrency, retry) in [(0, 1, 1), (-1, 1, 1), (1, 0, 1), (1, 1, 0), (1, 1, -1)] {
        assert!(reader_storage_postgres::PostgresIngestStore::new(
            pool.clone(),
            ChronoDuration::seconds(poll),
            concurrency,
            ChronoDuration::seconds(retry)
        )
        .is_err());
    }

    reader_storage_postgres::verify_schema(pool).await.unwrap();
    sqlx::query("UPDATE schema_releases SET version=99")
        .execute(pool)
        .await
        .unwrap();
    assert!(reader_storage_postgres::verify_schema(pool).await.is_err());
    assert!(
        prepare_schema(pool).await.is_err(),
        "initialization must not overwrite another release"
    );
    sqlx::query("UPDATE schema_releases SET version=$1")
        .bind(reader_storage_postgres::schema::VERSION)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE ai_chats RENAME COLUMN inputs TO misplaced_inputs")
        .execute(pool)
        .await
        .unwrap();
    assert!(reader_storage_postgres::verify_schema(pool).await.is_err());
    sqlx::query("ALTER TABLE ai_chats RENAME COLUMN misplaced_inputs TO inputs")
        .execute(pool)
        .await
        .unwrap();
    reader_storage_postgres::verify_schema(pool).await.unwrap();

    sqlx::raw_sql("DROP TABLE article_ratings,reading_completions; DELETE FROM schema_releases WHERE version=12; INSERT INTO schema_releases(version,release) VALUES(11,'targeted-ai-review-2026-09-29')")
        .execute(pool).await.unwrap();
    reader_storage_postgres::upgrade_schema(pool, std::num::NonZeroU32::new(50).unwrap())
        .await
        .unwrap();
    reader_storage_postgres::verify_schema(pool).await.unwrap();
    binary_roundtrip(pool).await;
}

/// Exercise the actual native upgrade, including late failure after chunk conversion.
pub async fn upgrade_roundtrip(pool: &PgPool) {
    sqlx::query("CREATE SCHEMA native_upgrade_fixture")
        .execute(pool)
        .await
        .unwrap();
    let isolated = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET search_path TO native_upgrade_fixture")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE library_origins(source_record_id TEXT,workspace_id TEXT,article_id TEXT); CREATE TABLE content_manifests(id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE articles(id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE accounts(id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE source_health(source_id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE subscription_sources(subscription_id TEXT,source_id TEXT); CREATE TABLE subscription_activity(subscription_id TEXT,occurred_at_ms BIGINT,document TEXT); CREATE TABLE subscriptions(id TEXT PRIMARY KEY,revision BIGINT NOT NULL,document TEXT NOT NULL); CREATE TABLE staged_content_chunks(record_id TEXT NOT NULL,refresh_id TEXT NOT NULL,representation TEXT NOT NULL,ordinal BIGINT NOT NULL,bytes TEXT NOT NULL,PRIMARY KEY(record_id,refresh_id,representation,ordinal)); CREATE TABLE ai_chats(id UUID PRIMARY KEY,owner UUID,workspace UUID,article UUID,document TEXT NOT NULL,inputs TEXT NOT NULL);")
        .execute(&isolated).await.unwrap();
    let value = ai_tests::record(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut progress = serde_json::to_value(&value).unwrap();
    let mut inputs = serde_json::Map::new();
    for field in [
        "snapshot",
        "system_prompt",
        "generation_mode",
        "cost_rates",
        "max_output_tokens",
        "limits",
        "review",
    ] {
        inputs.insert(
            field.into(),
            progress.as_object_mut().unwrap().remove(field).unwrap(),
        );
    }
    let document = progress.to_string();
    let inputs = serde_json::to_string_pretty(&inputs).unwrap();
    sqlx::query("INSERT INTO staged_content_chunks VALUES('1','r','raw',0,'[0,255,228,184,173]')")
        .execute(&isolated)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ai_chats VALUES($1,$2,$3,$4,$5,'invalid original')")
        .bind(value.view.id)
        .bind(value.owner)
        .bind(value.view.workspace_id)
        .bind(value.view.article_id)
        .bind(&document)
        .execute(&isolated)
        .await
        .unwrap();
    let batch = std::num::NonZeroU32::new(1).unwrap();
    assert!(reader_storage_postgres::upgrade_schema(&isolated, batch)
        .await
        .is_err());
    let retained: String = sqlx::query_scalar("SELECT bytes FROM staged_content_chunks")
        .fetch_one(&isolated)
        .await
        .unwrap();
    assert_eq!(retained, "[0,255,228,184,173]");
    let absent: bool =
        sqlx::query_scalar("SELECT to_regclass('native_upgrade_fixture.schema_releases') IS NULL")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert!(absent, "failed upgrade must not publish a schema version");
    sqlx::query("UPDATE ai_chats SET inputs=$1")
        .bind(&inputs)
        .execute(&isolated)
        .await
        .unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, batch)
        .await
        .unwrap();
    let (saved_document, saved_inputs, public): (String, String, String) =
        sqlx::query_as("SELECT document,inputs,public_view FROM ai_chats")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert_eq!(saved_document, document);
    assert_eq!(
        saved_inputs, inputs,
        "upgrade preserves exact serialized input bytes"
    );
    let expected = serde_json::to_value(value.into_public_view().unwrap()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&public).unwrap(),
        expected
    );
    let bytes: Vec<u8> =
        sqlx::query_scalar("SELECT bytes AS upgraded_bytes FROM staged_content_chunks")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert_eq!(bytes, [0, 255, 228, 184, 173]);
    assert!(
        reader_storage_postgres::upgrade_schema(&isolated, batch)
            .await
            .is_err(),
        "upgrade cannot be reapplied"
    );
    reader_storage_postgres::verify_schema(&isolated)
        .await
        .unwrap();
    isolated.close().await;
    sqlx::query("DROP SCHEMA native_upgrade_fixture CASCADE")
        .execute(pool)
        .await
        .unwrap();
}

async fn binary_roundtrip(pool: &PgPool) {
    sqlx::query("CREATE SCHEMA binary_fixture")
        .execute(pool)
        .await
        .unwrap();
    let isolated = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET search_path TO binary_fixture")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE library_origins(source_record_id TEXT,workspace_id TEXT,article_id TEXT); CREATE TABLE content_manifests(id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE articles(id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE accounts(id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE source_health(source_id TEXT PRIMARY KEY,document TEXT NOT NULL); CREATE TABLE subscription_sources(subscription_id TEXT,source_id TEXT); CREATE TABLE subscription_activity(subscription_id TEXT,occurred_at_ms BIGINT,document TEXT); CREATE TABLE subscriptions(id TEXT PRIMARY KEY,revision BIGINT NOT NULL,document TEXT NOT NULL); CREATE TABLE staged_content_chunks(record_id TEXT NOT NULL,refresh_id TEXT NOT NULL,representation TEXT NOT NULL,ordinal BIGINT NOT NULL,bytes TEXT NOT NULL,PRIMARY KEY(record_id,refresh_id,representation,ordinal)); CREATE TABLE ai_chats(id UUID PRIMARY KEY,owner UUID,workspace UUID,article UUID,document TEXT NOT NULL,inputs TEXT NOT NULL);")
        .execute(&isolated).await.unwrap();
    let source: Vec<u8> = (0..262144).map(|i| (i % 256) as u8).collect();
    let old = serde_json::to_string(&source).unwrap();
    sqlx::query("INSERT INTO staged_content_chunks VALUES('1','r','raw',0,$1),('2','r','raw',0,'[]'),('3','r','raw',0,'[256]')").bind(&old).execute(&isolated).await.unwrap();
    let batch = std::num::NonZeroU32::new(1).unwrap();
    assert!(reader_storage_postgres::upgrade_schema(&isolated, batch)
        .await
        .is_err());
    let retained: String =
        sqlx::query_scalar("SELECT bytes FROM staged_content_chunks WHERE record_id='1'")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert_eq!(
        retained, old,
        "late invalid chunk rolls back earlier converted rows"
    );
    for invalid in ["[-1]", "[1.5]", "[1e2]", "null", "{}", "[true]", "[0,256]"] {
        sqlx::query("UPDATE staged_content_chunks SET bytes=$1 WHERE record_id='3'")
            .bind(invalid)
            .execute(&isolated)
            .await
            .unwrap();
        assert!(
            reader_storage_postgres::upgrade_schema(&isolated, batch)
                .await
                .is_err(),
            "must reject {invalid}"
        );
    }
    sqlx::query("DELETE FROM staged_content_chunks WHERE record_id='3'")
        .execute(&isolated)
        .await
        .unwrap();
    let start = Instant::now();
    for _ in 0..10 {
        let value: String =
            sqlx::query_scalar("SELECT bytes FROM staged_content_chunks WHERE record_id='1'")
                .fetch_one(&isolated)
                .await
                .unwrap();
        assert_eq!(serde_json::from_str::<Vec<u8>>(&value).unwrap(), source);
    }
    let json_us = start.elapsed().as_micros();
    let start = Instant::now();
    reader_storage_postgres::upgrade_schema(&isolated, batch)
        .await
        .unwrap();
    let upgrade_us = start.elapsed().as_micros();
    let start = Instant::now();
    for _ in 0..10 {
        let value: Vec<u8> = sqlx::query_scalar(
            "SELECT bytes AS binary_bytes FROM staged_content_chunks WHERE record_id='1'",
        )
        .fetch_one(&isolated)
        .await
        .unwrap();
        assert_eq!(value, source);
    }
    let binary_us = start.elapsed().as_micros();
    let empty: Vec<u8> =
        sqlx::query_scalar("SELECT bytes FROM staged_content_chunks WHERE record_id='2'")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert!(empty.is_empty());
    eprintln!("content_storage_benchmark reads=10 json_bytes={} binary_bytes={} json_us={json_us} binary_us={binary_us} native_upgrade_us={upgrade_us}",old.len(),source.len());
    assert!(old.len() > source.len() * 3);
    isolated.close().await;
    sqlx::query("DROP SCHEMA binary_fixture CASCADE")
        .execute(pool)
        .await
        .unwrap();
}

pub async fn subscription_upgrade(pool: &PgPool) {
    sqlx::query("CREATE SCHEMA subscription_upgrade_fixture")
        .execute(pool)
        .await
        .unwrap();
    let isolated = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET search_path TO subscription_upgrade_fixture")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    prepare_schema(&isolated).await.unwrap();
    sqlx::raw_sql("ALTER TABLE source_health DROP COLUMN failure_since_ms; UPDATE schema_releases SET version=5,release='private-wiki-2026-09-28'; INSERT INTO subscriptions VALUES('exact',0,' { \"title\": \"原文\" } ');").execute(&isolated).await.unwrap();
    sqlx::raw_sql("INSERT INTO source_health VALUES('failing','{\"consecutive_failures\":5,\"last_success_ms\":100,\"last_error_ms\":900}'),('healthy','{\"consecutive_failures\":0,\"last_success_ms\":1000,\"last_error_ms\":900}'); INSERT INTO subscription_sources(subscription_id,source_id) VALUES('exact','failing'); INSERT INTO subscription_activity(subscription_id,occurred_at_ms,id,document) VALUES('exact',50,'a','{\"successful\":false}'),('exact',200,'b','{\"successful\":false}'),('exact',900,'c','{\"successful\":false}');").execute(&isolated).await.unwrap();
    sqlx::raw_sql(include_str!("../../src/schema/attention.sql"))
        .execute(&isolated)
        .await
        .unwrap();
    sqlx::raw_sql(
        "DROP TABLE article_ratings,reading_completions; DROP TABLE ai_call_responses; DROP TABLE wiki_favorites,wiki_subscription_roots; ALTER TABLE wiki_pages DROP COLUMN parent; ALTER TABLE wiki_revisions DROP COLUMN parent; DROP TRIGGER ai_new_article ON articles; DROP TRIGGER ai_new_fulltext ON content_manifests; DROP FUNCTION reader_ai_article_candidate(); DROP FUNCTION reader_ai_content_candidate(); DROP TABLE ai_call_models,ai_model_preferences,ai_budget_overrides; DROP TABLE ai_summary_queue,ai_automatic_accounts,ai_spending; ALTER TABLE ai_chats DROP COLUMN priority_at; DROP TABLE article_read_events; DROP TABLE search_content; DROP INDEX search_article_title,search_article_description,search_origins_record",
    )
    .execute(&isolated)
    .await
    .unwrap();
    assert!(reader_storage_postgres::verify_schema(&isolated)
        .await
        .is_err());
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    // Exercise the immediately preceding deployed release independently too.
    sqlx::raw_sql("DROP TABLE article_ratings,reading_completions; DROP TABLE ai_call_responses; DROP TABLE wiki_favorites,wiki_subscription_roots; ALTER TABLE wiki_pages DROP COLUMN parent; ALTER TABLE wiki_revisions DROP COLUMN parent; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(9,'ai-model-preferences-2026-09-29')").execute(&isolated).await.unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    sqlx::raw_sql("DROP TABLE article_ratings,reading_completions; DROP TABLE ai_call_responses; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(10,'wiki-organization-2026-09-29')").execute(&isolated).await.unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    let streaks: Vec<(String, Option<i64>)> =
        sqlx::query_as("SELECT source_id,failure_since_ms FROM source_health ORDER BY source_id")
            .fetch_all(&isolated)
            .await
            .unwrap();
    assert_eq!(
        streaks,
        vec![("failing".into(), Some(200)), ("healthy".into(), None)]
    );
    let exact: String =
        sqlx::query_scalar("SELECT document FROM article_subscription_provenance WHERE id='exact'")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert_eq!(exact, " { \"title\": \"原文\" } ");
    assert!(reader_storage_postgres::upgrade_schema(
        &isolated,
        std::num::NonZeroU32::new(1).unwrap()
    )
    .await
    .is_err());
    // Rehearse the exact deployed 6 -> 7 path without rebuilding search or source rows.
    sqlx::raw_sql("DROP TABLE article_ratings,reading_completions; DROP TABLE ai_call_responses; DROP TABLE wiki_favorites,wiki_subscription_roots; ALTER TABLE wiki_pages DROP COLUMN parent; ALTER TABLE wiki_revisions DROP COLUMN parent; DROP TRIGGER ai_new_article ON articles; DROP TRIGGER ai_new_fulltext ON content_manifests; DROP FUNCTION reader_ai_article_candidate(); DROP FUNCTION reader_ai_content_candidate(); DROP TABLE ai_call_models,ai_model_preferences,ai_budget_overrides; DROP TABLE ai_summary_queue,ai_automatic_accounts,ai_spending; ALTER TABLE ai_chats DROP COLUMN priority_at; DROP TABLE article_read_events; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(6,'unified-search-2026-09-28');")
        .execute(&isolated).await.unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM article_read_events")
        .fetch_one(&isolated)
        .await
        .unwrap();
    assert_eq!(
        events, 0,
        "never invent timestamps for existing read articles"
    );
    let after: String = sqlx::query_scalar("SELECT document FROM subscriptions WHERE id='exact'")
        .fetch_one(&isolated)
        .await
        .unwrap();
    assert_eq!(after, exact);
    sqlx::raw_sql("DROP TABLE article_ratings,reading_completions; DROP TABLE ai_call_responses; DROP TABLE wiki_favorites,wiki_subscription_roots; ALTER TABLE wiki_pages DROP COLUMN parent; ALTER TABLE wiki_revisions DROP COLUMN parent; DROP TRIGGER ai_new_article ON articles; DROP TRIGGER ai_new_fulltext ON content_manifests; DROP FUNCTION reader_ai_article_candidate(); DROP FUNCTION reader_ai_content_candidate(); DROP TABLE ai_call_models,ai_model_preferences,ai_budget_overrides; DROP TABLE ai_summary_queue,ai_automatic_accounts,ai_spending; ALTER TABLE ai_chats DROP COLUMN priority_at; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(7,'daily-read-activity-2026-09-29');").execute(&isolated).await.unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ai_spending")
            .fetch_one(&isolated)
            .await
            .unwrap(),
        0
    );
    sqlx::raw_sql("DROP TABLE article_ratings,reading_completions; DROP TABLE ai_call_responses; DROP TABLE wiki_favorites,wiki_subscription_roots; ALTER TABLE wiki_pages DROP COLUMN parent; ALTER TABLE wiki_revisions DROP COLUMN parent; DROP TABLE ai_call_models,ai_model_preferences,ai_budget_overrides; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(8,'automatic-ai-budget-2026-09-29');").execute(&isolated).await.unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT document FROM subscriptions WHERE id='exact'")
            .fetch_one(&isolated)
            .await
            .unwrap(),
        exact
    );
    isolated.close().await;
    sqlx::query("DROP SCHEMA subscription_upgrade_fixture CASCADE")
        .execute(pool)
        .await
        .unwrap();
}
