use super::*;

pub async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    workspace: Uuid,
    article: Uuid,
) {
    let op = Uuid::new_v4();
    let mut value = record(owner, workspace, article, op);
    value.operations[0].kind = OperationKind::Start { regenerate: true };
    value.operations[0].task = AttemptTask::Reply;
    value.view.messages[0].purpose = Some(MessagePurpose::Chat);
    let source = (0..5000)
        .map(|_| Uuid::new_v4().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    value.snapshot.as_mut().unwrap().text = source.clone();
    value.snapshot.as_mut().unwrap().safe_html = source;
    let chat = store.create_chat(value.clone(), op, true).await.unwrap();
    let pinned: String = sqlx::query_scalar("SELECT inputs FROM ai_chats WHERE id=$1")
        .bind(chat.view.id)
        .fetch_one(pool)
        .await
        .unwrap();
    // Poison at the head of both queued and expired work must be retained but
    // must not prevent the valid paid operation behind it from being claimed.
    for (expired, bad_draft) in [(false, false), (true, false), (true, true)] {
        let bad = Uuid::new_v4();
        let original = if bad_draft {
            let raw: String = sqlx::query_scalar("SELECT document FROM ai_chats WHERE id=$1")
                .bind(chat.view.id)
                .fetch_one(pool)
                .await
                .unwrap();
            let mut raw: serde_json::Value = serde_json::from_str(&raw).unwrap();
            raw["view"]["id"] = serde_json::json!(bad);
            raw["view"]["messages"][0]["purpose"] = serde_json::json!("summary");
            raw["operations"][0]["task"] =
                serde_json::json!({"Summary":{"draft":"invalid retained draft"}});
            raw.to_string()
        } else {
            "broken original".into()
        };
        sqlx::query("INSERT INTO ai_chats(id,owner,workspace,article,created_at,scheduled_at,status,document,inputs,lease,lease_until,public_view) VALUES($1,$2,$3,$4,now()-interval '1 day',now()-interval '1 day',$5,$9,$6,$7,$8,'broken public')")
            .bind(bad).bind(owner).bind(workspace).bind(article).bind(if expired {"generating"} else {"queued"})
            .bind(&pinned).bind(expired.then(Uuid::new_v4)).bind(expired.then(|| Utc::now()-chrono::Duration::minutes(1))).bind(&original)
            .execute(pool).await.unwrap();
        if !expired {
            assert!(store.claim(60).await.unwrap().is_none());
        } else {
            // Let the expiry sweep process poison without admitting the fixture yet.
            sqlx::query("UPDATE ai_chats SET lease=$2 WHERE id=$1")
                .bind(chat.view.id)
                .bind(Uuid::new_v4())
                .execute(pool)
                .await
                .unwrap();
            assert!(store.claim(60).await.unwrap().is_none());
            sqlx::query("UPDATE ai_chats SET lease=NULL WHERE id=$1")
                .bind(chat.view.id)
                .execute(pool)
                .await
                .unwrap();
        }
        let (status, raw): (String, String) =
            sqlx::query_as("SELECT status,document FROM ai_chats WHERE id=$1")
                .bind(bad)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(status, "quarantined");
        assert_eq!(raw, original);
    }
    let claim = store.claim(60).await.unwrap().unwrap();
    assert_eq!(claim.record.view.id, chat.view.id);
    let before: String = sqlx::query_scalar("SELECT pg_current_wal_insert_lsn()::text")
        .fetch_one(pool)
        .await
        .unwrap();
    let started = std::time::Instant::now();
    for index in 0..20 {
        store
            .update_claim(
                &claim,
                ChatStatus::Generating,
                Some(&format!("Progress {index}")),
                None,
                None,
            )
            .await
            .unwrap();
    }
    let current_us = started.elapsed().as_micros();
    let current_wal: i64 = sqlx::query_scalar(
        "SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(),$1::pg_lsn)::bigint",
    )
    .bind(before)
    .fetch_one(pool)
    .await
    .unwrap();
    let pinned_after: String = sqlx::query_scalar("SELECT inputs FROM ai_chats WHERE id=$1")
        .bind(chat.view.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(pinned_after, pinned);
    assert_eq!(
        store
            .chat(owner, chat.view.id)
            .await
            .unwrap()
            .snapshot
            .unwrap()
            .text,
        value.snapshot.as_ref().unwrap().text
    );
    let first = store.public_chat(owner, chat.view.id, None).await.unwrap();
    assert_eq!(first.chat.unwrap().messages[0].content, "Progress 19");
    assert!(store
        .public_chat(owner, chat.view.id, Some(&first.revision))
        .await
        .unwrap()
        .chat
        .is_none());
    assert!(matches!(
        store
            .public_chat(Uuid::new_v4(), chat.view.id, Some(&first.revision))
            .await,
        Err(AiError::NotFound)
    ));
    // Quarantine is a visible failure even if a previously valid public cache
    // remains and the caller sends the unchanged revision. Accounting cannot revive it.
    sqlx::query("UPDATE ai_chats SET status='quarantined' WHERE id=$1")
        .bind(chat.view.id)
        .execute(pool)
        .await
        .unwrap();
    assert!(matches!(
        store
            .public_chat(owner, chat.view.id, Some(&first.revision))
            .await,
        Err(AiError::Storage)
    ));
    assert!(matches!(
        store
            .update_claim(
                &claim,
                ChatStatus::Generating,
                Some("forbidden"),
                None,
                None
            )
            .await,
        Err(AiError::Storage)
    ));
    sqlx::query("UPDATE ai_chats SET status='generating' WHERE id=$1")
        .bind(chat.view.id)
        .execute(pool)
        .await
        .unwrap();
    // Once a worker has loaded its immutable input, reads and progress must not
    // select/decode that column again. Corruption remains visible to execution reads.
    sqlx::query("UPDATE ai_chats SET inputs='invalid fixture input' WHERE id=$1")
        .bind(chat.view.id)
        .execute(pool)
        .await
        .unwrap();
    store
        .update_claim(
            &claim,
            ChatStatus::Generating,
            Some("Progress without reloading inputs"),
            None,
            None,
        )
        .await
        .unwrap();
    let changed = store
        .public_chat(owner, chat.view.id, Some(&first.revision))
        .await
        .unwrap();
    assert_ne!(changed.revision, first.revision);
    assert_eq!(
        changed.chat.unwrap().messages[0].content,
        "Progress without reloading inputs"
    );
    assert!(store.chat(owner, chat.view.id).await.is_err());
    sqlx::query("UPDATE ai_chats SET inputs=$2 WHERE id=$1")
        .bind(chat.view.id)
        .bind(&pinned)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE ai_write_baseline(document TEXT NOT NULL)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ai_write_baseline VALUES($1)")
        .bind(serde_json::to_string(&value).unwrap())
        .execute(pool)
        .await
        .unwrap();
    let before: String = sqlx::query_scalar("SELECT pg_current_wal_insert_lsn()::text")
        .fetch_one(pool)
        .await
        .unwrap();
    let started = std::time::Instant::now();
    for index in 0..20 {
        value.view.messages[0].content = format!("Progress {index}");
        sqlx::query("UPDATE ai_write_baseline SET document=$1")
            .bind(serde_json::to_string(&value).unwrap())
            .execute(pool)
            .await
            .unwrap();
    }
    let baseline_us = started.elapsed().as_micros();
    let baseline_wal: i64 = sqlx::query_scalar(
        "SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(),$1::pg_lsn)::bigint",
    )
    .bind(before)
    .fetch_one(pool)
    .await
    .unwrap();
    eprintln!("chat_progress_benchmark updates=20 pinned_bytes={} new_wal={current_wal} old_wal={baseline_wal} new_us={current_us} old_write_only_us={baseline_us}", pinned.len());
    assert!(
        current_wal * 5 < baseline_wal,
        "progress must not rewrite the article payload"
    );
    sqlx::query("DROP TABLE ai_write_baseline")
        .execute(pool)
        .await
        .unwrap();
    store.stop(owner, chat.view.id).await.unwrap();
}
