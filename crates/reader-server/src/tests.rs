use super::*;
use reader_application::Argon2idPolicy;
use reader_core::{StateEvent, WorkspaceStateEvent};

struct NoDiscovery;
#[async_trait::async_trait]
impl FeedDiscovery for NoDiscovery {
    async fn discover(&self, _: Url) -> Result<FeedPreviewResponse, String> {
        unreachable!()
    }
    async fn preview_web_feed(
        &self,
        _: &WebFeedRecipeDraft,
    ) -> Result<FeedPreviewResponse, String> {
        unreachable!()
    }
}

fn state(origin: &str) -> AppState<()> {
    AppState::new(
        Arc::new(()),
        Arc::new(NoDiscovery),
        ReasonPolicy::new(128).unwrap(),
        AuthPolicy {
            session_lifetime_seconds: 3600,
            invite_lifetime_seconds: 3600,
            reset_lifetime_seconds: 3600,
            argon2id: Argon2idPolicy::new(19_456, 2, 1).unwrap(),
        },
        origin.into(),
        10,
        100,
    )
}

#[test]
fn session_cookie_has_browser_security_attributes() {
    let value = session_cookie("opaque", 60)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(value.contains("Secure"));
    assert!(value.contains("HttpOnly"));
    assert!(value.contains("SameSite=Strict"));
    assert!(value.contains("Path=/"));
}

#[test]
fn opml_rejects_non_http_sources() {
    let document = r#"<opml><body><outline xmlUrl="file:///etc/passwd"/></body></opml>"#;
    assert!(opml_outlines(document).is_err());
}

#[test]
fn editor_names_are_not_silently_trimmed() {
    assert!(valid_name(" workspace").is_err());
    assert!(valid_name("workspace").is_ok());
}

#[test]
fn csrf_requires_an_exact_origin_including_scheme_and_port() {
    let state = state("https://reader.example:8443");
    let mut headers = HeaderMap::new();
    assert!(matches!(csrf(&state, &headers), Err(ApiFailure::Csrf)));
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://reader.example"),
    );
    assert!(matches!(csrf(&state, &headers), Err(ApiFailure::Csrf)));
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://reader.example:8443"),
    );
    assert!(csrf(&state, &headers).is_ok());
}

#[test]
fn cookie_parser_selects_the_named_cookie_without_prefix_confusion() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static(
            "reader_session_old=wrong; theme=dark; reader_session=opaque-token",
        ),
    );
    assert_eq!(cookie(&headers, SESSION_COOKIE), Some("opaque-token"));
}

#[test]
fn opml_parses_nested_outlines_and_preserves_authored_titles() {
    let document = r#"<?xml version="1.0"?><opml><body><outline text="Folder"><outline text=" Feed A " xmlUrl="https://example.test/a.xml"/><outline title="Feed B" xmlUrl="https://example.test/b.xml"/></outline></body></opml>"#;
    assert!(
        opml_outlines(document).is_err(),
        "whitespace-changing title must be rejected rather than trimmed"
    );
    let valid = document.replace(" Feed A ", "Feed A");
    let values = opml_outlines(&valid).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].0.as_str(), "https://example.test/a.xml");
    assert_eq!(values[0].1, "Feed A");
    assert_eq!(values[1].1, "Feed B");
}

#[test]
fn opml_rejects_entity_and_doctype_declarations() {
    for document in [
        r#"<!DOCTYPE opml><opml><body/></opml>"#,
        r#"<!ENTITY x "secret"><opml><body/></opml>"#,
    ] {
        assert!(opml_outlines(document).is_err());
    }
}

#[test]
fn workspace_and_subscription_dtos_expose_reasons_without_domain_layout() {
    let reason = ReasonPolicy::new(128)
        .unwrap()
        .validate("Maintenance window".into())
        .unwrap();
    let event = WorkspaceStateEvent {
        reason,
        actor: ActorId::new(),
        at: Utc::now(),
    };
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Research".into());
    workspace.archive(event);
    let json = serde_json::to_value(workspace_view(&workspace)).unwrap();
    assert_eq!(json["archived"], true);
    assert_eq!(json["archiveReason"], "Maintenance window");
    assert!(json.get("revision").is_none());

    let mut subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    subscription.pause(StateEvent {
        reason: ReasonPolicy::new(128)
            .unwrap()
            .validate("Review".into())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc::now(),
    });
    let json = serde_json::to_value(subscription_view(
        &subscription,
        &reader_application::SubscriptionStats {
            article_count: 17,
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(json["status"], "paused");
    assert_eq!(json["reason"], "Review");
    assert_eq!(json["count"], 17);
}

#[test]
fn rule_wire_mapping_increments_version_and_rejects_ambiguous_phrases() {
    let subscription_id = SubscriptionId::new();
    let draft = RuleDraft {
        id: None,
        subscription_id: subscription_id.as_uuid(),
        field: "title_or_full_text".into(),
        phrase: "Rust".into(),
        action: "mark_read".into(),
        enabled: true,
    };
    let id = RuleId::new();
    let parsed = parse_rule(Some((id, 12)), &draft).unwrap();
    assert_eq!(parsed.id, id);
    assert_eq!(parsed.version, 12);
    assert_eq!(parsed.field, RuleField::Both);
    assert_eq!(parsed.action, RuleAction::MarkRead);
    let invalid = RuleDraft {
        phrase: " Rust ".into(),
        ..draft
    };
    assert!(parse_rule(None, &invalid).is_err());
}

#[test]
fn article_dto_keeps_url_sources_failure_reason_and_plain_paragraphs() {
    let article = reader_core::Article {
        id: ArticleId::new(),
        key: reader_core::DedupKey {
            location: reader_core::ArticleLocation::from(
                Url::parse("https://example.test/post").unwrap(),
            ),
            title: "Post".into(),
            description: Some("Excerpt".into()),
        },
        state: reader_core::ArticleState::default(),
        first_arrived_at: Utc::now(),
        origins: vec![],
        revision: 0,
    };
    let presentation = reader_application::ArticlePresentation {
        article,
        subscription_ids: vec![],
        subscription_titles: vec!["Feed A".into(), "Feed B".into()],
        safe_html: Some("<p>First</p><p>Second <strong>paragraph</strong></p>".into()),
        full_text_status: "failed",
        failure_reason: Some("upstream timeout".into()),
    };
    let json = serde_json::to_value(article_view(&presentation)).unwrap();
    assert_eq!(json["url"], "https://example.test/post");
    assert_eq!(json["source"], "Feed A");
    assert_eq!(json["sources"], serde_json::json!(["Feed A", "Feed B"]));
    assert_eq!(
        json["body"],
        serde_json::json!(["First", "Second paragraph"])
    );
    assert_eq!(json["fullText"], "failed");
    assert_eq!(json["fullTextReason"], "upstream timeout");
}
