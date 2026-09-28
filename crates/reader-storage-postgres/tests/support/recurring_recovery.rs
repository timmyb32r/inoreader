use super::*;

pub(super) async fn verify(pool: &PgPool) {
    let now = Utc::now();
    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        1,
        ChronoDuration::hours(1),
    )
    .unwrap();
    for kind in ["poll", "web", "refresh"] {
        let recurring = kind != "refresh";
        let source_id = SourceId::new();
        let source = SourceDefinition::new(
            source_id,
            url::Url::parse("https://recovery.example/feed").unwrap(),
            SourceKind::XmlFeed,
        )
        .unwrap();
        sqlx::query("INSERT INTO sources(id,revision,document) VALUES($1,0,$2)")
            .bind(source_id.as_uuid().to_string())
            .bind(serde_json::to_string(&source).unwrap())
            .execute(pool)
            .await
            .unwrap();
        let item = if recurring {
            WorkItem::PollSource { source_id }
        } else {
            WorkItem::RefreshSource { source_id }
        };
        let job = JobId::new();
        let token = LeaseToken::new();
        sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',$2,$2,'https://recovery.example',4,$3,0,$4,$5)")
            .bind(job.as_uuid().to_string()).bind(now.timestamp_millis()).bind(serde_json::to_string(&item).unwrap()).bind(token.as_uuid().to_string()).bind((now+ChronoDuration::minutes(1)).timestamp_millis()).execute(pool).await.unwrap();
        assert_eq!(
            store
                .fail(job, LeaseToken::new(), "wrong token", None)
                .await,
            Err(StoreError::StaleLease)
        );
        store
            .fail(
                job,
                token,
                "remote_http_status: 503",
                Some(now + ChronoDuration::minutes(10)),
            )
            .await
            .unwrap();
        let row = sqlx::query("SELECT * FROM ingest_jobs WHERE id=$1")
            .bind(job.as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            row.get::<String, _>("status"),
            if recurring { "ready" } else { "failed" }
        );
        assert_eq!(
            row.get::<String, _>("diagnostic"),
            "remote_http_status: 503"
        );
        if recurring {
            assert_eq!(row.get::<i64, _>("attempt"), 0);
            assert!(
                row.get::<i64, _>("run_at_ms")
                    >= (now + ChronoDuration::minutes(10)).timestamp_millis()
            );
            assert_eq!(
                row.get::<i64, _>("run_at_ms"),
                row.get::<i64, _>("first_attempt_ms")
            );
            assert!(row.get::<Option<String>, _>("lease_token").is_none());
        }
        // Retry-age exhaustion obeys the same periodic/one-shot contract.
        sqlx::query(
            "UPDATE ingest_jobs SET status='ready',run_at_ms=$2,first_attempt_ms=$2 WHERE id=$1",
        )
        .bind(job.as_uuid().to_string())
        .bind((now - ChronoDuration::hours(2)).timestamp_millis())
        .execute(pool)
        .await
        .unwrap();
        let _ = store
            .claim("recovery-test", now, now + ChronoDuration::minutes(1))
            .await
            .unwrap();
        let row =
            sqlx::query("SELECT status,diagnostic,run_at_ms,attempt FROM ingest_jobs WHERE id=$1")
                .bind(job.as_uuid().to_string())
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(
            row.get::<String, _>("status"),
            if recurring { "ready" } else { "failed" }
        );
        assert_eq!(
            row.get::<String, _>("diagnostic"),
            "maximum retry age exceeded"
        );
        if recurring {
            assert_eq!(
                row.get::<i64, _>("run_at_ms"),
                (now + ChronoDuration::minutes(5)).timestamp_millis()
            );
        }
        sqlx::query("DELETE FROM ingest_jobs WHERE id=$1")
            .bind(job.as_uuid().to_string())
            .execute(pool)
            .await
            .unwrap();
    }
}
