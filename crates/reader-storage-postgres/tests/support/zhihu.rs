use super::*;
use reader_ingest::zhihu::Store;
pub async fn verify(pool: &PgPool) {
    sqlx::query("CREATE SCHEMA zhihu_fixture")
        .execute(pool)
        .await
        .unwrap();
    let isolated = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET search_path TO zhihu_fixture")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
        .unwrap();
    prepare_schema(&isolated).await.unwrap();
    verify_inner(&isolated).await;
    isolated.close().await;
    sqlx::query("DROP SCHEMA zhihu_fixture CASCADE")
        .execute(pool)
        .await
        .unwrap();
}
async fn verify_inner(pool: &PgPool) {
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
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Zhihu fixture".into());
    repo.create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse("https://www.zhihu.com/people/fixture").unwrap(),
        "原文".into(),
    );
    repo.save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let before: String = sqlx::query_scalar("SELECT document FROM subscriptions WHERE id=$1")
        .bind(subscription.id().as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    let store = reader_storage_postgres::PostgresZhihuStore::new(pool.clone());
    let owner = owner.id.as_uuid();
    let cipher = reader_ai::CredentialCipher::new(&[7; 32]).unwrap();
    let encrypted = cipher.encrypt(owner, "z_c0=fixture").unwrap();
    store
        .save(
            owner,
            encrypted.clone(),
            std::num::NonZeroUsize::new(2).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(store.load(owner).await.unwrap(), Some(encrypted.clone()));
    assert!(store.load(Uuid::new_v4()).await.unwrap().is_none());
    assert!(cipher.decrypt(Uuid::new_v4(), &encrypted).is_err());
    let raw:String=sqlx::query_scalar("SELECT src.document FROM sources src JOIN subscription_sources ss ON ss.source_id=src.id WHERE ss.subscription_id=$1").bind(subscription.id().as_uuid().to_string()).fetch_one(pool).await.unwrap();
    assert!(!raw.contains("z_c0"));
    let source: SourceDefinition = serde_json::from_str(&raw).unwrap();
    assert_eq!(store.source_owner(&source).await.unwrap(), owner);
    assert!(matches!(
        source.kind(),
        SourceKind::BuiltIn(reader_ingest::BuiltInAdapter::Zhihu { .. })
    ));
    store.remove(owner).await.unwrap();
    assert!(store.load(owner).await.unwrap().is_none());
    let after: String = sqlx::query_scalar("SELECT document FROM subscriptions WHERE id=$1")
        .bind(subscription.id().as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    // A second account must never borrow another account's session, even when
    // public source caching has associated both subscriptions with one source.
    let other = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let w = Workspace::new(WorkspaceId::new(), other.id, "Second".into());
    repo.create_account_and_workspace(other, w.clone())
        .await
        .unwrap();
    let second = Subscription::new(
        SubscriptionId::new(),
        w.id(),
        subscription.source_url().clone(),
        "Second".into(),
    );
    repo.save_subscription(None, second).await.unwrap();
    assert!(store.source_owner(&source).await.is_err());
    assert!(store
        .save(owner, encrypted, std::num::NonZeroUsize::new(2).unwrap())
        .await
        .is_err());
    assert!(store.load(owner).await.unwrap().is_none());
}
