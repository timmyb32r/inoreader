use super::*;

#[test]
fn parses_relative_json_feed_url_without_rewriting_query_or_fragment() {
    let records = parse_json(
        br#"{"items":[{"id":"1","url":"/a?q=1#x","title":"T","summary":""}]}"#,
        &Url::parse("https://example.test/feed").unwrap(),
    )
    .unwrap();
    assert_eq!(
        records[0].absolute_url.as_ref().unwrap().as_str(),
        "https://example.test/a?q=1#x"
    );
    assert_eq!(records[0].description.as_deref(), Some(""));
}

#[test]
fn preserves_url_less_entry_when_stable_identity_exists() {
    let records = parse_json(
        br#"{"items":[{"id":"stable","title":"T"}]}"#,
        &Url::parse("https://example.test/feed").unwrap(),
    )
    .unwrap();
    assert_eq!(records[0].upstream_id, "stable");
    assert_eq!(records[0].absolute_url, None);
}
