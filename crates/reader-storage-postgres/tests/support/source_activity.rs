use super::*;
pub async fn verify(
    pool: &PgPool,
    repository: &PostgresRepository,
    workspace: WorkspaceId,
    foreign: WorkspaceId,
) {
    let today = Utc::now().date_naive();
    let range = reader_application::SourceActivityPeriod::new(today, today).unwrap();
    let days = repository
        .source_activity(workspace, "UTC", range)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!(days[0].total, 3);
    assert_eq!(days[0].sources[0].subscription_id, None);
    assert_eq!(days[0].sources[0].count, 3);
    assert!(repository
        .source_activity(foreign, "UTC", range)
        .await
        .unwrap()
        .unwrap()
        .is_empty());
    assert!(repository
        .source_activity(workspace, "invalid", range)
        .await
        .unwrap()
        .is_none());
    let articles = repository.articles_by_workspace(workspace).await.unwrap();
    let mut subscriptions = Vec::new();
    for name in ["Source A", "Source B"] {
        let subscription = Subscription::new(
            SubscriptionId::new(),
            workspace,
            url::Url::parse(&format!(
                "https://example.test/source/{}",
                subscriptions.len()
            ))
            .unwrap(),
            name.into(),
        );
        repository
            .save_subscription(None, subscription.clone())
            .await
            .unwrap();
        subscriptions.push(subscription);
    }
    // One article, two subscriptions, two source records from each: 2 contributions, not 4.
    for subscription in &subscriptions {
        for _ in 0..2 {
            sqlx::query("INSERT INTO library_origins(workspace_id,article_id,subscription_id,source_record_id) VALUES($1,$2,$3,$4)")
                .bind(workspace.as_uuid().to_string()).bind(articles[0].id.as_uuid().to_string())
                .bind(subscription.id().as_uuid().to_string()).bind(Uuid::new_v4().to_string())
                .execute(pool).await.unwrap();
        }
    }

    // The digest selects unread articles across the entire source, not just a feed batch.
    let unread_request = |subscription| {
        reader_application::ArticlePageRequest::new(
            reader_application::ArticleScope::SubscriptionUnread(subscription),
            None,
            reader_application::ArticlePageDirection::Older,
            reader_application::SelectionLimit::new(50).unwrap(),
        )
        .unwrap()
    };
    let selected = repository
        .article_summary_page(workspace, unread_request(subscriptions[0].id()))
        .await
        .unwrap();
    assert_eq!(selected.total, 1);
    assert_eq!(
        selected.articles.len(),
        1,
        "duplicate origins must not duplicate an article"
    );
    assert_eq!(selected.articles[0].article.id, articles[0].id);
    assert!(repository
        .article_summary_page(foreign, unread_request(subscriptions[0].id()))
        .await
        .unwrap()
        .articles
        .is_empty());
    let mut read = articles[0].clone();
    read.state.read = true;
    read.revision += 1;
    repository
        .save_article(workspace, Some(articles[0].revision), read.clone())
        .await
        .unwrap();
    assert!(repository
        .article_summary_page(workspace, unread_request(subscriptions[0].id()))
        .await
        .unwrap()
        .articles
        .is_empty());
    read.state.read = false;
    read.revision += 1;
    repository
        .save_article(workspace, Some(read.revision - 1), read)
        .await
        .unwrap();
    let days = repository
        .source_activity(workspace, "UTC", range)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(days[0].total, 3);
    assert_eq!(days[0].sources.len(), 3);
    assert_eq!(
        days[0]
            .sources
            .iter()
            .map(|source| source.count)
            .sum::<u32>(),
        4
    );
    for source in &days[0].sources {
        assert_eq!(
            source.count,
            if source.subscription_id.is_none() {
                2
            } else {
                1
            }
        );
    }
    // Exact subsecond arrival remains on the previous local calendar day.
    let mut boundary = articles[1].clone();
    boundary.first_arrived_at = format!("{}T20:59:59.999999999Z", today.pred_opt().unwrap())
        .parse()
        .unwrap();
    boundary.revision += 1;
    repository
        .save_article(workspace, Some(articles[1].revision), boundary.clone())
        .await
        .unwrap();
    let days = repository
        .source_activity(
            workspace,
            "Europe/Moscow",
            reader_application::SourceActivityPeriod::new(today, today.succ_opt().unwrap())
                .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(days.iter().map(|day| day.total).sum::<u32>(), 2);
    boundary.first_arrived_at = articles[1].first_arrived_at;
    boundary.revision += 1;
    repository
        .save_article(workspace, Some(boundary.revision - 1), boundary)
        .await
        .unwrap();
    // Remove this check's read event before the shared bulk-read assertions.
    sqlx::query("DELETE FROM article_read_events WHERE workspace_id=$1 AND article_id=$2")
        .bind(workspace.as_uuid().to_string())
        .bind(articles[0].id.as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
    // Remove fixture provenance so subsequent bulk-read checks retain their setup.
    sqlx::query("DELETE FROM library_origins WHERE workspace_id=$1")
        .bind(workspace.as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
}
