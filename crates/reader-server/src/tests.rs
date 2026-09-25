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
fn cross_user_resource_ids_are_indistinguishable_from_missing_ids() {
    let owner = AccountId::new();
    let other = AccountId::new();
    assert!(require_owner(owner, owner).is_ok());
    assert!(matches!(
        require_owner(owner, other),
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    ));
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
            unread_count: 4,
            ..Default::default()
        },
    ))
    .unwrap();
    assert_eq!(json["status"], "paused");
    assert_eq!(json["reason"], "Review");
    assert_eq!(json["count"], 17);
    assert_eq!(json["unreadCount"], 4);
    assert_eq!(json["sourceTitle"], "Feed");
    assert_eq!(json["customName"], serde_json::Value::Null);
    assert_eq!(json["personalNote"], "");
    assert_eq!(json["sourceUrl"], "https://example.test/feed");
    assert_eq!(json["sourceType"], "feed");
    assert_eq!(json["needsAttention"], false);
}

#[test]
fn subscription_attention_is_explained_after_three_failures() {
    let subscription = Subscription::new(
        SubscriptionId::new(),
        WorkspaceId::new(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    let view = subscription_view(
        &subscription,
        &reader_application::SubscriptionStats {
            consecutive_failures: 3,
            error: Some("upstream timeout".into()),
            ..Default::default()
        },
    );
    assert!(view.needs_attention);
    assert_eq!(view.attention_reason.as_deref(), Some("upstream timeout"));
}

#[test]
fn incomplete_refresh_needs_attention_without_lowering_failure_threshold() {
    let subscription = Subscription::new(
        SubscriptionId::new(),
        WorkspaceId::new(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    let incomplete = subscription_view(
        &subscription,
        &reader_application::SubscriptionStats {
            incomplete: true,
            consecutive_failures: 2,
            ..Default::default()
        },
    );
    assert!(incomplete.needs_attention);
    assert!(incomplete.attention_reason.unwrap().contains("incomplete"));
    let ordinary = subscription_view(
        &subscription,
        &reader_application::SubscriptionStats {
            consecutive_failures: 2,
            ..Default::default()
        },
    );
    assert!(!ordinary.needs_attention);
}

#[test]
fn web_feed_extraction_summary_uses_the_effective_selectors() {
    let draft: WebFeedRecipeDraft = serde_json::from_value(serde_json::json!({
        "workspaceId": Uuid::new_v4(),
        "url": "https://example.test",
        "selector": ".fallback",
        "loading": "static",
        "cardSelector": ".card",
        "titleSelector": "h2",
        "contentSelector": ".body"
    }))
    .unwrap();
    assert_eq!(
        web_feed_recipe_summary(&draft),
        "Cards: .card; title: h2; content: .body"
    );
}

#[test]
fn web_feed_url_change_is_rejected_before_preview() {
    let stats = reader_application::SubscriptionStats {
        editable_web_feed: true,
        source_type: "web".into(),
        ..Default::default()
    };
    assert!(matches!(
        ensure_source_url_change_allowed(&stats),
        Err(ApiFailure::Validation(_))
    ));
}

#[test]
fn add_subscription_ignores_the_client_title_in_favor_of_server_discovery() {
    assert_eq!(
        trusted_discovery_title(Some("Untrusted title"), "Server discovery".into()),
        "Server discovery"
    );
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

use chrono::DateTime;
use reader_application::{
    token_hash, AccountRecord, ArticlePresentation, InviteRecord, PasswordResetRecord,
    RuleApplicationProgress, SeedSource, SourceUrlPreviewRecord,
};
use reader_core::*;
use std::sync::Mutex;
use tower::ServiceExt;

struct RouteRepository {
    workspace: Workspace,
    subscription: Subscription,
    saved_workspace: Mutex<Vec<(Option<u64>, Workspace)>>,
    saved_subscription: Mutex<Vec<(Option<u64>, Subscription)>>,
    refreshes: Mutex<Vec<SubscriptionId>>,
    created_account_workspace: Mutex<Vec<(AccountRecord, Workspace)>>,
    account: AccountRecord,
    session: SessionRecord,
    preview: Option<SourceUrlPreviewRecord>,
}

impl RouteRepository {
    fn new(
        workspace: Workspace,
        subscription: Subscription,
        account: AccountRecord,
        session: SessionRecord,
        preview: Option<SourceUrlPreviewRecord>,
    ) -> Self {
        Self {
            workspace,
            subscription,
            saved_workspace: Mutex::new(vec![]),
            saved_subscription: Mutex::new(vec![]),
            refreshes: Mutex::new(vec![]),
            created_account_workspace: Mutex::new(vec![]),
            account,
            session,
            preview,
        }
    }
}

fn route_fixture(
    workspace_owner: AccountId,
    preview_subscription: Option<SubscriptionId>,
) -> (AppState<RouteRepository>, HeaderMap, Subscription) {
    let account_id = AccountId::new();
    let account = AccountRecord {
        id: account_id,
        username: "reader".into(),
        password_hash: "unused".into(),
        admin: false,
        auth_revision: 3,
        revision: 0,
    };
    let session = SessionRecord {
        id: Uuid::new_v4(),
        verifier_hash: token_hash("route-token"),
        account_id,
        account_auth_revision: account.auth_revision,
        expires_at: Utc::now() + chrono::Duration::hours(1),
        revision: 0,
    };
    let workspace = Workspace::new(WorkspaceId::new(), workspace_owner, "Private".into());
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    let preview = preview_subscription.map(|subscription_id| SourceUrlPreviewRecord {
        id: Uuid::new_v4(),
        subscription_id,
        url: "https://example.test/new-feed".into(),
        source_title: "New feed".into(),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
        revision: 0,
    });
    let repository = Arc::new(RouteRepository::new(
        workspace,
        subscription.clone(),
        account,
        session,
        preview,
    ));
    let state = AppState::new(
        repository,
        Arc::new(NoDiscovery),
        ReasonPolicy::new(128).unwrap(),
        AuthPolicy {
            session_lifetime_seconds: 3600,
            invite_lifetime_seconds: 3600,
            reset_lifetime_seconds: 3600,
            argon2id: Argon2idPolicy::new(19_456, 2, 1).unwrap(),
        },
        "https://reader.test".into(),
        10,
        100,
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("reader_session=route-token"),
    );
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://reader.test"),
    );
    (state, headers, subscription)
}

#[tokio::test]
async fn subscription_routes_hide_cross_user_detail_and_mutations() {
    let (state, headers, subscription) = route_fixture(AccountId::new(), None);
    let id = subscription.id().as_uuid();
    assert!(matches!(
        get_subscription(State(state.clone()), headers.clone(), Path(id)).await,
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    ));
    assert!(matches!(
        save_subscription_note(
            State(state.clone()),
            headers.clone(),
            Path(id),
            Json(SaveSubscriptionNoteRequest {
                note: "secret".into()
            })
        )
        .await,
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    ));
    assert!(matches!(
        preview_subscription_source_url(
            State(state.clone()),
            headers.clone(),
            Path(id),
            Json(PreviewSourceUrlRequest {
                url: "https://example.test/new".into()
            })
        )
        .await,
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    ));
    assert!(matches!(
        commit_subscription_source_url(
            State(state),
            headers,
            Path(id),
            Json(CommitSourceUrlRequest {
                preview_token: Uuid::new_v4()
            })
        )
        .await,
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    ));
}

#[tokio::test]
async fn router_returns_not_found_for_cross_user_subscription_routes() {
    let (state, _, subscription) = route_fixture(AccountId::new(), None);
    let app = router(state);
    let id = subscription.id().as_uuid();
    for (method, uri, body) in [
        ("GET", format!("/api/subscriptions/{id}"), None),
        ("GET", format!("/api/subscriptions/{id}/activity"), None),
        ("GET", format!("/api/subscriptions/{id}/extraction"), None),
        (
            "PUT",
            format!("/api/subscriptions/{id}/note"),
            Some(r#"{"note":"private"}"#),
        ),
        (
            "POST",
            format!("/api/subscriptions/{id}/source-url/preview"),
            Some(r#"{"url":"https://example.test/new"}"#),
        ),
        (
            "PUT",
            format!("/api/subscriptions/{id}/source-url"),
            Some(r#"{"previewToken":"00000000-0000-0000-0000-000000000001"}"#),
        ),
    ] {
        let request = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, "reader_session=route-token")
            .header(header::ORIGIN, "https://reader.test")
            .header(header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body.unwrap_or_default()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn source_url_commit_hides_token_for_another_subscription() {
    let owner = AccountId::new();
    let foreign_subscription = SubscriptionId::new();
    let (state, headers, subscription) = route_fixture(owner, Some(foreign_subscription));
    // Align the workspace owner with the authenticated account stored by the fixture.
    let actor = auth(&state, &headers).await.unwrap();
    let owned_workspace = Workspace::new(
        state.repository.workspace.id(),
        actor.account.id,
        "Private".into(),
    );
    let preview = state.repository.preview.clone().unwrap();
    let repository = Arc::new(RouteRepository::new(
        owned_workspace,
        subscription.clone(),
        state.repository.account.clone(),
        state.repository.session.clone(),
        Some(preview.clone()),
    ));
    let owned_state = AppState::new(
        repository,
        Arc::new(NoDiscovery),
        state.reason_policy,
        state.auth_policy,
        state.external_origin.clone(),
        10,
        100,
    );
    assert!(matches!(
        commit_subscription_source_url(
            State(owned_state),
            headers,
            Path(subscription.id().as_uuid()),
            Json(CommitSourceUrlRequest {
                preview_token: preview.id
            })
        )
        .await,
        Err(ApiFailure::Repository(RepositoryError::NotFound))
    ));
}

fn route_unused<T>() -> Result<T, RepositoryError> {
    Err(RepositoryError::NotFound)
}
#[async_trait::async_trait]
impl ReaderRepository for RouteRepository {
    async fn readiness(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
    async fn save_job(&self, _: Option<u64>, _: DurableJob) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn account(&self, id: AccountId) -> Result<AccountRecord, RepositoryError> {
        if id == self.account.id {
            Ok(self.account.clone())
        } else {
            route_unused()
        }
    }
    async fn account_by_username(&self, _: &str) -> Result<AccountRecord, RepositoryError> {
        route_unused()
    }
    async fn save_account(&self, _: Option<u64>, _: AccountRecord) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn create_account_and_workspace(
        &self,
        account: AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError> {
        self.created_account_workspace
            .lock()
            .unwrap()
            .push((account, workspace));
        Ok(())
    }
    async fn invite_by_token_hash(&self, _: &str) -> Result<InviteRecord, RepositoryError> {
        route_unused()
    }
    async fn save_invite(&self, _: Option<u64>, _: InviteRecord) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn session_by_verifier_hash(&self, hash: &str) -> Result<SessionRecord, RepositoryError> {
        if hash == self.session.verifier_hash {
            Ok(self.session.clone())
        } else {
            route_unused()
        }
    }
    async fn save_session(&self, _: Option<u64>, _: SessionRecord) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn delete_session(&self, _: &str, _: u64) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn password_reset_by_token_hash(
        &self,
        _: &str,
    ) -> Result<PasswordResetRecord, RepositoryError> {
        route_unused()
    }
    async fn save_password_reset(
        &self,
        _: Option<u64>,
        _: PasswordResetRecord,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn consume_invite_create_account_and_workspace(
        &self,
        _: u64,
        _: InviteRecord,
        _: AccountRecord,
        _: Workspace,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn consume_reset_and_update_account(
        &self,
        _: u64,
        _: PasswordResetRecord,
        _: u64,
        _: AccountRecord,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn workspace(&self, id: WorkspaceId) -> Result<Workspace, RepositoryError> {
        if id == self.workspace.id() {
            Ok(self.workspace.clone())
        } else {
            route_unused()
        }
    }
    async fn workspaces_by_owner(&self, _: AccountId) -> Result<Vec<Workspace>, RepositoryError> {
        route_unused()
    }
    async fn save_workspace(
        &self,
        expected: Option<u64>,
        value: Workspace,
    ) -> Result<(), RepositoryError> {
        self.saved_workspace.lock().unwrap().push((expected, value));
        Ok(())
    }
    async fn restore_workspace_with_refreshes(
        &self,
        expected: u64,
        value: Workspace,
        active: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        self.saved_workspace
            .lock()
            .unwrap()
            .push((Some(expected), value));
        self.refreshes
            .lock()
            .unwrap()
            .extend(active.into_iter().map(|subscription| subscription.id()));
        Ok(())
    }
    async fn subscription(&self, id: SubscriptionId) -> Result<Subscription, RepositoryError> {
        if id == self.subscription.id() {
            Ok(self.subscription.clone())
        } else {
            route_unused()
        }
    }
    async fn subscriptions_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Subscription>, RepositoryError> {
        if workspace == self.workspace.id() {
            Ok(vec![self.subscription.clone()])
        } else {
            route_unused()
        }
    }
    async fn subscription_stats(
        &self,
        _: WorkspaceId,
        values: &[Subscription],
    ) -> Result<
        std::collections::HashMap<SubscriptionId, reader_application::SubscriptionStats>,
        RepositoryError,
    > {
        Ok(values
            .iter()
            .map(|v| (v.id(), reader_application::SubscriptionStats::default()))
            .collect())
    }
    async fn save_subscription(
        &self,
        expected: Option<u64>,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        self.saved_subscription
            .lock()
            .unwrap()
            .push((expected, value));
        Ok(())
    }
    async fn activate_subscription_with_refresh(
        &self,
        expected: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        self.saved_subscription
            .lock()
            .unwrap()
            .push((Some(expected), value.clone()));
        self.refreshes.lock().unwrap().push(value.id());
        Ok(())
    }
    async fn enqueue_subscription_refresh(
        &self,
        value: &Subscription,
    ) -> Result<(), RepositoryError> {
        self.refreshes.lock().unwrap().push(value.id());
        Ok(())
    }
    async fn save_web_feed_subscription(
        &self,
        _: Subscription,
        _: String,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn web_feed_recipe(&self, _: SubscriptionId) -> Result<(u64, String), RepositoryError> {
        route_unused()
    }
    async fn update_web_feed_recipe(
        &self,
        _: SubscriptionId,
        _: u64,
        _: String,
    ) -> Result<u64, RepositoryError> {
        route_unused()
    }
    async fn article(&self, _: WorkspaceId, _: ArticleId) -> Result<Article, RepositoryError> {
        route_unused()
    }
    async fn articles_by_workspace(&self, _: WorkspaceId) -> Result<Vec<Article>, RepositoryError> {
        route_unused()
    }
    async fn article_presentations_by_workspace(
        &self,
        _: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        route_unused()
    }
    async fn save_article(
        &self,
        _: WorkspaceId,
        _: Option<u64>,
        _: Article,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn enqueue_article_full_text_refresh(
        &self,
        _: WorkspaceId,
        _: &Article,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn rules_by_workspace(&self, _: WorkspaceId) -> Result<Vec<Rule>, RepositoryError> {
        route_unused()
    }
    async fn rule(&self, _: WorkspaceId, _: RuleId) -> Result<Rule, RepositoryError> {
        route_unused()
    }
    async fn save_rule(
        &self,
        _: WorkspaceId,
        _: Option<u64>,
        _: Rule,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn delete_rule(&self, _: WorkspaceId, _: RuleId, _: u64) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn enqueue_rule_application(
        &self,
        _: WorkspaceId,
        _: Rule,
    ) -> Result<Uuid, RepositoryError> {
        route_unused()
    }
    async fn rule_application_progress(
        &self,
        _: WorkspaceId,
        _: Uuid,
    ) -> Result<RuleApplicationProgress, RepositoryError> {
        route_unused()
    }
    async fn record_login_attempt(
        &self,
        _: &str,
        _: DateTime<Utc>,
        _: u32,
    ) -> Result<bool, RepositoryError> {
        route_unused()
    }
    async fn mark_articles_read_atomic(
        &self,
        _: WorkspaceId,
        _: Vec<Article>,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn import_subscriptions_atomic(
        &self,
        _: WorkspaceId,
        _: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn apply_seed_atomic(
        &self,
        _: WorkspaceId,
        _: Vec<(String, Subscription, SeedSource, serde_json::Value)>,
    ) -> Result<(), RepositoryError> {
        route_unused()
    }
    async fn source_url_preview(
        &self,
        id: Uuid,
    ) -> Result<SourceUrlPreviewRecord, RepositoryError> {
        self.preview
            .clone()
            .filter(|v| v.id == id)
            .ok_or(RepositoryError::NotFound)
    }
}
