use reader_application::WorkspaceRepository;
use reader_core::{AccountId, ReasonPolicy, Workspace, WorkspaceId};
use reader_glossary::*;
use reader_storage_postgres::{PostgresGlossaryStore, PostgresRepository};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn archived(id: i64, term: &str, body: &str) -> ObservedPost {
    ObservedPost::archive(json!({"channel":CHANNEL_USERNAME,"id":id,"permalink":format!("https://t.me/{CHANNEL_USERNAME}/{id}"),"timestamp":null,"formatted_html":format!("<b>{term}</b> — {body}"),"extra":{"preserved":true}}).to_string()).unwrap()
}
fn batch(update: i64, message: i64, text: &str, edit: Option<i64>) -> UpdateBatch {
    let mut post = json!({"message_id":message,"date":100,"chat":{"id":-100777,"type":"channel"},"text":text,"entities":[{"type":"bold","offset":0,"length":3}]});
    if let Some(edit) = edit {
        post["edit_date"] = json!(edit);
    }
    let kind = if edit.is_some() {
        "edited_channel_post"
    } else {
        "channel_post"
    };
    UpdateBatch::new(json!({"ok":true,"result":[{"update_id":update,kind:post}]}).to_string())
        .unwrap()
}

pub async fn verify(pool: &PgPool) {
    let reader =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(4096).unwrap(), 100).unwrap();
    let owner = AccountId::new();
    let other = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "Glossary".into());
    let foreign = Workspace::new(WorkspaceId::new(), other, "Other glossary".into());
    reader
        .save_workspace(None, workspace.clone())
        .await
        .unwrap();
    reader.save_workspace(None, foreign.clone()).await.unwrap();
    let owner = owner.as_uuid();
    let other = other.as_uuid();
    let ws = workspace.id().as_uuid();
    let store = PostgresGlossaryStore::new(pool.clone());
    assert!(!store.status(owner, ws).await.unwrap().index_ready);
    assert_eq!(
        store.status(other, ws).await.unwrap_err(),
        GlossaryError::NotFound
    );
    store
        .begin_archive(owner, ws, "{\"post_count\":2,\"missing\":[7]}")
        .await
        .unwrap();
    let first = archived(1, "CDC", "первое");
    store.import_post(owner, ws, &first).await.unwrap();
    store.import_post(owner, ws, &first).await.unwrap();
    store
        .import_post(owner, ws, &archived(2, "CDC", "последнее"))
        .await
        .unwrap();
    store.finish_archive(owner, ws, 2).await.unwrap();
    let status = store.status(owner, ws).await.unwrap();
    assert!(status.index_ready && status.history_incomplete);
    assert_eq!(status.posts, 2);
    let found = store
        .lookup(
            owner,
            ws,
            &["CDC".into(), "cdc".into(), "Change Data Capture".into()],
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].permalink, "https://t.me/reading_data_news/2");
    assert_eq!(found[0].definition.paragraph().text(), "CDC — последнее");
    assert!(store.lookup(other, ws, &["CDC".into()]).await.is_err());
    assert!(store.request_sync(other, ws).await.is_err());
    let binding = ChannelBinding::new(777, "glossary_bot".into(), -100777).unwrap();
    assert!(store.configure(other, ws, &binding, vec![1]).await.is_err());
    store
        .configure(owner, ws, &binding, vec![1, 2, 3])
        .await
        .unwrap();
    assert_eq!(
        store
            .configure(other, foreign.id().as_uuid(), &binding, vec![4])
            .await
            .unwrap_err(),
        GlossaryError::ReceiverConflict
    );
    let claim = store.claim_poll(120).await.unwrap().unwrap();
    assert!(store.claim_poll(120).await.unwrap().is_none());
    assert!(claim.offset.is_none());
    let event = batch(10, 3, "CDC — новое", None);
    store.commit_batch(&claim, &event).await.unwrap();
    let cursor: Option<i64> = sqlx::query_scalar(
        "SELECT poll_cursor FROM glossary_channels WHERE owner=$1 AND workspace=$2",
    )
    .bind(owner)
    .bind(ws)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(cursor, Some(11));
    let preserved: String = sqlx::query_scalar("SELECT raw FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='bot_batch' ORDER BY received_at LIMIT 1").bind(owner).bind(ws).fetch_one(pool).await.unwrap();
    assert_eq!(preserved, event.raw());
    assert_eq!(
        store.status(owner, ws).await.unwrap().posts,
        2,
        "raw commit precedes independent indexing"
    );
    store.finish_poll(&claim, None, 0).await.unwrap();
    assert_eq!(store.apply_pending(100).await.unwrap(), 1);
    assert_eq!(store.status(owner, ws).await.unwrap().posts, 3);
    let replay = store.claim_poll(120).await.unwrap().unwrap();
    store.commit_batch(&replay, &event).await.unwrap();
    store.finish_poll(&replay, None, 0).await.unwrap();
    assert_eq!(store.apply_pending(100).await.unwrap(), 0);
    let edit = store.claim_poll(120).await.unwrap().unwrap();
    store
        .commit_batch(&edit, &batch(12, 2, "CDC — изменено старое", Some(200)))
        .await
        .unwrap();
    store.finish_poll(&edit, None, 0).await.unwrap();
    store.apply_pending(100).await.unwrap();
    assert_eq!(
        store.lookup(owner, ws, &["CDC".into()]).await.unwrap()[0].permalink,
        "https://t.me/reading_data_news/3",
        "old-post edit cannot outrank newer publication"
    );
    let expired = store.claim_poll(120).await.unwrap().unwrap();
    sqlx::query("UPDATE glossary_channels SET poll_until=now()-interval '1 second' WHERE owner=$1 AND workspace=$2").bind(owner).bind(ws).execute(pool).await.unwrap();
    assert_eq!(
        store
            .commit_batch(&expired, &batch(13, 4, "CDC — не сохранится", None))
            .await
            .unwrap_err(),
        GlossaryError::Conflict
    );
    assert!(store.finish_poll(&expired, None, 0).await.is_err());
    let long_term = format!("Term{}", "长".repeat(4000));
    store
        .import_post(owner, ws, &archived(5, &long_term, "без обрезки"))
        .await
        .unwrap();
    assert_eq!(
        store
            .lookup(owner, ws, std::slice::from_ref(&long_term))
            .await
            .unwrap()[0]
            .definition
            .term(),
        long_term
    );
    let conflicts = store
        .import_post(owner, ws, &archived(1, "CDC", "конфликт"))
        .await;
    assert_eq!(conflicts.unwrap_err(), GlossaryError::Conflict);
    let raw_count:i64=sqlx::query_scalar("SELECT count(*) FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='archive_post' AND source_key='1'").bind(owner).bind(ws).fetch_one(pool).await.unwrap();
    assert_eq!(raw_count, 2, "conflict preserves both original records");
    assert!(store
        .configure(Uuid::new_v4(), ws, &binding, vec![1])
        .await
        .is_err());
    store
        .begin_archive(owner, ws, "{\"post_count\":2,\"missing\":[7]}")
        .await
        .unwrap();
    let inventories:i64=sqlx::query_scalar("SELECT count(*) FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='archive_inventory'").bind(owner).bind(ws).fetch_one(pool).await.unwrap();
    assert_eq!(inventories, 1);
    let before = store.lookup(owner, ws, &["CDC".into()]).await.unwrap();
    let conflicts_before = store.status(owner, ws).await.unwrap().conflicts;
    sqlx::query(
        "UPDATE glossary_posts SET parser_version='earlier-parser' WHERE owner=$1 AND workspace=$2",
    )
    .bind(owner)
    .bind(ws)
    .execute(pool)
    .await
    .unwrap();
    assert!(store.reindex(owner, ws, 2).await.unwrap() > 0);
    let versions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT parser_version FROM glossary_posts WHERE owner=$1 AND workspace=$2",
    )
    .bind(owner)
    .bind(ws)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(versions, vec![reader_glossary::PARSER_VERSION.to_owned()]);
    assert_eq!(
        store.lookup(owner, ws, &["CDC".into()]).await.unwrap()[0].definition,
        before[0].definition
    );
    assert_eq!(
        store.status(owner, ws).await.unwrap().conflicts,
        conflicts_before
    );
    assert!(store.reindex(other, ws, 2).await.is_err());
}
