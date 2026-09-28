use super::*;

async fn jobs(pool: &PgPool, subscription: &Subscription) -> Vec<(String, String)> {
    sqlx::query_as("SELECT j.id,j.status FROM ingest_jobs j JOIN subscription_sources ss ON ss.source_id=j.item::jsonb#>>'{RefreshSource,source_id}' WHERE ss.subscription_id=$1 ORDER BY j.id")
        .bind(subscription.id().as_uuid().to_string()).fetch_all(pool).await.unwrap()
}

pub async fn verify(pool: &PgPool, repo: &PostgresRepository, a: &Subscription, b: &Subscription) {
    let (first, concurrent) = tokio::join!(
        repo.enqueue_subscription_refresh(a),
        repo.enqueue_subscription_refresh(a)
    );
    first.unwrap();
    concurrent.unwrap();
    let initial = jobs(pool, a).await;
    assert_eq!(
        initial.len(),
        1,
        "concurrent activation shares one pending refresh"
    );
    assert_eq!(initial[0].1, "ready");
    assert_eq!(
        jobs(pool, b).await,
        initial,
        "public fetch work is shared below the user-data boundary"
    );
    assert_eq!(
        repo.subscription(b.id()).await.unwrap(),
        *b,
        "refresh leaves the other owner's subscription unchanged"
    );

    sqlx::query("UPDATE ingest_jobs SET status='leased',lease_token=$2,lease_deadline_ms=9999999999999,revision=revision+1 WHERE id=$1")
        .bind(&initial[0].0).bind(Uuid::new_v4().to_string()).execute(pool).await.unwrap();
    repo.enqueue_subscription_refresh(a).await.unwrap();
    assert_eq!(jobs(pool, a).await.len(), 1, "in-flight request is reused");
    sqlx::query("UPDATE ingest_jobs SET status='completed',lease_token=NULL,lease_deadline_ms=NULL,revision=revision+1 WHERE id=$1")
        .bind(&initial[0].0).execute(pool).await.unwrap();
    repo.enqueue_subscription_refresh(a).await.unwrap();
    let second = jobs(pool, a).await;
    assert_eq!(
        second.len(),
        2,
        "same unchanged subscription can refresh again"
    );
    let new = second.iter().find(|(_, status)| status == "ready").unwrap();
    assert_ne!(new.0, initial[0].0);
    sqlx::query("UPDATE ingest_jobs SET status='failed',revision=revision+1 WHERE id=$1")
        .bind(&new.0)
        .execute(pool)
        .await
        .unwrap();
    repo.enqueue_subscription_refresh(a).await.unwrap();
    assert_eq!(
        jobs(pool, a).await.len(),
        3,
        "failed history does not suppress a retry click"
    );

    let forged = Subscription::new(
        a.id(),
        b.workspace_id(),
        a.source_url().clone(),
        "Wrong workspace".into(),
    );
    assert!(matches!(
        repo.enqueue_subscription_refresh(&forged).await,
        Err(RepositoryError::Conflict)
    ));
    repo.enqueue_subscription_refresh(b).await.unwrap();
    assert_eq!(
        jobs(pool, b).await.len(),
        3,
        "same public source reuses pending fetch work without mutating either subscription"
    );
    assert_eq!(repo.subscription(b.id()).await.unwrap(), *b);
    assert_eq!(jobs(pool, a).await.len(), 3);

    // The fixture deliberately shares a public fetch identity across two
    // owners. Source locking must serialize concurrent refreshes through their
    // distinct subscription locks; their user-visible records remain separate.
    sqlx::query("UPDATE ingest_jobs SET status='completed',revision=revision+1 WHERE status='ready' AND item::jsonb#>>'{RefreshSource,source_id}'=(SELECT source_id FROM subscription_sources WHERE subscription_id=$1)")
        .bind(a.id().as_uuid().to_string()).execute(pool).await.unwrap();
    let (left, right) = tokio::join!(
        repo.enqueue_subscription_refresh(a),
        repo.enqueue_subscription_refresh(b)
    );
    left.unwrap();
    right.unwrap();
    assert_eq!(
        jobs(pool, a)
            .await
            .iter()
            .filter(|(_, status)| status == "ready")
            .count(),
        1
    );
    assert_eq!(jobs(pool, a).await, jobs(pool, b).await);
    assert_eq!(repo.subscription(a.id()).await.unwrap(), *a);
    assert_eq!(repo.subscription(b.id()).await.unwrap(), *b);
}
