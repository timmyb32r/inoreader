use super::*;
use reader_application::{
    ArticleRating, CompleteReading, FocusedReadingRepository, ReadingRevision,
};

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
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Upgrade fixture".into());
    repository
        .create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let article = Article {
        id: ArticleId::new(),
        key: DedupKey {
            location: url::Url::parse("https://example.test/rating-upgrade")
                .unwrap()
                .into(),
            title: "Fixture".into(),
            description: None,
        },
        state: ArticleState::default(),
        first_arrived_at: Utc::now(),
        origins: vec![],
        revision: 0,
    };
    repository
        .save_article(workspace.id(), None, article.clone())
        .await
        .unwrap();
    let command = CompleteReading {
        operation_id: Uuid::new_v4(),
        expected_revision: ReadingRevision::new(0).unwrap(),
        rating: Some(ArticleRating::try_from(8).unwrap()),
        reason: None,
    };
    let before = repository
        .complete_reading(owner.id, workspace.id(), article.id, command.clone())
        .await
        .unwrap();
    // Construct the preceding schema's exact receipt shape (no reason fields).
    sqlx::query("UPDATE reading_completions SET document=(document::jsonb #- '{command,reason}' #- '{previous,reason}' #- '{completed,state,reason}')::text WHERE id=$1")
        .bind(command.operation_id).execute(pool).await.unwrap();
    let raw: String = sqlx::query_scalar("SELECT document FROM reading_completions WHERE id=$1")
        .bind(command.operation_id)
        .fetch_one(pool)
        .await
        .unwrap();
    // Build the old three-column layout, rather than DROP COLUMN + ADD: the
    // latter leaves an artificial attnum hole unlike any production v12 table.
    super::schema_contracts::remove_interests(pool).await;
    sqlx::raw_sql("CREATE TEMP TABLE rating_upgrade_values AS SELECT article_key,rating,rated_at FROM article_ratings; DROP TABLE article_ratings; CREATE TABLE article_ratings(article_key TEXT PRIMARY KEY REFERENCES articles(id) DEFERRABLE INITIALLY DEFERRED,rating SMALLINT NOT NULL CHECK(rating BETWEEN 1 AND 10),rated_at TIMESTAMPTZ NOT NULL); INSERT INTO article_ratings SELECT * FROM rating_upgrade_values; DROP TABLE rating_upgrade_values; DELETE FROM schema_releases; INSERT INTO schema_releases(version,release) VALUES(12,'focused-reading-2026-09-29')").execute(pool).await.unwrap();
    reader_storage_postgres::upgrade_schema(pool, std::num::NonZeroU32::new(50).unwrap())
        .await
        .unwrap();
    reader_storage_postgres::verify_schema(pool).await.unwrap();
    let preserved: String =
        sqlx::query_scalar("SELECT document FROM reading_completions WHERE id=$1")
            .bind(command.operation_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(raw, preserved, "upgrade never rewrites old receipt bytes");
    assert_eq!(
        repository
            .reading_state(owner.id, workspace.id(), article.id)
            .await
            .unwrap(),
        before.state
    );
    assert_eq!(
        repository
            .complete_reading(owner.id, workspace.id(), article.id, command.clone())
            .await
            .unwrap(),
        before
    );
    let undone = repository
        .undo_reading(owner.id, workspace.id(), article.id, command.operation_id)
        .await
        .unwrap();
    assert!(undone.undone && !undone.state.read);
    assert!(undone.state.reason.is_none());
}
