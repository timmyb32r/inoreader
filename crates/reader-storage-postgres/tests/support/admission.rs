use super::*;

async fn enqueue(pool: &PgPool, origin: &str, id: JobId) {
    let item = WorkItem::CleanupContent {
        record_id: reader_core::SourceRecordId::new(),
        keep_refresh_id: Uuid::new_v4(),
    };
    sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision) VALUES($1,'ready',0,$2,$3,0,$4,0)")
        .bind(id.as_uuid().to_string()).bind(Utc::now().timestamp_millis()).bind(origin)
        .bind(serde_json::to_string(&item).unwrap()).execute(pool).await.unwrap();
}

pub async fn verify(pool: &PgPool) {
    // Isolate scheduler fixtures from jobs created by previous acceptance cases.
    sqlx::query("DELETE FROM ingest_jobs")
        .execute(pool)
        .await
        .unwrap();
    let store = std::sync::Arc::new(
        PostgresIngestStore::new(
            pool.clone(),
            ChronoDuration::minutes(5),
            2,
            ChronoDuration::days(1),
        )
        .unwrap(),
    );
    for _ in 0..140 {
        enqueue(pool, "https://busy.example", JobId::new()).await;
    }
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.spawn(async move {
            store
                .claim("race", Utc::now(), Utc::now() + ChronoDuration::minutes(5))
                .await
                .unwrap()
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    // A try-lock loser may return no work. Subsequent worker ticks fill capacity.
    for _ in 0..2 {
        store
            .claim("fill", Utc::now(), Utc::now() + ChronoDuration::minutes(5))
            .await
            .unwrap();
    }
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM ingest_jobs WHERE status='leased'")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        active, 2,
        "parallel claims must never oversubscribe an origin"
    );
    let healthy = JobId::new();
    enqueue(pool, "https://healthy.example", healthy).await;
    sqlx::query("UPDATE ingest_jobs SET run_at_ms=1 WHERE id=$1")
        .bind(healthy.as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
    let work = store
        .claim(
            "healthy",
            Utc::now(),
            Utc::now() + ChronoDuration::minutes(5),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        work.job_id, healthy,
        "more than 128 saturated-origin jobs must not hide healthy work"
    );
    sqlx::query("UPDATE ingest_jobs SET lease_deadline_ms=0 WHERE origin_key='https://busy.example' AND status='leased'").execute(pool).await.unwrap();
    assert!(store
        .claim(
            "expired",
            Utc::now(),
            Utc::now() + ChronoDuration::minutes(5)
        )
        .await
        .unwrap()
        .is_some());
    sqlx::query("DELETE FROM ingest_jobs")
        .execute(pool)
        .await
        .unwrap();
}
