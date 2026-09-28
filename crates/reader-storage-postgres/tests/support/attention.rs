use super::*;

pub async fn verify(pool: &PgPool) {
    for seconds in [0, u64::MAX] {
        assert!(PostgresRepository::new(
            pool.clone(),
            ReasonPolicy::new(256).unwrap(),
            20,
            seconds
        )
        .is_err());
    }
    let repo =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let owner = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Attention".into());
    repo.create_account_and_workspace(owner, workspace.clone())
        .await
        .unwrap();
    let sub = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse("https://attention.example/feed").unwrap(),
        "Attention".into(),
    );
    repo.save_subscription(None, sub.clone()).await.unwrap();
    let source: String =
        sqlx::query_scalar("SELECT source_id FROM subscription_sources WHERE subscription_id=$1")
            .bind(sub.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    let source_id = SourceId::from_uuid(Uuid::parse_str(&source).unwrap());
    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(30),
        2,
        ChronoDuration::days(1),
    )
    .unwrap();
    let first = lease(pool, source_id).await;
    store
        .fail(first.job_id, first.token, "timeout", None)
        .await
        .unwrap();
    let since: i64 =
        sqlx::query_scalar("SELECT failure_since_ms FROM source_health WHERE source_id=$1")
            .bind(&source)
            .fetch_one(pool)
            .await
            .unwrap();
    let second = lease(pool, source_id).await;
    store
        .fail(
            second.job_id,
            second.token,
            "different transient error",
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        since,
        sqlx::query_scalar::<_, i64>(
            "SELECT failure_since_ms FROM source_health WHERE source_id=$1"
        )
        .bind(&source)
        .fetch_one(pool)
        .await
        .unwrap()
    );
    // Fast repeated attempts never replace an elapsed-time threshold.
    for (elapsed, expected) in [
        (100, false),
        (86_399_999, false),
        (86_400_000, true),
        (86_400_001, true),
    ] {
        sqlx::query("UPDATE source_health SET document=jsonb_set(jsonb_set(document::jsonb,'{last_error_ms}',to_jsonb($2::bigint)),'{consecutive_failures}','100')::text WHERE source_id=$1")
            .bind(&source).bind(since+elapsed).execute(pool).await.unwrap();
        let reopened =
            PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400)
                .unwrap();
        let stats = reopened
            .subscription_stats(workspace.id(), std::slice::from_ref(&sub))
            .await
            .unwrap();
        assert_eq!(stats[&sub.id()].needs_attention, expected);
        assert!(
            stats[&sub.id()].error.is_some(),
            "diagnostic must remain visible during grace period"
        );
    }
    let success = lease(pool, source_id).await;
    store
        .record_source_success(&success, source_id, Utc::now(), true, 1)
        .await
        .unwrap();
    let cleared: Option<i64> =
        sqlx::query_scalar("SELECT failure_since_ms FROM source_health WHERE source_id=$1")
            .bind(&source)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(cleared, None);
    let stats = repo
        .subscription_stats(workspace.id(), std::slice::from_ref(&sub))
        .await
        .unwrap();
    assert!(!stats[&sub.id()].needs_attention);
    assert!(
        stats[&sub.id()].incomplete,
        "queued continuation is progress, not a failed attempt"
    );
    let new_failure = lease(pool, source_id).await;
    store
        .fail(new_failure.job_id, new_failure.token, "new outage", None)
        .await
        .unwrap();
    let stats = repo
        .subscription_stats(workspace.id(), std::slice::from_ref(&sub))
        .await
        .unwrap();
    assert!(!stats[&sub.id()].needs_attention);
    assert_eq!(stats[&sub.id()].consecutive_failures, 1);
    // Keep unrelated queue-admission fixtures isolated from these manually leased jobs.
    sqlx::query("DELETE FROM ingest_jobs WHERE origin_key='https://attention.example'")
        .execute(pool)
        .await
        .unwrap();
}
async fn lease(pool: &PgPool, source_id: SourceId) -> reader_ingest::LeasedWork {
    let lease = reader_ingest::LeasedWork {
        job_id: JobId::new(),
        item: WorkItem::RefreshSource { source_id },
        token: LeaseToken::new(),
        deadline: Utc::now() + ChronoDuration::minutes(5),
        attempt: 0,
    };
    sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',0,$2,'https://attention.example',0,$3,0,$4,$5)")
        .bind(lease.job_id.as_uuid().to_string()).bind(Utc::now().timestamp_millis()).bind(serde_json::to_string(&lease.item).unwrap()).bind(lease.token.as_uuid().to_string()).bind(lease.deadline.timestamp_millis()).execute(pool).await.unwrap();
    lease
}
