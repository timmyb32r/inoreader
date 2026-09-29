use super::*;
fn usage() -> Usage {
    Usage {
        prompt_tokens: 1,
        completion_tokens: 1,
        prompt_cache_hit_tokens: 0,
        prompt_cache_miss_tokens: 1,
        estimated_cost_usd: None,
    }
}
fn draft() -> String {
    serde_json::json!({"segments":[{"kind":"text","content":"**Source title**"},{"kind":"text","content":"Engine stores 12 GB. It is optional."},{"kind":"quote","content":"12 GB"}]}).to_string()
}
fn review(raw: &str) -> Result<CompletedGeneration, AiError> {
    CompletedGeneration::reviewed(
        raw.into(),
        usage(),
        &draft(),
        "Source title",
        "Source says 12 GB; 14 GB is false.",
        10000,
    )
}
#[test]
fn unchanged_preserves_text_and_exact_response() {
    let raw = "{ \"verdict\": \"unchanged\" }";
    let result = review(raw).unwrap();
    assert_eq!(result.envelope(), raw);
    assert_eq!(
        result.content(),
        "**Source title**\n\nEngine stores 12 GB. It is optional.\n\n> 12 GB"
    );
}
#[test]
fn patches_use_original_anchors_preserve_style_and_allow_explicit_deletion() {
    let result = review(r#"{"verdict":"corrections","changes":[{"segment":1,"before":"12 GB","after":"14 GB","reason":"Correction"},{"segment":1,"before":" It is optional.","after":"","reason":"Unsupported"}]}"#).unwrap();
    assert_eq!(
        result.content(),
        "**Source title**\n\nEngine stores 14 GB.\n\n> 12 GB"
    );
}
#[test]
fn invalid_review_never_partially_publishes_or_changes_title_or_quotes() {
    for raw in [
        r#"{"verdict":"corrections","changes":[]}"#,
        r#"{"verdict":"unchanged","changes":[]}"#,
        r#"{"verdict":"unable","reason":"insufficient information"}"#,
        r#"{"segments":[]}"#,
        r#"{"verdict":"corrections","changes":[{"segment":0,"before":"Source","after":"Other","reason":"title"}]}"#,
        r#"{"verdict":"corrections","changes":[{"segment":1,"before":"missing","after":"x","reason":"y"}]}"#,
        r#"{"verdict":"corrections","changes":[{"segment":2,"before":"12 GB","after":"invented quote","reason":"y"}]}"#,
        r#"{"verdict":"corrections","changes":[{"segment":1,"before":"12 GB","after":"14 GB","reason":"y"},{"segment":1,"before":"GB","after":"GiB","reason":"y"}]}"#,
        r#"{"verdict":"corrections","changes":[{"segment":1,"before":"12 GB","after":"14 GB","reason":"y"},{"segment":99,"before":"a","after":"b","reason":"y"}]}"#,
    ] {
        assert!(review(raw).is_err(), "{raw}");
    }
}
#[test]
fn ambiguous_anchor_including_overlapping_occurrences_is_rejected() {
    let raw = r#"{"verdict":"corrections","changes":[{"segment":0,"before":"aa","after":"b","reason":"y"}]}"#;
    for text in ["aaa", "aa aa"] {
        let draft = serde_json::json!({"segments":[{"kind":"text","content":text}]}).to_string();
        assert!(CompletedGeneration::reviewed(
            raw.into(),
            usage(),
            &draft,
            "title",
            "source",
            10000
        )
        .is_err());
    }
}
#[test]
fn application_adds_exact_unicode_heading_without_model_title_validation() {
    let draft = r#"{"segments":[{"kind":"text","content":"Body without heading"}]}"#;
    let result = CompletedGeneration::new(draft.into(), usage(), "source", 10000)
        .unwrap()
        .with_source_title("Original\u{00a0}title", "source", 10000)
        .unwrap();
    assert_eq!(
        result.content(),
        "**Original\u{00a0}title**\n\nBody without heading"
    );
}
