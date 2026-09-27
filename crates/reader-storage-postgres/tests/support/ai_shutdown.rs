use super::*;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct SlowProvider {
    marker: PathBuf,
    calls: AtomicUsize,
}
#[async_trait]
impl AiProvider for SlowProvider {
    async fn balance(&self, _: &str) -> Result<Balance, AiError> {
        Ok(Balance {
            available: true,
            balances: vec![],
            updated_at: Utc::now(),
        })
    }
    async fn definitions(&self, _: &str, _: DefinitionsInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn translate(&self, _: &str, _: TranslationInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn generate(
        &self,
        _: &str,
        input: GenerationInput,
        progress: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::fs::write(&self.marker, b"admitted").unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        progress.check_active().await?;
        let envelope = serde_json::json!({"segments":[{"kind":"text","content":"**Title**\n\nExact source 12.5%."}]}).to_string();
        CompletedGeneration::new(
            envelope,
            Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                prompt_cache_hit_tokens: 0,
                prompt_cache_miss_tokens: 10,
                estimated_cost_usd: None,
            },
            &input.article.text,
            input.max_response_bytes,
        )
    }
}

pub async fn child(connection: &str, marker: PathBuf) {
    let pool = sqlx::PgPool::connect(connection).await.unwrap();
    let reader = Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(4096).unwrap(), 100).unwrap(),
    );
    let owner = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "Shutdown fixture".into());
    reader
        .save_workspace(None, workspace.clone())
        .await
        .unwrap();
    let article = Article {
        id: ArticleId::new(),
        key: DedupKey {
            location: ArticleLocation::from(
                url::Url::parse("https://example.com/shutdown").unwrap(),
            ),
            title: "Title".into(),
            description: None,
        },
        state: ArticleState::default(),
        first_arrived_at: Utc::now(),
        origins: vec![],
        revision: 0,
    };
    reader
        .save_article(workspace.id(), None, article.clone())
        .await
        .unwrap();
    let store = Arc::new(PostgresAiStore::new(
        pool,
        reader,
        std::num::NonZeroU32::new(2).unwrap(),
    ));
    let op = Uuid::new_v4();
    let initial = record(
        owner.as_uuid(),
        workspace.id().as_uuid(),
        article.id.as_uuid(),
        op,
    );
    let id = initial.view.id;
    store.create_chat(initial, op, false).await.unwrap();
    let provider = Arc::new(SlowProvider {
        marker,
        calls: AtomicUsize::new(0),
    });
    let service = Arc::new(AiService::new(
        store.clone(),
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        policy(owner.as_uuid()),
    ));
    service
        .save_key(owner.as_uuid(), "fixture-only-key".into())
        .await
        .unwrap();
    let signal = tokio::spawn(reader_runtime::termination_signal());
    tokio::task::yield_now().await;
    let mut supervisor = reader_runtime::TaskSupervisor::new();
    service.spawn_workers(&mut supervisor);
    signal.await.unwrap().unwrap();
    supervisor.request_shutdown();
    tokio::time::timeout(Duration::from_secs(5), supervisor.drain())
        .await
        .unwrap()
        .unwrap();
    let saved = store.chat(owner.as_uuid(), id).await.unwrap();
    assert_eq!(saved.view.status, ChatStatus::Completed);
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        2,
        "SIGTERM drains both phases without replay"
    );
    assert_eq!(saved.view.provider_calls.len(), 2);
    assert!(saved
        .view
        .provider_calls
        .iter()
        .all(|call| call.status == CallStatus::Completed));
}

pub fn parent(connection: &str) {
    let directory = std::env::temp_dir().join(format!("reader-shutdown-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let marker = directory.join("admitted");
    let log = directory.join("child.log");
    let output = std::fs::File::create(&log).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "real_postgres_creates_the_complete_idempotent_schema",
            "--nocapture",
        ])
        .env("READER_SHUTDOWN_FIXTURE_DATABASE", connection)
        .env("READER_SHUTDOWN_FIXTURE_MARKER", &marker)
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(output))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !marker.exists() && Instant::now() < deadline {
        assert!(
            child.try_wait().unwrap().is_none(),
            "shutdown child exited: {}",
            std::fs::read_to_string(&log).unwrap()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    if !marker.exists() {
        child.kill().unwrap();
        panic!(
            "provider never admitted: {}",
            std::fs::read_to_string(&log).unwrap()
        );
    }
    assert!(Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "SIGTERM did not drain work: {}",
                std::fs::read_to_string(&log).unwrap()
            );
            break;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("SIGTERM drain timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::remove_dir_all(directory).unwrap();
}
