use super::*;

pub async fn verify(pool: &PgPool) {
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2026-12-31',holidays=ARRAY['2026-10-01'::date]")
        .execute(pool).await.unwrap();
    for (time, resume) in [
        ("2026-10-08T00:59:59Z", None),
        ("2026-10-08T01:00:00Z", Some("2026-10-08T04:00:00Z")),
        ("2026-10-08T03:59:59Z", Some("2026-10-08T04:00:00Z")),
        ("2026-10-08T04:00:00Z", None),
        ("2026-10-08T05:59:59Z", None),
        ("2026-10-08T06:00:00Z", Some("2026-10-08T10:00:00Z")),
        ("2026-10-08T09:59:59Z", Some("2026-10-08T10:00:00Z")),
        ("2026-10-08T10:00:00Z", None),
        ("2026-10-10T01:00:00Z", None), // Saturday, even a Chinese makeup workday.
        ("2026-10-11T06:00:00Z", None),
        ("2026-10-01T01:00:00Z", None), // Official holiday.
        ("2027-01-01T01:00:00Z", Some("2027-01-01T16:00:00Z")), // Expired calendar.
    ] {
        let time = time.parse::<chrono::DateTime<Utc>>().unwrap();
        let actual: Option<chrono::DateTime<Utc>> =
            sqlx::query_scalar("SELECT reader_ai_automatic_resume_at($1)")
                .bind(time)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(
            actual,
            resume.map(|s| s.parse::<chrono::DateTime<Utc>>().unwrap()),
            "{time}"
        );
    }
    for (peak, time, expected) in [
        (true, "2026-10-08T02:00:00Z", "2026-10-08T04:00:00Z"),
        (true, "2026-10-08T04:00:00Z", "2026-10-08T04:00:00Z"),
        (true, "2026-10-08T10:00:00Z", "2026-10-08T10:00:00Z"),
        (false, "2026-10-08T04:00:00Z", "2026-10-08T21:00:00Z"),
    ] {
        let actual: chrono::DateTime<Utc> =
            sqlx::query_scalar("SELECT reader_ai_deferred_until($1,$2)")
                .bind(peak)
                .bind(time.parse::<chrono::DateTime<Utc>>().unwrap())
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(actual, expected.parse::<chrono::DateTime<Utc>>().unwrap());
    }
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    verify_upgrade(pool).await;
}

async fn verify_upgrade(pool: &PgPool) {
    sqlx::query("CREATE SCHEMA off_peak_upgrade_fixture")
        .execute(pool)
        .await
        .unwrap();
    let isolated = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET search_path TO off_peak_upgrade_fixture")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    prepare_schema(&isolated).await.unwrap();
    // Upgrade the deployed v18 without rewriting retained documents.
    sqlx::raw_sql("DROP FUNCTION reader_ai_terms_superseded(UUID); DROP INDEX ai_definitions_article_prompt; DROP INDEX ai_terms_failures_article_prompt; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(18,'automatic-ai-off-peak-2026-10-08')").execute(&isolated).await.unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    reader_storage_postgres::verify_schema(&isolated)
        .await
        .unwrap();

    sqlx::raw_sql("INSERT INTO article_read_events(workspace_id,article_id,revision,method) VALUES('w','a',1,'single'); DROP FUNCTION reader_ai_deferred_until(BOOLEAN,TIMESTAMPTZ); DROP FUNCTION reader_ai_terms_superseded(UUID); DROP INDEX ai_definitions_article_prompt; DROP INDEX ai_terms_failures_article_prompt; DROP FUNCTION reader_ai_automatic_resume_at(TIMESTAMPTZ); DROP TABLE ai_automatic_schedule; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(17,'reading-session-provenance-2026-10-06')").execute(&isolated).await.unwrap();
    let before: String =
        sqlx::query_scalar("SELECT row_to_json(e)::text FROM article_read_events e")
            .fetch_one(&isolated)
            .await
            .unwrap();
    reader_storage_postgres::upgrade_schema(&isolated, std::num::NonZeroU32::new(1).unwrap())
        .await
        .unwrap();
    let after: String =
        sqlx::query_scalar("SELECT row_to_json(e)::text FROM article_read_events e")
            .fetch_one(&isolated)
            .await
            .unwrap();
    assert_eq!(before, after);
    reader_storage_postgres::schema::verify_schema(&isolated)
        .await
        .unwrap();
    isolated.close().await;
    sqlx::query("DROP SCHEMA off_peak_upgrade_fixture CASCADE")
        .execute(pool)
        .await
        .unwrap();
}
