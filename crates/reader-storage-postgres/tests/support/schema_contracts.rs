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
    sqlx::query("UPDATE schema_releases SET version=1")
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

    let mut connection = pool.acquire().await.unwrap();
    sqlx::raw_sql("CREATE SCHEMA binary_fixture; SET search_path TO binary_fixture; CREATE TABLE ai_chats(inputs TEXT); CREATE TABLE staged_content_chunks(id INTEGER PRIMARY KEY,bytes TEXT);").execute(&mut *connection).await.unwrap();
    let source: Vec<u8> = (0..262144).map(|i| (i % 256) as u8).collect();
    let old = serde_json::to_string(&source).unwrap();
    sqlx::query("INSERT INTO staged_content_chunks VALUES(1,$1),(2,'[]'),(3,'[256]')")
        .bind(&old)
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("BEGIN")
        .execute(&mut *connection)
        .await
        .unwrap();
    assert!(
        sqlx::raw_sql(include_str!("../../../../tools/upgrade_binary_content.sql"))
            .execute(&mut *connection)
            .await
            .is_err()
    );
    sqlx::query("ROLLBACK")
        .execute(&mut *connection)
        .await
        .unwrap();
    let retained: String = sqlx::query_scalar("SELECT bytes FROM staged_content_chunks WHERE id=1")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(
        retained, old,
        "failed conversion rolls back every original byte"
    );
    sqlx::query("DELETE FROM staged_content_chunks WHERE id=3")
        .execute(&mut *connection)
        .await
        .unwrap();
    let start = Instant::now();
    for _ in 0..10 {
        let text: String = sqlx::query_scalar("SELECT bytes FROM staged_content_chunks WHERE id=1")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(serde_json::from_str::<Vec<u8>>(&text).unwrap(), source);
    }
    let json_us = start.elapsed().as_micros();
    sqlx::query("BEGIN")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../../../tools/upgrade_binary_content.sql"))
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("COMMIT")
        .execute(&mut *connection)
        .await
        .unwrap();
    let start = Instant::now();
    for _ in 0..10 {
        let bytes: Vec<u8> = sqlx::query_scalar(
            "SELECT bytes AS binary_bytes FROM staged_content_chunks WHERE id=1",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert_eq!(bytes, source);
    }
    let binary_us = start.elapsed().as_micros();
    let empty: Vec<u8> = sqlx::query_scalar("SELECT bytes FROM staged_content_chunks WHERE id=2")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert!(empty.is_empty());
    eprintln!("content_storage_benchmark reads=10 json_bytes={} binary_bytes={} json_us={json_us} binary_us={binary_us}", old.len(), source.len());
    assert!(old.len() > source.len() * 3);
    sqlx::raw_sql("SET search_path TO public; DROP SCHEMA binary_fixture CASCADE;")
        .execute(&mut *connection)
        .await
        .unwrap();
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
    sqlx::raw_sql("CREATE TABLE staged_content_chunks(id INTEGER PRIMARY KEY,bytes TEXT NOT NULL); CREATE TABLE ai_chats(id UUID PRIMARY KEY,owner UUID,workspace UUID,article UUID,document TEXT NOT NULL,inputs TEXT NOT NULL);")
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
    sqlx::query("INSERT INTO staged_content_chunks VALUES(1,'[0,255,228,184,173]')")
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
