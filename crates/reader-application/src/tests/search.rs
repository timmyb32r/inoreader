use super::*;
#[test]
fn search_validates_limits_and_closed_scope_without_changing_query() {
    let limits = SearchLimits::new(SearchLimitsInput {
        query_bytes: 20,
        page_size: 25,
        excerpt_characters: 200,
    })
    .unwrap();
    let request =
        SearchRequest::new(&limits, " 数据 %_ ".into(), SearchKind::All, None, None, 0).unwrap();
    assert_eq!(request.query(), " 数据 %_ ");
    assert!(SearchRequest::new(&limits, "x".repeat(21), SearchKind::All, None, None, 0).is_err());
    assert!(SearchRequest::new(&limits, "a\0b".into(), SearchKind::All, None, None, 0).is_err());
    assert!(SearchRequest::new(
        &limits,
        "x".into(),
        SearchKind::News,
        Some(Uuid::new_v4()),
        None,
        0
    )
    .is_err());
    assert!(SearchLimits::new(SearchLimitsInput {
        query_bytes: 0,
        page_size: 25,
        excerpt_characters: 200
    })
    .is_err());
}
