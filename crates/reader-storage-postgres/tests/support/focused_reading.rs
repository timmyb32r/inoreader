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
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Focused reading".into());
    repository
        .create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let article = Article {
        id: ArticleId::new(),
        key: DedupKey {
            location: url::Url::parse("https://example.test/same-public-article")
                .unwrap()
                .into(),
            title: "Source title".into(),
            description: None,
        },
        state: ArticleState {
            later: true,
            protect_unread: true,
            ..Default::default()
        },
        first_arrived_at: Utc::now(),
        origins: vec![],
        revision: 0,
    };
    repository
        .save_article(workspace.id(), None, article.clone())
        .await
        .unwrap();
    let before = repository
        .reading_state(owner.id, workspace.id(), article.id)
        .await
        .unwrap();
    assert_eq!(before.rating, None);
    let command = CompleteReading {
        operation_id: Uuid::new_v4(),
        expected_revision: before.revision,
        rating: Some(ArticleRating::try_from(8).unwrap()),
        reason: Some(
            reader_application::RatingReason::try_from(
                "  Полезно: CDC 中文 🦆\r\nНо нет замеров.  ".to_owned(),
            )
            .unwrap(),
        ),
    };
    let (first, repeated) = tokio::join!(
        repository.complete_reading(owner.id, workspace.id(), article.id, command.clone()),
        repository.complete_reading(owner.id, workspace.id(), article.id, command.clone())
    );
    let result = first.unwrap();
    let method: String = sqlx::query_scalar("SELECT method FROM article_read_events WHERE workspace_id=$1 AND article_id=$2 ORDER BY revision DESC LIMIT 1").bind(workspace.id().as_uuid().to_string()).bind(article.id.as_uuid().to_string()).fetch_one(pool).await.unwrap();
    assert_eq!(method, "reader");
    assert_eq!(
        repository
            .article_presentation(workspace.id(), article.id)
            .await
            .unwrap()
            .read_method,
        Some(reader_application::ReadMethod::Reader)
    );
    assert_eq!(result, repeated.unwrap());
    assert!(result.state.read);
    assert_eq!(result.state.rating, command.rating);
    assert_eq!(result.state.reason, command.reason);
    assert_eq!(
        repository
            .reading_state(owner.id, workspace.id(), article.id)
            .await
            .unwrap()
            .reason,
        command.reason
    );
    let mut different_reason = command.clone();
    different_reason.reason = None;
    assert!(matches!(
        repository
            .complete_reading(owner.id, workspace.id(), article.id, different_reason)
            .await,
        Err(RepositoryError::Conflict)
    ));
    let saved = repository
        .article(workspace.id(), article.id)
        .await
        .unwrap();
    assert!(saved.state.later && saved.state.protect_unread);
    assert_eq!(saved.key, article.key);
    assert_eq!(saved.revision, 1);
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM article_read_events WHERE workspace_id=$1 AND article_id=$2",
    )
    .bind(workspace.id().as_uuid().to_string())
    .bind(article.id.as_uuid().to_string())
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(events, 1);
    let mut different = command.clone();
    different.rating = Some(ArticleRating::try_from(1).unwrap());
    assert!(matches!(
        repository
            .complete_reading(owner.id, workspace.id(), article.id, different)
            .await,
        Err(RepositoryError::Conflict)
    ));
    let undone = repository
        .undo_reading(owner.id, workspace.id(), article.id, command.operation_id)
        .await
        .unwrap();
    assert!(undone.undone);
    assert!(!undone.state.read);
    assert_eq!(undone.state.rating, None);
    assert_eq!(undone.state.reason, None);
    assert_eq!(
        undone,
        repository
            .undo_reading(owner.id, workspace.id(), article.id, command.operation_id)
            .await
            .unwrap()
    );
    assert_eq!(
        undone,
        repository
            .complete_reading(owner.id, workspace.id(), article.id, command.clone())
            .await
            .unwrap()
    );
    let restored = repository
        .article(workspace.id(), article.id)
        .await
        .unwrap();
    assert_eq!(restored.state, article.state);
    let second = CompleteReading {
        operation_id: Uuid::new_v4(),
        expected_revision: undone.state.revision,
        rating: Some(ArticleRating::try_from(9).unwrap()),
        reason: Some(
            reader_application::RatingReason::try_from("Previous explanation".to_owned()).unwrap(),
        ),
    };
    repository
        .complete_reading(owner.id, workspace.id(), article.id, second.clone())
        .await
        .unwrap();
    let scope = reader_application::article_commands::OwnedWorkspace::resolve(
        &repository,
        owner.id,
        workspace.id(),
    )
    .await
    .unwrap();
    reader_application::article_commands::update_article(
        &repository,
        &scope,
        article.id,
        reader_application::article_commands::ArticlePatch {
            read: Some(false),
            later: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        repository
            .reading_state(owner.id, workspace.id(), article.id)
            .await
            .unwrap()
            .rating,
        second.rating
    );
    assert!(matches!(
        repository
            .undo_reading(owner.id, workspace.id(), article.id, second.operation_id)
            .await,
        Err(RepositoryError::Conflict)
    ));
    // A late completion cannot overwrite a newer read/unread mutation.
    let stale = CompleteReading {
        operation_id: Uuid::new_v4(),
        expected_revision: ReadingRevision::new(0).unwrap(),
        rating: command.rating,
        reason: command.reason.clone(),
    };
    assert!(matches!(
        repository
            .complete_reading(owner.id, workspace.id(), article.id, stale)
            .await,
        Err(RepositoryError::Conflict)
    ));
    // Restore a pre-existing rating exactly (including its original timestamp).
    let prior = repository
        .reading_state(owner.id, workspace.id(), article.id)
        .await
        .unwrap();
    let third = CompleteReading {
        operation_id: Uuid::new_v4(),
        expected_revision: prior.revision,
        rating: Some(ArticleRating::try_from(2).unwrap()),
        reason: None,
    };
    repository
        .complete_reading(owner.id, workspace.id(), article.id, third.clone())
        .await
        .unwrap();
    let undo = repository
        .undo_reading(owner.id, workspace.id(), article.id, third.operation_id)
        .await
        .unwrap();
    assert_eq!(undo.state.rating, prior.rating);
    assert_eq!(undo.state.reason, prior.reason);
    assert_eq!(undo.state.rated_at, prior.rated_at);
    // Foreign owner with an identical public source cannot read/mutate these results.
    let foreign = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let foreign_workspace = Workspace::new(WorkspaceId::new(), foreign.id, "Other reader".into());
    repository
        .create_account_and_workspace(foreign.clone(), foreign_workspace.clone())
        .await
        .unwrap();
    repository
        .save_article(foreign_workspace.id(), None, article.clone())
        .await
        .unwrap();
    assert!(matches!(
        repository
            .reading_state(foreign.id, workspace.id(), article.id)
            .await,
        Err(RepositoryError::NotFound)
    ));
    assert!(matches!(
        repository
            .complete_reading(foreign.id, workspace.id(), article.id, command.clone())
            .await,
        Err(RepositoryError::NotFound)
    ));
    assert!(matches!(
        repository
            .undo_reading(foreign.id, workspace.id(), article.id, command.operation_id)
            .await,
        Err(RepositoryError::NotFound)
    ));
    assert!(matches!(
        repository
            .undo_reading(
                foreign.id,
                foreign_workspace.id(),
                article.id,
                command.operation_id
            )
            .await,
        Err(RepositoryError::NotFound)
    ));
    assert_eq!(
        repository
            .reading_state(foreign.id, foreign_workspace.id(), article.id)
            .await
            .unwrap()
            .rating,
        None
    );
    // A failed write anywhere rolls back read, rating, event and receipt together.
    let mut other = article.clone();
    other.id = ArticleId::new();
    repository
        .save_article(workspace.id(), None, other.clone())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_focused_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected rating failure'; END $$; CREATE TRIGGER reject_focused_fixture BEFORE INSERT ON article_ratings FOR EACH ROW EXECUTE FUNCTION reject_focused_fixture();").execute(pool).await.unwrap();
    let failing = CompleteReading {
        operation_id: Uuid::new_v4(),
        expected_revision: ReadingRevision::new(0).unwrap(),
        rating: command.rating,
        reason: command.reason.clone(),
    };
    assert!(repository
        .complete_reading(owner.id, workspace.id(), other.id, failing.clone())
        .await
        .is_err());
    sqlx::raw_sql("DROP TRIGGER reject_focused_fixture ON article_ratings; DROP FUNCTION reject_focused_fixture();").execute(pool).await.unwrap();
    assert_eq!(
        repository.article(workspace.id(), other.id).await.unwrap(),
        other
    );
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM reading_completions WHERE id=$1")
        .bind(failing.operation_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(receipts, 0);
    assert!(repository
        .complete_reading(owner.id, workspace.id(), other.id, failing)
        .await
        .is_ok());
    // Ingest's identity-preserving delete/reinsert must retain ratings/receipts.
    // A permanent identity deletion must fail, never cascade away user data.
    let key = format!("{}/{}", workspace.id().as_uuid(), other.id.as_uuid());
    let mut tx = pool.begin().await.unwrap();
    let (revision, document): (i64, String) =
        sqlx::query_as("DELETE FROM articles WHERE id=$1 RETURNING revision,document")
            .bind(&key)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    sqlx::query("INSERT INTO articles(id,revision,document) VALUES($1,$2,$3)")
        .bind(&key)
        .bind(revision)
        .bind(document)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        repository
            .reading_state(owner.id, workspace.id(), other.id)
            .await
            .unwrap()
            .rating,
        command.rating
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM articles WHERE id=$1")
        .bind(&key)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    assert!(repository.article(workspace.id(), other.id).await.is_ok());
    assert_eq!(
        repository
            .reading_state(owner.id, workspace.id(), other.id)
            .await
            .unwrap()
            .reason,
        command.reason
    );
    let retained: String =
        sqlx::query_scalar("SELECT document FROM reading_completions WHERE owner=$1 AND id=$2")
            .bind(owner.id.as_uuid().to_string())
            .bind(command.operation_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&retained).unwrap()["command"]["reason"],
        serde_json::to_value(&command.reason).unwrap()
    );
    // An explicit abstention has a receipt/date, but no numeric training label.
    let unknown_article = Article {
        id: ArticleId::new(),
        key: other.key.clone(),
        state: ArticleState::default(),
        first_arrived_at: chrono::Utc::now(),
        origins: vec![],
        revision: 0,
    };
    repository
        .save_article(workspace.id(), None, unknown_article.clone())
        .await
        .unwrap();
    let receipt = repository
        .complete_reading(
            owner.id,
            workspace.id(),
            unknown_article.id,
            CompleteReading {
                operation_id: Uuid::new_v4(),
                expected_revision: ReadingRevision::new(0).unwrap(),
                rating: None,
                reason: Some("Не знаю".to_string().try_into().unwrap()),
            },
        )
        .await
        .unwrap();
    assert!(
        receipt.state.read && receipt.state.rating.is_none() && receipt.state.rated_at.is_some()
    );
    assert_eq!(
        repository
            .reading_state(owner.id, workspace.id(), unknown_article.id)
            .await
            .unwrap(),
        receipt.state
    );
    assert!(repository
        .reading_state(foreign.id, workspace.id(), unknown_article.id)
        .await
        .is_err());
    let undone = repository
        .undo_reading(
            owner.id,
            workspace.id(),
            unknown_article.id,
            receipt.operation_id,
        )
        .await
        .unwrap();
    assert!(!undone.state.read && undone.state.rated_at.is_none());
}

