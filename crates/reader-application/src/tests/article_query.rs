use super::*;
#[test]
fn scope_and_limits_reject_invalid_execution_inputs() {
    let id = SubscriptionId::new();
    for (view, subscription) in [
        ("invalid", None),
        ("feed", Some(id)),
        ("later", Some(id)),
        ("subscription", None),
        ("subscription-unread", None),
    ] {
        assert!(ArticleScope::from_wire(view, subscription).is_err());
    }
    assert_eq!(
        ArticleScope::from_wire("subscription", Some(id)).unwrap(),
        ArticleScope::Subscription(id)
    );
    assert!(SelectionLimit::new(0).is_err());
    assert!(SelectionLimit::new(usize::MAX).is_err());
    let limit = SelectionLimit::new(50).unwrap();
    assert_eq!(limit.lookahead(), 51);
    assert!(
        ArticlePageRequest::new(ArticleScope::Feed, None, ArticlePageDirection::Newer, limit)
            .is_err()
    );
    let request = ArticlePageRequest::new(
        ArticleScope::Later,
        None,
        ArticlePageDirection::Older,
        limit,
    )
    .unwrap();
    assert_eq!(request.clone().scope(), ArticleScope::Later);
    assert_eq!(request.clone().limit().get(), 50);
}

#[test]
fn read_period_is_validated_and_cannot_leak_into_other_scopes() {
    let start = chrono::DateTime::parse_from_rfc3339("2026-09-28T21:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let end = start + chrono::Duration::days(1);
    assert!(ReadPeriod::new(start + chrono::Duration::nanoseconds(1), end).is_err());
    assert!(ReadPeriod::new(start, start).is_err());
    assert!(ReadPeriod::new(end, start).is_err());
    let period = ReadPeriod::new(start, end).unwrap();
    assert_eq!(period.start(), start);
    assert_eq!(period.end(), end);
    for scope in [
        ArticleScope::Later,
        ArticleScope::Subscription(SubscriptionId::new()),
    ] {
        assert!(ArticlePageRequest::new(
            scope,
            None,
            ArticlePageDirection::Older,
            SelectionLimit::new(1).unwrap()
        )
        .unwrap()
        .with_read_period(Some(period))
        .is_err());
    }
    let request = ArticlePageRequest::new(
        ArticleScope::Feed,
        None,
        ArticlePageDirection::Older,
        SelectionLimit::new(1).unwrap(),
    )
    .unwrap()
    .with_read_period(Some(period))
    .unwrap();
    assert_eq!(request.clone().read_period(), Some(period));
    assert_eq!(request.with_read_period(None).unwrap().read_period(), None);
}

#[test]
fn unread_subscription_scope_preserves_identity() {
    let id = SubscriptionId::new();
    let scope = ArticleScope::from_wire("subscription-unread", Some(id)).unwrap();
    assert_eq!(scope, ArticleScope::SubscriptionUnread(id));
    assert_eq!(scope.subscription(), Some(id));
}

#[test]
fn commit_project_is_validated_and_cannot_be_combined_with_history_or_other_scopes() {
    let project = CommitProject::from_url("https://github.com/acme/engine/commit/abc").unwrap();
    assert_eq!(
        project.clone().prefixes()[0],
        "https://github.com/acme/engine/commit/"
    );
    for invalid in [
        "https://github.com.evil/acme/engine/commit/abc",
        "https://secret@github.com/acme/engine/commit/abc",
        "https://github.com/acme/engine/issues/1",
        "https://github.com/acme/engine/commit/",
        "https://github.com/acme/old/../engine/commit/abc",
        "https://github.com/acme/%65ngine/commit/abc",
        "ftp://github.com/acme/engine/commit/abc",
    ] {
        assert!(CommitProject::from_url(invalid).is_err(), "{invalid}");
    }
    let request = || {
        ArticlePageRequest::new(
            ArticleScope::Feed,
            None,
            ArticlePageDirection::Older,
            SelectionLimit::new(2).unwrap(),
        )
        .unwrap()
    };
    let now = Utc::now();
    let now = DateTime::from_timestamp_micros(now.timestamp_micros()).unwrap();
    let period = ReadPeriod::new(now, now + chrono::Duration::days(1)).unwrap();
    assert!(request()
        .with_read_period(Some(period))
        .unwrap()
        .with_commit_project(project.clone())
        .is_err());
    assert!(request()
        .with_commit_project(project.clone())
        .unwrap()
        .with_read_period(Some(period))
        .is_err());
    assert!(ArticlePageRequest::new(
        ArticleScope::Later,
        None,
        ArticlePageDirection::Older,
        SelectionLimit::new(2).unwrap()
    )
    .unwrap()
    .with_commit_project(project.clone())
    .is_err());
    assert_eq!(
        request()
            .with_commit_project(project.clone())
            .unwrap()
            .clone()
            .commit_project(),
        Some(&project)
    );
}
