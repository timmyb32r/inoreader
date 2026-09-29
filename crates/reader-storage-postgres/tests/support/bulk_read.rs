#[path = "source_activity.rs"]
mod source_activity;
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
    source_activity::verify(pool, &repository, workspace.id(), foreign_workspace.id()).await;
    let before_reads = repository
        .reading_activity(workspace.id(), "UTC")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before_reads.len(), 1);
    assert_eq!(before_reads[0].count, 0);
    assert_eq!(before_reads[0].arrived, 3);
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
    let expected_revision = concurrent.revision;
    concurrent.revision += 1;
    concurrent.state.later = true;
    repository
        .save_article(workspace.id(), Some(expected_revision), concurrent)
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
    let activity = repository
        .reading_activity(workspace.id(), "UTC")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(activity.len(), 1);
    assert_eq!(activity[0].count, 3);
    assert_eq!(activity[0].arrived, 3);
    assert!(repository
        .reading_activity(foreign_workspace.id(), "UTC")
        .await
        .unwrap()
        .unwrap()
        .is_empty());
    assert!(repository
        .reading_activity(workspace.id(), "invalid/timezone")
        .await
        .unwrap()
        .is_none());
    // Already-read writes and retries do not create another event.
    let article = repository
        .articles_by_workspace(workspace.id())
        .await
        .unwrap()
        .remove(0);
    let events = |pool: PgPool, id: String| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM article_read_events WHERE workspace_id=$1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(
        events(pool.clone(), workspace.id().as_uuid().to_string()).await,
        3
    );
    for read in [true, false, true] {
        reader_application::article_commands::update_article(
            &repository,
            &scope,
            article.id,
            reader_application::article_commands::ArticlePatch {
                read: Some(read),
                later: None,
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(
        events(pool.clone(), workspace.id().as_uuid().to_string()).await,
        4
    );
    assert_eq!(
        repository
            .reading_activity(workspace.id(), "UTC")
            .await
            .unwrap()
            .unwrap()[0]
            .count,
        3
    );
    // Explicit boundary: 23:30 UTC is already tomorrow in Moscow; count distinct articles per day.
    sqlx::query("UPDATE article_read_events SET occurred_at=((now() AT TIME ZONE 'UTC')::date::timestamp AT TIME ZONE 'UTC') - interval '30 minutes' WHERE workspace_id=$1")
        .bind(workspace.id().as_uuid().to_string()).execute(pool).await.unwrap();
    let utc = repository
        .reading_activity(workspace.id(), "UTC")
        .await
        .unwrap()
        .unwrap();
    let moscow = repository
        .reading_activity(workspace.id(), "Europe/Moscow")
        .await
        .unwrap()
        .unwrap();
    assert_ne!(utc[0].day, moscow[0].day);
    assert_eq!(utc[0].count, 3);
    assert_eq!(moscow[0].count, 3);
    let mut boundary = article.clone();
    boundary.id = ArticleId::new();
    boundary.revision = 0;
    boundary.key.title = "Nanosecond day boundary".into();
    boundary.first_arrived_at = Utc::now()
        .date_naive()
        .pred_opt()
        .unwrap()
        .and_hms_nano_opt(23, 59, 59, 999_999_999)
        .unwrap()
        .and_utc();
    let expected_utc = boundary.first_arrived_at.format("%Y-%m-%d").to_string();
    let expected_moscow = (boundary.first_arrived_at + ChronoDuration::hours(3))
        .format("%Y-%m-%d")
        .to_string();
    repository
        .save_article(workspace.id(), None, boundary.clone())
        .await
        .unwrap();
    let utc = repository
        .reading_activity(workspace.id(), "UTC")
        .await
        .unwrap()
        .unwrap();
    let moscow = repository
        .reading_activity(workspace.id(), "Europe/Moscow")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        utc.iter().find(|d| d.day == expected_utc).unwrap().arrived,
        1,
        "nanoseconds must not round an arrival into tomorrow"
    );
    assert!(
        moscow
            .iter()
            .find(|d| d.day == expected_moscow)
            .unwrap()
            .arrived
            >= 1
    );
    assert_eq!(utc.iter().map(|d| d.arrived).sum::<u32>(), 4);
    assert_eq!(moscow.iter().map(|d| d.arrived).sum::<u32>(), 4);
    assert_eq!(
        repository
            .article(workspace.id(), boundary.id)
            .await
            .unwrap()
            .first_arrived_at,
        boundary.first_arrived_at,
        "calendar aggregation never modifies source precision"
    );
    // History uses read events and current read state, never publication/arrival dates.
    let ids: Vec<String> = sqlx::query_scalar("SELECT DISTINCT article_id FROM article_read_events WHERE workspace_id=$1 ORDER BY article_id")
        .bind(workspace.id().as_uuid().to_string()).fetch_all(pool).await.unwrap();
    assert_eq!(ids.len(), 3);
    let start = chrono::DateTime::parse_from_rfc3339("2026-09-28T21:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end = start + ChronoDuration::days(1);
    for (id, at) in [
        (&ids[0], start),
        (&ids[1], end - ChronoDuration::seconds(1)),
        (&ids[2], end),
    ] {
        sqlx::query(
            "UPDATE article_read_events SET occurred_at=$3 WHERE workspace_id=$1 AND article_id=$2",
        )
        .bind(workspace.id().as_uuid().to_string())
        .bind(id)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
    }
    let first_id = ArticleId::from_uuid(Uuid::parse_str(&ids[0]).unwrap());
    let request = |cursor, direction| {
        reader_application::ArticlePageRequest::new(
            ArticleScope::Feed,
            cursor,
            direction,
            SelectionLimit::new(1).unwrap(),
        )
        .unwrap()
        .with_read_period(Some(
            reader_application::ReadPeriod::new(start, end).unwrap(),
        ))
        .unwrap()
    };
    use reader_application::{ArticlePageCursor, ArticlePageDirection as Direction};
    let first = repository
        .article_summary_page(workspace.id(), request(None, Direction::Older))
        .await
        .unwrap();
    assert_eq!(
        first.total, 2,
        "end is exclusive and repeated events collapse per article"
    );
    assert_eq!(first.articles[0].article.id.as_uuid().to_string(), ids[1]);
    assert!(first.has_older);
    let cursor = |row: &reader_application::ArticlePresentation| ArticlePageCursor {
        arrived_at: row.marked_read_at.unwrap(),
        article_id: row.article.id,
    };
    let second = repository
        .article_summary_page(
            workspace.id(),
            request(Some(cursor(&first.articles[0])), Direction::Older),
        )
        .await
        .unwrap();
    assert_eq!(second.articles[0].article.id, first_id);
    assert!(
        second.articles[0].article.state.read,
        "history contains currently read articles"
    );
    assert!(!second.has_older);
    assert_eq!(second.articles[0].marked_read_at, Some(start));
    let back = repository
        .article_summary_page(
            workspace.id(),
            request(Some(cursor(&second.articles[0])), Direction::Newer),
        )
        .await
        .unwrap();
    assert_eq!(back.articles[0].article.id, first.articles[0].article.id);
    let foreign = repository
        .article_summary_page(foreign_workspace.id(), request(None, Direction::Older))
        .await
        .unwrap();
    assert!(
        foreign.articles.is_empty(),
        "history must stay workspace-scoped"
    );
    let before_events = events(pool.clone(), workspace.id().as_uuid().to_string()).await;
    let mut unread = repository.article(workspace.id(), first_id).await.unwrap();
    let previous = unread.revision;
    unread.state.read = false;
    unread.revision += 1;
    repository
        .save_article(workspace.id(), Some(previous), unread)
        .await
        .unwrap();

    let refreshed = repository
        .article_summary_page(workspace.id(), request(None, Direction::Older))
        .await
        .unwrap();
    assert_eq!(
        refreshed.total, 1,
        "mark unread removes the article after refresh"
    );
    assert_eq!(
        refreshed.articles[0].article.id,
        first.articles[0].article.id
    );
    let missing = repository
        .article_summary_page(
            workspace.id(),
            request(Some(cursor(&first.articles[0])), Direction::Older),
        )
        .await
        .unwrap();
    assert!(
        missing.articles.is_empty(),
        "old cursors cannot expose unread articles"
    );
    let feed = repository
        .article_summary_page(
            workspace.id(),
            reader_application::ArticlePageRequest::new(
                ArticleScope::Feed,
                None,
                Direction::Older,
                SelectionLimit::new(50).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        feed.articles
            .iter()
            .any(|row| row.article.id == first_id && !row.article.state.read),
        "marked-unread article returns to the ordinary Feed"
    );
    let today = Utc::now();
    sqlx::query("UPDATE article_read_events SET occurred_at=$2 WHERE workspace_id=$1")
        .bind(workspace.id().as_uuid().to_string())
        .bind(today)
        .execute(pool)
        .await
        .unwrap();
    let activity = repository
        .reading_activity(workspace.id(), "UTC")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        activity.iter().map(|day| day.count).sum::<u32>(),
        2,
        "dashboard also excludes the unread article"
    );
    assert_eq!(
        events(pool.clone(), workspace.id().as_uuid().to_string()).await,
        before_events,
        "raw read events are preserved"
    );
}
