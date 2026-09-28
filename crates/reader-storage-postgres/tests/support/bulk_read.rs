use super::*;
use reader_application::{
    article_commands::{mark_articles_read, MarkReadError, OwnedWorkspace},
    ArticleScope, SelectionLimit,
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
    let workspace = Workspace::new(WorkspaceId::new(), owner.id, "Bulk fixture".into());
    repository
        .create_account_and_workspace(owner.clone(), workspace.clone())
        .await
        .unwrap();
    let scope = OwnedWorkspace::resolve(&repository, owner.id, workspace.id())
        .await
        .unwrap();
    assert!(
        OwnedWorkspace::resolve(&repository, AccountId::new(), workspace.id())
            .await
            .is_err()
    );
    let other = AccountRecord {
        id: AccountId::new(),
        username: Uuid::new_v4().to_string(),
        password_hash: "fixture".into(),
        admin: false,
        auth_revision: 0,
        revision: 0,
    };
    let foreign_workspace = Workspace::new(WorkspaceId::new(), other.id, "Foreign".into());
    repository
        .create_account_and_workspace(other, foreign_workspace.clone())
        .await
        .unwrap();
    let foreign_subscription = Subscription::new(
        SubscriptionId::new(),
        foreign_workspace.id(),
        url::Url::parse("https://example.test/foreign").unwrap(),
        "Foreign".into(),
    );
    repository
        .save_subscription(None, foreign_subscription.clone())
        .await
        .unwrap();
    assert!(matches!(
        mark_articles_read(
            &repository,
            &scope,
            ArticleScope::Subscription(foreign_subscription.id()),
            SelectionLimit::new(10).unwrap()
        )
        .await,
        Err(MarkReadError::Repository(RepositoryError::NotFound))
    ));
    for number in 0..3 {
        repository
            .save_article(
                workspace.id(),
                None,
                Article {
                    id: ArticleId::new(),
                    key: DedupKey {
                        location: ArticleLocation::from(
                            url::Url::parse(&format!("https://example.test/bulk/{number}"))
                                .unwrap(),
                        ),
                        title: format!("Article {number}"),
                        description: None,
                    },
                    state: ArticleState {
                        later: number == 1,
                        ..Default::default()
                    },
                    first_arrived_at: Utc::now(),
                    origins: vec![],
                    revision: 0,
                },
            )
            .await
            .unwrap();
    }
    let limit = SelectionLimit::new(1).unwrap();
    assert_eq!(
        repository
            .unread_selection(workspace.id(), ArticleScope::Feed, limit)
            .await
            .unwrap()
            .len(),
        2,
        "exactly one lookahead row, not all three"
    );
    assert!(matches!(
        mark_articles_read(&repository, &scope, ArticleScope::Feed, limit).await,
        Err(MarkReadError::Limit)
    ));
    assert!(repository
        .articles_by_workspace(workspace.id())
        .await
        .unwrap()
        .iter()
        .all(|a| !a.state.read));
    mark_articles_read(&repository, &scope, ArticleScope::Later, limit)
        .await
        .unwrap();
    let values = repository
        .articles_by_workspace(workspace.id())
        .await
        .unwrap();
    assert_eq!(values.iter().filter(|a| a.state.read).count(), 1);
    assert!(values.iter().find(|a| a.state.read).unwrap().state.later);
    // A concurrent modification must roll back the entire selected snapshot.
    let mut selected = repository
        .unread_selection(
            workspace.id(),
            ArticleScope::Feed,
            SelectionLimit::new(3).unwrap(),
        )
        .await
        .unwrap();
    let mut concurrent = selected.last().unwrap().clone();
    concurrent.revision += 1;
    concurrent.state.later = true;
    repository
        .save_article(workspace.id(), Some(0), concurrent)
        .await
        .unwrap();
    for value in &mut selected {
        value.revision += 1;
        value.state.read = true;
    }
    assert!(matches!(
        repository
            .mark_articles_read_atomic(workspace.id(), selected)
            .await,
        Err(RepositoryError::Conflict)
    ));
    assert_eq!(
        repository
            .articles_by_workspace(workspace.id())
            .await
            .unwrap()
            .iter()
            .filter(|a| a.state.read)
            .count(),
        1
    );
    mark_articles_read(
        &repository,
        &scope,
        ArticleScope::Feed,
        SelectionLimit::new(3).unwrap(),
    )
    .await
    .unwrap();
    assert!(repository
        .articles_by_workspace(workspace.id())
        .await
        .unwrap()
        .iter()
        .all(|a| a.state.read));
}
