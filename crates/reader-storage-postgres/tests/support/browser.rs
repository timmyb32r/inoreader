use super::*;
use reader_application::{token_hash, Argon2idPolicy, AuthPolicy, SessionRecord};
use std::sync::Arc;
struct NoDiscovery;
#[async_trait::async_trait]
impl reader_server::FeedDiscovery for NoDiscovery {
    async fn discover(
        &self,
        _: url::Url,
    ) -> Result<reader_server_contracts::FeedPreviewResponse, String> {
        Err("No network in acceptance fixture".into())
    }
    async fn preview_web_feed(
        &self,
        _: &reader_server_contracts::WebFeedRecipeDraft,
    ) -> Result<reader_server_contracts::FeedPreviewResponse, String> {
        Err("No network in acceptance fixture".into())
    }
}
pub async fn verify(pool: &PgPool) {
    let repository = Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20).unwrap(),
    );
    let mut fixtures = Vec::new();
    for label in ["Browser owner", "Private owner"] {
        let account = AccountRecord {
            id: AccountId::new(),
            username: Uuid::new_v4().to_string(),
            password_hash: "unused-test-only".into(),
            admin: false,
            auth_revision: 0,
            revision: 0,
        };
        let workspace = Workspace::new(WorkspaceId::new(), account.id, label.into());
        repository
            .create_account_and_workspace(account.clone(), workspace.clone())
            .await
            .unwrap();
        let article = Article {
            id: ArticleId::new(),
            key: DedupKey {
                location: ArticleLocation::from(
                    url::Url::parse("https://example.test/browser").unwrap(),
                ),
                title: "Persistent browser article".into(),
                description: Some("Exact source text".into()),
            },
            state: ArticleState::default(),
            first_arrived_at: Utc::now(),
            origins: vec![],
            revision: 0,
        };
        repository
            .save_article(workspace.id(), None, article.clone())
            .await
            .unwrap();
        fixtures.push((account, workspace, article));
    }
    let (account, workspace, article) = &fixtures[0];
    let token = Uuid::new_v4().to_string();
    repository
        .save_session(
            None,
            SessionRecord {
                id: Uuid::new_v4(),
                verifier_hash: token_hash(&token),
                account_id: account.id,
                account_auth_revision: 0,
                expires_at: Utc::now() + ChronoDuration::minutes(10),
                revision: 0,
            },
        )
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let state = reader_server::AppState::new(
        repository.clone(),
        Arc::new(NoDiscovery),
        ReasonPolicy::new(256).unwrap(),
        AuthPolicy {
            session_lifetime_seconds: 600,
            invite_lifetime_seconds: 600,
            reset_lifetime_seconds: 600,
            argon2id: Argon2idPolicy::new(19456, 2, 1).unwrap(),
        },
        origin.clone(),
        100,
        1000,
    );
    let app = reader_server::router(state).fallback(|uri: axum::http::Uri| async move {
        match reader_server_ui::asset(uri.path()) {
            Some(asset) => axum::http::Response::builder()
                .header("content-type", asset.content_type)
                .body(axum::body::Body::from(asset.bytes))
                .unwrap(),
            None => axum::http::Response::builder()
                .status(404)
                .body(axum::body::Body::empty())
                .unwrap(),
        }
    });
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
    });
    let private_workspace = fixtures[1].1.id().as_uuid().to_string();
    let private_article = fixtures[1].2.id.as_uuid().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("npm")
            .args(["run", "test:e2e:acceptance"])
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web"))
            .env("READER_ACCEPTANCE_URL", origin)
            .env("READER_ACCEPTANCE_TOKEN", token)
            .env("READER_PRIVATE_WORKSPACE", private_workspace)
            .env("READER_PRIVATE_ARTICLE", private_article)
            .output()
            .expect("Playwright must be installed for release acceptance")
    })
    .await
    .unwrap();
    let _ = stop_tx.send(());
    server.await.unwrap().unwrap();
    assert_success("real browser + Rust + PostgreSQL acceptance", &output);
    let saved = repository
        .article(workspace.id(), article.id)
        .await
        .unwrap();
    assert!(
        saved.state.later,
        "browser mutation must be committed to PostgreSQL"
    );
    assert!(
        !repository
            .article(fixtures[1].1.id(), fixtures[1].2.id)
            .await
            .unwrap()
            .state
            .later,
        "private account state must remain unchanged"
    );
}
