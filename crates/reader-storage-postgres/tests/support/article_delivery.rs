use super::*;
use reader_ingest::{DeliveryCommit, DeliveryResult, DeliveryTarget, LeasedWork, SourceRecord};

pub async fn verify(pool: &PgPool) {
    let repository =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20).unwrap();
    let owner = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Delivery fixture".into());
    repository
        .create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse("https://delivery.example/feed").unwrap(),
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
    sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',0,$2,'https://delivery.example',0,$3,0,$4,$5)")
        .bind(lease.job_id.as_uuid().to_string()).bind(Utc::now().timestamp_millis())
        .bind(serde_json::to_string(&item).unwrap()).bind(lease.token.as_uuid().to_string())
        .bind(lease.deadline.timestamp_millis()).execute(pool).await.unwrap();
    let key = DedupKey {
        location: ArticleLocation::from(
            url::Url::parse("https://delivery.example/article").unwrap(),
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

    // In-flight user writes lock the authoritative row. Delivery must observe
    // the committed version, even when its dedup lookup ran before the commit.
    let mut tx = pool.begin().await.unwrap();
    let row_id = format!("{}/{}", workspace.id().as_uuid(), id.as_uuid());
    sqlx::query("SELECT id FROM articles WHERE id=$1 FOR UPDATE")
        .bind(&row_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let deliver = store.deliver(&lease, commit.clone());
    tokio::pin!(deliver);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut deliver)
            .await
            .is_err()
    );
    article.state.later = false;
    article.revision += 1;
    sqlx::query("UPDATE articles SET revision=$2,document=$3 WHERE id=$1")
        .bind(row_id)
        .bind(i64::try_from(article.revision).unwrap())
        .bind(serde_json::to_string(&article).unwrap())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    deliver.await.unwrap();
    assert_eq!(
        repository.article(workspace.id(), id).await.unwrap(),
        article
    );
    let ai_store = reader_storage_postgres::PostgresAiStore::new(
        pool.clone(),
        std::sync::Arc::new(
            PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20).unwrap(),
        ),
    );
    let operation = Uuid::new_v4();
    let chat = super::ai_tests::record(
        owner.id.as_uuid(),
        workspace.id().as_uuid(),
        id.as_uuid(),
        operation,
    );
    let chat_id = chat.view.id;
    reader_ai::AiStore::create_chat(&ai_store, chat, operation, false)
        .await
        .unwrap();
    let mut revision = serde_json::to_value(&commit.record).unwrap();
    revision["key"]["title"] = "Updated title".into();
    revision["key"]["description"] = "Updated description".into();
    revision["revision"] = 1.into();
    let mut edited = commit.clone();
    edited.record = serde_json::from_value(revision.clone()).unwrap();
    edited.proposed_article_id = ArticleId::new();
    store.deliver(&lease, edited.clone()).await.unwrap();
    let updated = repository.article(workspace.id(), id).await.unwrap();
    assert_eq!(updated.key.title, "Updated title");
    assert_eq!(updated.state, article.state);
    assert!(updated.revision > article.revision);
    assert_eq!(
        reader_ai::AiStore::chats(
            &ai_store,
            owner.id.as_uuid(),
            workspace.id().as_uuid(),
            id.as_uuid()
        )
        .await
        .unwrap()[0]
            .view
            .id,
        chat_id,
        "upstream editing retains article ID and saved history"
    );

    // A different existing article becomes the destination of a real merge.
    // Preserve immutable old history through a workspace-scoped association.
    revision["id"] = serde_json::json!(Uuid::new_v4());
    revision["upstream_id"] = "record-2".into();
    revision["key"]["title"] = "Merge destination".into();
    let mut destination = edited.clone();
    destination.record = serde_json::from_value(revision.clone()).unwrap();
    destination.proposed_article_id = ArticleId::new();
    let destination_id = destination.proposed_article_id;
    store.deliver(&lease, destination).await.unwrap();
    revision["id"] = serde_json::json!(commit.record.id());
    revision["upstream_id"] = "record-1".into();
    edited.record = serde_json::from_value(revision).unwrap();
    store.deliver(&lease, edited.clone()).await.unwrap();
    assert_eq!(
        reader_ai::AiStore::chats(
            &ai_store,
            owner.id.as_uuid(),
            workspace.id().as_uuid(),
            destination_id.as_uuid()
        )
        .await
        .unwrap()[0]
            .view
            .id,
        chat_id,
        "merge keeps the original conversation available"
    );
    assert!(reader_ai::AiStore::chats(
        &ai_store,
        AccountId::new().as_uuid(),
        workspace.id().as_uuid(),
        destination_id.as_uuid()
    )
    .await
    .is_err());
    // Split one origin back out of the merged article: both retain lineage and
    // user state, while the saved provider inputs keep their original article ID.
    let merged_state = repository
        .article(workspace.id(), destination_id)
        .await
        .unwrap()
        .state;
    assert_eq!(
        merged_state,
        ArticleState::merge([article.state, ArticleState::default()]).unwrap()
    );
    let mut split_value = serde_json::to_value(&edited.record).unwrap();
    split_value["key"]["title"] = "Split article".into();
    split_value["revision"] = 2.into();
    edited.record = serde_json::from_value(split_value).unwrap();
    edited.proposed_article_id = ArticleId::new();
    let split_id = edited.proposed_article_id;
    store.deliver(&lease, edited).await.unwrap();
    for article_id in [split_id, destination_id] {
        assert_eq!(
            repository
                .article(workspace.id(), article_id)
                .await
                .unwrap()
                .state,
            merged_state
        );
        assert!(reader_ai::AiStore::chats(
            &ai_store,
            owner.id.as_uuid(),
            workspace.id().as_uuid(),
            article_id.as_uuid()
        )
        .await
        .unwrap()
        .iter()
        .any(|chat| chat.view.id == chat_id));
    }
    let immutable = reader_ai::AiStore::chat(&ai_store, owner.id.as_uuid(), chat_id)
        .await
        .unwrap();
    assert_eq!(immutable.view.article_id, id.as_uuid());
    assert_eq!(
        immutable.snapshot.unwrap().source_revision,
        "pinned-version"
    );
    let mut batch = Vec::new();
    for index in 0..4 {
        let mut value = serde_json::to_value(&commit.record).unwrap();
        value["id"] = serde_json::json!(Uuid::new_v4());
        value["upstream_id"] = format!("backlog-{index}").into();
        batch.push(reader_ingest::PolledRecord {
            record: serde_json::from_value(value).unwrap(),
            action: reader_ingest::PollAction::Deliver,
        });
    }
    let first = batch.remove(0);
    store
        .commit_poll(
            &lease,
            reader_ingest::PollCommit {
                source_id: source,
                source_revision: 0,
                records: vec![first],
                remainder: batch.clone(),
                fetched_at: Utc::now(),
                final_url: url::Url::parse("https://delivery.example/feed").unwrap(),
                validators: reader_ingest::CacheValidators {
                    etag: Some("persisted-etag".into()),
                    last_modified: None,
                },
                incomplete: true,
                duration_ms: 1,
            },
        )
        .await
        .unwrap();
    // New store instance models restart: the remainder is persisted, not held by
    // a worker. Conditional requests can now return 304 without losing records.
    let restarted = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        4,
        ChronoDuration::days(1),
    )
    .unwrap();
    for expected in batch {
        let pending = restarted.pending_poll(source, 1).await.unwrap().unwrap();
        assert_eq!(pending.records, vec![expected]);
        assert_eq!(pending.validators.etag.as_deref(), Some("persisted-etag"));
        restarted.commit_poll(&lease, pending).await.unwrap();
    }
    assert!(restarted.pending_poll(source, 1).await.unwrap().is_none());
    let state: String =
        sqlx::query_scalar("SELECT document FROM content_refresh_state WHERE id=$1")
            .bind(format!("source/{}", source.as_uuid()))
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&state).unwrap()["incomplete"],
        false
    );
}
