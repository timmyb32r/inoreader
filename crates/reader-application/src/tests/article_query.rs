use super::*;
#[test]
fn scope_and_limits_reject_invalid_execution_inputs() {
    let id = SubscriptionId::new();
    for (view, subscription) in [
        ("invalid", None),
        ("feed", Some(id)),
        ("later", Some(id)),
        ("subscription", None),
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
