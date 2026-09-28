use super::*;
use reader_application::{SearchKind, SearchLimits, SearchPort, SearchRequest};
use reader_core::SourceRecordId;
use reader_ingest::{ContentChunk, ContentRevision, LeasedWork};
pub async fn verify(pool: &PgPool, owner: Uuid, workspace: WorkspaceId, limits: &SearchLimits) {
    let record = SourceRecordId::new();
    let url = url::Url::parse("https://search.example/full").unwrap();
    let repo =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace,
        url.clone(),
        "Search content".into(),
    );
    repo.save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let source: String =
        sqlx::query_scalar("SELECT source_id FROM subscription_sources WHERE subscription_id=$1")
            .bind(subscription.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    let document = serde_json::json!({"id":record,"source_id":source,"upstream_id":"search-content","key":{"location":ArticleLocation::from(url.clone()),"title":"Full content","description":null},"feed_content_html":null,"published_at":null,"revision":0});
    sqlx::query("INSERT INTO source_records(id,revision,document) VALUES($1,0,$2)")
        .bind(record.as_uuid().to_string())
        .bind(document.to_string())
        .execute(pool)
        .await
        .unwrap();
    let item = WorkItem::ExtractFullText {
        record_id: record,
        source_revision: 0,
        url: url.clone(),
        manual: false,
    };
    let lease = LeasedWork {
        job_id: JobId::new(),
        item: item.clone(),
        token: LeaseToken::new(),
        deadline: Utc::now() + ChronoDuration::minutes(10),
        attempt: 0,
    };
    sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',0,$2,'https://search.example',0,$3,0,$4,$5)").bind(lease.job_id.as_uuid().to_string()).bind(Utc::now().timestamp_millis()).bind(serde_json::to_string(&item).unwrap()).bind(lease.token.as_uuid().to_string()).bind(lease.deadline.timestamp_millis()).execute(pool).await.unwrap();
    let article: String =
        sqlx::query_scalar("SELECT article_key FROM articles WHERE workspace_key=$1 LIMIT 1")
            .bind(workspace.as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO library_origins VALUES($1,$2,$3,$4)")
        .bind(workspace.as_uuid().to_string())
        .bind(article)
        .bind(subscription.id().as_uuid().to_string())
        .bind(record.as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        4,
        ChronoDuration::days(1),
    )
    .unwrap();
    let search = reader_storage_postgres::PostgresSearchStore::new(pool.clone(), limits.clone());
    let query = |text: &str| {
        SearchRequest::new(limits, text.into(), SearchKind::News, None, None, 0).unwrap()
    };
    let mut revision = ContentRevision {
        publication: vec![],
        record_id: record,
        source_revision: 0,
        refresh_id: Uuid::new_v4(),
        raw_chunks: vec![],
        safe_html_chunks: vec![ContentChunk {
            ordinal: 0,
            bytes: "<p>FulltextUnique 原文</p>".as_bytes().to_vec(),
        }],
        fetched_at: Utc::now(),
        final_url: url,
    };
    store
        .publish_content(&lease, revision.clone())
        .await
        .unwrap();
    assert_eq!(
        search
            .search(owner, query("FulltextUnique"))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(search
        .search(Uuid::new_v4(), query("FulltextUnique"))
        .await
        .unwrap()
        .items
        .is_empty());
    let original: Vec<u8> = sqlx::query_scalar(
        "SELECT bytes FROM staged_content_chunks WHERE record_id=$1 AND refresh_id=$2",
    )
    .bind(record.as_uuid().to_string())
    .bind(revision.refresh_id.to_string())
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(original, revision.safe_html_chunks[0].bytes);
    revision.refresh_id = Uuid::new_v4();
    revision.fetched_at += ChronoDuration::seconds(1);
    revision.safe_html_chunks[0].bytes = b"<p>ReplacementUnique</p>".to_vec();
    store
        .publish_content(&lease, revision.clone())
        .await
        .unwrap();
    assert!(search
        .search(owner, query("FulltextUnique"))
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        search
            .search(owner, query("ReplacementUnique"))
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    revision.refresh_id = Uuid::new_v4();
    revision.fetched_at += ChronoDuration::seconds(1);
    revision.safe_html_chunks[0].bytes = vec![255];
    assert!(store.publish_content(&lease, revision).await.is_err());
    assert_eq!(
        search
            .search(owner, query("ReplacementUnique"))
            .await
            .unwrap()
            .items
            .len(),
        1,
        "invalid content rolls back projection and source publication together"
    );
}
