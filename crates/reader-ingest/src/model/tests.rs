use super::*;

fn parsed(id: &str, title: &str, description: Option<&str>, html: Option<&str>) -> ParsedRecord {
    ParsedRecord {
        upstream_id: id.into(),
        original_url: "https://example.test/a".into(),
        absolute_url: Some(Url::parse("https://example.test/a").unwrap()),
        title: title.into(),
        description: description.map(str::to_owned),
        content_html: html.map(str::to_owned),
        published_at: None,
    }
}

#[test]
fn same_url_with_a_changed_title_requires_regrouping() {
    let value = SourceRecord::from_parsed(
        SourceRecordId::new(),
        SourceId::new(),
        parsed("id", "old", Some("same"), None),
    )
    .unwrap();
    let revised = value
        .revise(parsed("id", "new", Some("same"), None))
        .unwrap();
    assert_eq!(revised.effect, RecordRevisionEffect::RegroupAndRefresh);
    assert_eq!(revised.record.revision(), 1)
}

#[test]
fn same_url_and_title_with_a_changed_description_requires_regrouping() {
    let value = SourceRecord::from_parsed(
        SourceRecordId::new(),
        SourceId::new(),
        parsed("id", "same", Some("old"), None),
    )
    .unwrap();
    let revised = value
        .revise(parsed("id", "same", Some("new"), None))
        .unwrap();
    assert_eq!(revised.effect, RecordRevisionEffect::RegroupAndRefresh)
}

#[test]
fn only_feed_content_change_refreshes_without_changing_dedup_identity() {
    let value = SourceRecord::from_parsed(
        SourceRecordId::new(),
        SourceId::new(),
        parsed("id", "same", Some("same"), Some("old")),
    )
    .unwrap();
    let revised = value
        .revise(parsed("id", "same", Some("same"), Some("new")))
        .unwrap();
    assert_eq!(revised.effect, RecordRevisionEffect::RefreshContent);
    assert_eq!(revised.record.key(), value.key())
}

#[test]
fn identical_repeat_is_idempotent_at_the_semantic_boundary() {
    let value = SourceRecord::from_parsed(
        SourceRecordId::new(),
        SourceId::new(),
        parsed("id", "same", None, None),
    )
    .unwrap();
    assert_eq!(
        value
            .revise(parsed("id", "same", None, None))
            .unwrap()
            .effect,
        RecordRevisionEffect::Unchanged
    )
}

#[test]
fn absolute_url_spelling_is_preserved_in_dedup_identity() {
    let source = SourceId::new();
    let mut upper = parsed("upper", "same", None, None);
    upper.original_url = "https://EXAMPLE.test/a".into();
    let mut lower = parsed("lower", "same", None, None);
    lower.original_url = "https://example.test/a".into();
    let upper = SourceRecord::from_parsed(SourceRecordId::new(), source, upper).unwrap();
    let lower = SourceRecord::from_parsed(SourceRecordId::new(), source, lower).unwrap();
    assert_ne!(
        upper.key(),
        lower.key(),
        "URL parser canonicalization must not merge exact source values"
    );
    assert_eq!(
        upper.key().location.fetch_url(),
        lower.key().location.fetch_url()
    );
}

#[test]
fn revision_cannot_silently_replace_upstream_identity() {
    let value = SourceRecord::from_parsed(
        SourceRecordId::new(),
        SourceId::new(),
        parsed("first", "same", None, None),
    )
    .unwrap();
    assert_eq!(
        value
            .revise(parsed("second", "same", None, None))
            .unwrap_err(),
        ModelError::UpstreamIdentityChanged
    )
}

#[test]
fn selector_language_and_loading_are_validated_before_execution() {
    assert_eq!(
        WebSelector::new(SelectorLanguage::Css, "[".into()).unwrap_err(),
        ModelError::InvalidSelector
    );
    assert_eq!(
        WebSelector::new(SelectorLanguage::XPath, "article".into()).unwrap_err(),
        ModelError::InvalidSelector
    );
    let xpath = WebSelector::new(SelectorLanguage::XPath, "//article".into()).unwrap();
    assert_eq!(
        WebFeedRecipe::advanced(xpath, WebLoading::Static, WebFeedActions::default()).unwrap_err(),
        ModelError::XPathRequiresBrowser
    )
}

#[test]
fn action_and_page_limits_reject_oversized_recipe() {
    let page = Url::parse("https://example.test/page-2").unwrap();
    assert_eq!(
        WebFeedActions::new(
            WebViewport::Mobile,
            vec![],
            vec![page],
            None,
            None,
            0,
            0,
            1,
            3
        )
        .unwrap_err(),
        ModelError::LimitExceeded("start_pages")
    );
    assert_eq!(
        WebFeedActions::new(WebViewport::Desktop, vec![], vec![], None, None, 2, 2, 2, 3)
            .unwrap_err(),
        ModelError::LimitExceeded("actions")
    )
}