pub async fn verify_commit_projects(pool: &PgPool) {
    use reader_application::{
        ArticlePageCursor, ArticlePageDirection, ArticlePageRequest, ArticleScope, CommitProject,
        SelectionLimit,
    };
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
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Project commits".into());
    repository
        .create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let foreign = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let foreign_workspace = Workspace::new(
        WorkspaceId::new(),
        foreign.id,
        "Other project commits".into(),
    );
    repository
        .create_account_and_workspace(foreign, foreign_workspace.clone())
        .await
        .unwrap();
    let mut expected = Vec::new();
    for (index, path, read) in [
        (0, "engine/commit/a", false),
        (1, "engine/commits/b", false),
        (2, "engine/commit/c", false),
        (3, "engine/commit/read", true),
        (4, "engine-extra/commit/d", false),
        (5, "engine/issues/1", false),
    ] {
        let article = Article {
            id: ArticleId::new(),
            key: DedupKey {
                location: url::Url::parse(&format!("https://github.com/acme/{path}"))
                    .unwrap()
                    .into(),
                title: path.into(),
                description: None,
            },
            state: ArticleState {
                read,
                ..Default::default()
            },
            first_arrived_at: Utc::now() + chrono::Duration::seconds(index),
            origins: vec![],
            revision: 0,
        };
        repository
            .save_article(workspace.id(), None, article.clone())
            .await
            .unwrap();
        repository
            .save_article(foreign_workspace.id(), None, article.clone())
            .await
            .unwrap();
        if index < 3 {
            expected.push(article.id);
        }
    }
    let project = CommitProject::from_url("https://github.com/acme/engine/commit/a").unwrap();
    let request = |cursor| {
        ArticlePageRequest::new(
            ArticleScope::Feed,
            cursor,
            ArticlePageDirection::Older,
            SelectionLimit::new(2).unwrap(),
        )
        .unwrap()
        .with_commit_project(project.clone())
        .unwrap()
    };
    let first = repository
        .article_summary_page(workspace.id(), request(None))
        .await
        .unwrap();
    assert_eq!(first.total, 3);
    assert_eq!(first.articles.len(), 2);
    assert!(first.has_older);
    let last = &first.articles.last().unwrap().article;
    let second = repository
        .article_summary_page(
            workspace.id(),
            request(Some(ArticlePageCursor {
                article_id: last.id,
                arrived_at: last.first_arrived_at,
            })),
        )
        .await
        .unwrap();
    assert_eq!(second.articles.len(), 1);
    assert!(!second.has_older);
    let mut actual: Vec<_> = first
        .articles
        .iter()
        .chain(second.articles.iter())
        .map(|a| a.article.id)
        .collect();
    actual.sort_by_key(|id| id.as_uuid());
    expected.sort_by_key(|id| id.as_uuid());
    assert_eq!(actual, expected);
    let owned = reader_application::article_commands::OwnedWorkspace::resolve(
        &repository,
        owner.id,
        workspace.id(),
    )
    .await
    .unwrap();
    reader_application::article_commands::update_article(
        &repository,
        &owned,
        actual[0],
        reader_application::article_commands::ArticlePatch {
            read: Some(true),
            later: None,
        },
    )
    .await
    .unwrap();
    let updated = repository
        .article_summary_page(workspace.id(), request(None))
        .await
        .unwrap();
    assert_eq!(updated.total, 2);
    let foreign_page = repository
        .article_summary_page(foreign_workspace.id(), request(None))
        .await
        .unwrap();
    assert_eq!(foreign_page.total, 3);
    assert!(
        !repository
            .article(foreign_workspace.id(), actual[0])
            .await
            .unwrap()
            .state
            .read
    );
}
