use super::*;
use reader_ingest::{DeliveryCommit, DeliveryResult, DeliveryTarget, LeasedWork, SourceRecord};

pub async fn verify(pool: &PgPool) {
    let repository =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let owner = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Deletion fixture".into());
    repository
        .create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse("https://deletion.example/feed").unwrap(),
        "Delivery".into(),
    );
    repository
        .save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let source: String =
        sqlx::query_scalar("SELECT source_id FROM subscription_sources WHERE subscription_id=$1")
            .bind(subscription.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    let source = SourceId::from_uuid(Uuid::parse_str(&source).unwrap());
    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        4,
        ChronoDuration::days(1),
    )
    .unwrap();
    let item = WorkItem::RefreshSource { source_id: source };
    let lease = LeasedWork {
        job_id: JobId::new(),
        item: item.clone(),
        token: LeaseToken::new(),
        deadline: Utc::now() + ChronoDuration::minutes(10),
        attempt: 0,
    };
    sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',0,$2,'https://deletion.example',0,$3,0,$4,$5)")
        .bind(lease.job_id.as_uuid().to_string()).bind(Utc::now().timestamp_millis())
        .bind(serde_json::to_string(&item).unwrap()).bind(lease.token.as_uuid().to_string())
        .bind(lease.deadline.timestamp_millis()).execute(pool).await.unwrap();
    let key = DedupKey {
        location: ArticleLocation::from(
            url::Url::parse("https://deletion.example/article").unwrap(),
        ),
        title: "Retained article".into(),
        description: Some("Original description".into()),
    };
    let record: SourceRecord = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "source_id": source, "upstream_id": "record-1",
        "key": key, "feed_content_html": null, "published_at": null, "revision": 0,
    }))
    .unwrap();
    let id = ArticleId::new();
    let commit = DeliveryCommit {
        target: DeliveryTarget {
            workspace_id: workspace.id(),
            subscription_id: subscription.id(),
        },
        record,
        proposed_article_id: id,
        delivered_at: Utc::now(),
    };
    assert_eq!(
        store.deliver(&lease, commit.clone()).await.unwrap(),
        DeliveryResult::Delivered(id)
    );
    let mut article = repository.article(workspace.id(), id).await.unwrap();
    let expected = article.revision;
    article.revision += 1;
    article.state = ArticleState {
        read: true,
        later: true,
        protect_unread: true,
    };
    repository
        .save_article(workspace.id(), Some(expected), article.clone())
        .await
        .unwrap();
    assert_eq!(
        store.deliver(&lease, commit.clone()).await.unwrap(),
        DeliveryResult::AlreadyDelivered(id)
    );
    assert_eq!(
        repository.article(workspace.id(), id).await.unwrap(),
        article,
        "redelivery must not overwrite user state/revision from a stale dedup snapshot"
    );

    let refresh = Uuid::new_v4();
    let pointer = reader_ingest::ContentManifestPointer {
        publication: None,
        record_id: commit.record.id(),
        source_revision: 0,
        refresh_id: refresh,
        raw_chunks: 1,
        safe_html_chunks: 1,
        fetched_at: Utc::now(),
        final_url: url::Url::parse("https://deletion.example/article").unwrap(),
    };
    sqlx::query("INSERT INTO content_manifests(id,revision,document) VALUES($1,0,$2)")
        .bind(commit.record.id().as_uuid().to_string())
        .bind(serde_json::to_string(&pointer).unwrap())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staged_content_chunks(record_id,refresh_id,representation,ordinal,bytes) VALUES($1,$2,'safe',0,$3)")
        .bind(commit.record.id().as_uuid().to_string()).bind(refresh.to_string()).bind(b"<p>Retained full text</p>".as_slice()).execute(pool).await.unwrap();
    let before = repository
        .article_presentation(workspace.id(), id)
        .await
        .unwrap();
    assert_eq!(
        before.safe_html.as_deref(),
        Some("<p>Retained full text</p>")
    );
    let exact: String = sqlx::query_scalar("SELECT document FROM subscriptions WHERE id=$1")
        .bind(subscription.id().as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    let mut stale = subscription.clone();
    stale.rename("Wrong revision".into());
    assert!(matches!(
        repository.delete_subscription(&stale).await,
        Err(RepositoryError::Conflict)
    ));
    assert_eq!(
        repository.subscription(subscription.id()).await.unwrap(),
        subscription
    );
    repository.delete_subscription(&subscription).await.unwrap();
    assert!(matches!(
        repository.subscription(subscription.id()).await,
        Err(RepositoryError::NotFound)
    ));
    assert!(repository
        .subscriptions_by_workspace(workspace.id())
        .await
        .unwrap()
        .is_empty());
    let retained: String =
        sqlx::query_scalar("SELECT document FROM removed_subscriptions WHERE id=$1")
            .bind(subscription.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(exact, retained);
    let after = repository
        .article_presentation(workspace.id(), id)
        .await
        .unwrap();
    assert_eq!(after.article, before.article);
    assert_eq!(after.safe_html, before.safe_html);
    assert_eq!(after.subscription_titles, before.subscription_titles);
    assert_eq!(after.publication, before.publication);
    assert!(after.subscription_ids.is_empty());
    assert_eq!(
        store.deliver(&lease, commit).await.unwrap(),
        DeliveryResult::SkippedInactive
    );
    let replacement = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        subscription.source_url().clone(),
        "Re-added".into(),
    );
    repository
        .save_subscription(None, replacement)
        .await
        .unwrap();
    assert_eq!(
        repository.article(workspace.id(), id).await.unwrap(),
        article
    );
}
