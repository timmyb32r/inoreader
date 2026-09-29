use super::*;

const RETIRE: &str = include_str!("../../../../tools/remove_subscription_notes.sql");

pub async fn verify(pool: &PgPool) {
    let repository =
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap();
    let account = AccountRecord {
        id: AccountId::new(),
        username: format!("notes-{}", Uuid::new_v4()),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let workspace = Workspace::new(WorkspaceId::new(), account.id, "Notes".into());
    repository
        .create_account_and_workspace(account.clone(), workspace.clone())
        .await
        .unwrap();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        "https://example.test/feed".parse().unwrap(),
        "Source".into(),
    );
    repository
        .save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let note = "  Exact 中文\n\n**bold** and [[Wiki]]\n";
    let mut original = serde_json::to_value(&subscription).unwrap();
    original["personal_note"] = serde_json::json!(note);
    sqlx::query("UPDATE subscriptions SET document=$1 WHERE id=$2")
        .bind(original.to_string())
        .bind(subscription.id().as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
    assert!(reader_storage_postgres::verify_schema(pool).await.is_err());
    let mut conn = pool.acquire().await.unwrap();
    assert!(sqlx::raw_sql(RETIRE).execute(&mut *conn).await.is_err());
    sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await.unwrap();
    let retained: String = sqlx::query_scalar(
        "SELECT document::jsonb->>'personal_note' FROM subscriptions WHERE id=$1",
    )
    .bind(subscription.id().as_uuid().to_string())
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(retained, note);
    let namespace = Uuid::new_v4();
    let page = Uuid::new_v4();
    let revision = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO wiki_namespaces(id,owner,name) VALUES($1,$2,'Private')")
        .bind(namespace)
        .bind(account.id.as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO wiki_pages(namespace,id,revision,name,markdown,deleted,author,updated_at) VALUES($1,$2,$3,'Source',$4,false,$5,now())").bind(namespace).bind(page).bind(revision).bind(note).bind(account.id.as_uuid().to_string()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO wiki_revisions(namespace,page,revision,name,markdown,deleted,author,created_at,action) VALUES($1,$2,$3,'Source',$4,false,$5,now(),'save')").bind(namespace).bind(page).bind(revision).bind(note).bind(account.id.as_uuid().to_string()).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO subscription_wiki_links(owner,subscription_id,namespace,page) VALUES($1,$2,$3,$4)").bind(account.id.as_uuid().to_string()).bind(subscription.id().as_uuid().to_string()).bind(namespace).bind(page).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    // A near match is not enough: preserve the note if the page text has changed.
    sqlx::query("UPDATE wiki_pages SET markdown=markdown || 'changed' WHERE namespace=$1")
        .bind(namespace)
        .execute(pool)
        .await
        .unwrap();
    assert!(sqlx::raw_sql(RETIRE).execute(&mut *conn).await.is_err());
    sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await.unwrap();
    sqlx::query("UPDATE wiki_pages SET markdown=$1 WHERE namespace=$2")
        .bind(note)
        .bind(namespace)
        .execute(pool)
        .await
        .unwrap();
    sqlx::raw_sql(RETIRE).execute(&mut *conn).await.unwrap();
    sqlx::raw_sql(RETIRE).execute(&mut *conn).await.unwrap();
    let stored: String = sqlx::query_scalar("SELECT document FROM subscriptions WHERE id=$1")
        .bind(subscription.id().as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    original.as_object_mut().unwrap().remove("personal_note");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).unwrap(),
        original
    );
    let preserved: String =
        sqlx::query_scalar("SELECT markdown FROM wiki_pages WHERE namespace=$1 AND id=$2")
            .bind(namespace)
            .bind(page)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(preserved, note);
    reader_storage_postgres::verify_schema(pool).await.unwrap();
}
