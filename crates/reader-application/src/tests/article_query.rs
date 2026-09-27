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
