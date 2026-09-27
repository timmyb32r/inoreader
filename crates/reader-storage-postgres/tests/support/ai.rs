use async_trait::async_trait;
use chrono::Utc;
use reader_ai::*;
use reader_application::{ArticleRepository, SubscriptionRepository, WorkspaceRepository};
use reader_core::*;
use reader_storage_postgres::{PostgresAiStore, PostgresRepository};
use sqlx::PgPool;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;

#[path = "ai_definitions.rs"]
mod definitions;
#[path = "ai_translation.rs"]
mod translation;
#[path = "ai_two_stage.rs"]
mod two_stage;

struct Provider {
    calls: AtomicUsize,
    modes: Mutex<Vec<GenerationMode>>,
    models: Mutex<Vec<String>>,
}
#[async_trait]
impl AiProvider for Provider {
    async fn definitions(&self, _: &str, _: DefinitionsInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn translate(&self, _: &str, _: TranslationInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn balance(&self, key: &str) -> Result<Balance, AiError> {
        if key == "bad" {
            return Err(AiError::InvalidKey);
        }
        Ok(Balance {
            available: true,
            balances: vec![BalanceAmount {
                currency: "USD".into(),
                total: "1.000000001".into(),
                granted: "0".into(),
                topped_up: "1.000000001".into(),
            }],
            updated_at: Utc::now(),
        })
    }
    async fn generate(
        &self,
        _: &str,
        input: GenerationInput,
        progress: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError> {
        assert_eq!(input.article.text, "Exact source 12.5%.");
        self.modes.lock().unwrap().push(input.generation_mode);
        self.models.lock().unwrap().push(input.model);
        self.calls.fetch_add(1, Ordering::SeqCst);
        progress.check_active().await?;
        progress.publish("First verified paragraph.").await?;
        progress
            .publish("First verified paragraph.\n\nSecond verified paragraph.")
            .await?;
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            prompt_cache_hit_tokens: 2,
            prompt_cache_miss_tokens: 8,
            estimated_cost_usd: None,
        };
        let content = if input.review_draft.is_some() {
            format!(
                "**{}**\n\nFirst verified paragraph.\n\nSecond verified paragraph.",
                input.article.title
            )
        } else {
            "First verified paragraph.\n\nSecond verified paragraph.".into()
        };
        CompletedGeneration::new(
            serde_json::json!({"segments":[{"kind":"text","content":content}]}).to_string(),
            usage,
            &input.article.text,
            input.max_response_bytes,
        )
    }
}
fn policy(owner: Uuid) -> AiPolicy {
    AiPolicy::new(
        AiConfig {
            prompt_approved: true,
            prompt_path: "test".into(),
            prompt_version: "test-v1".into(),
            review: ReviewConfig {
                prompt_path: "review".into(),
                prompt_version: "review-v1".into(),
                model: "deepseek-flash".into(),
                generation_mode: GenerationMode::standard(0.3).unwrap(),
                max_output_tokens: 100,
                input_usd_per_million_tokens: "0.30".into(),
                cached_input_usd_per_million_tokens: "0.006".into(),
                output_usd_per_million_tokens: "1.20".into(),
            },
            enabled_accounts: vec![owner],
            encryption_key_file_env: "UNUSED".into(),
            model: "deepseek-flash".into(),
            generation_mode: GenerationMode::standard(0.3).unwrap(),
            context_tokens: 10000,
            framing_tokens_per_message: 64,
            framing_tokens_base: 128,
            max_output_tokens: 100,
            max_input_bytes: 10000,
            max_message_bytes: 1000,
            max_response_bytes: 10000,
            request_timeout_seconds: 1,
            connect_timeout_seconds: 1,
            workers: 1,
            poll_milliseconds: 10,
            lease_seconds: 5,
            input_usd_per_million_tokens: "0.30".into(),
            cached_input_usd_per_million_tokens: "0.006".into(),
            output_usd_per_million_tokens: "1.20".into(),
        },
        "Test prompt".into(),
        "Review prompt".into(),
    )
    .unwrap()
}
fn record(owner: Uuid, workspace: Uuid, article: Uuid, operation: Uuid) -> ChatRecord {
    let now = Utc::now();
    let assistant = Uuid::new_v4();
    ChatRecord {
        owner,
        view: ArticleChat {
            id: Uuid::new_v4(),
            article_id: article,
            workspace_id: workspace,
            title: "Title".into(),
            source_url: "https://example.com/same".into(),
            created_at: now,
            model: "deepseek-flash".into(),
            prompt_version: "test-v1".into(),
            status: ChatStatus::Queued,
            provider_calls: vec![],
            messages: vec![ChatMessage {
                id: assistant,
                role: MessageRole::Assistant,
                content: String::new(),
                status: MessageStatus::Pending,
                created_at: now,
                phase: Some(GenerationPhase::Generating),
                purpose: Some(MessagePurpose::Summary),
            }],
            error: None,
        },
        snapshot: Some(ArticleSnapshot {
            title: "Title".into(),
            source_url: "https://example.com/same".into(),
            safe_html: "<p>Exact source 12.5%.</p>".into(),
            text: "Exact source 12.5%.".into(),
            source_revision: "pinned-version".into(),
        }),
        system_prompt: "Test prompt".into(),
        generation_mode: GenerationMode::standard(0.3).unwrap(),
        cost_rates: policy(owner).cost_rates().clone(),
        max_output_tokens: 100,
        limits: InputLimits::from(policy(owner).config()),
        review: policy(owner).review_snapshot().unwrap(),
        operations: vec![Operation {
            id: operation,
            kind: OperationKind::Start { regenerate: false },
            assistant_id: assistant,
            task: AttemptTask::Summary { draft: None },
        }],
    }
}

pub async fn verify(pool: &PgPool) {
    let reader = Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(4096).unwrap(), 100).unwrap(),
    );
    let store = Arc::new(PostgresAiStore::new(pool.clone(), reader.clone()));
    let owner = AccountId::new();
    let other = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "AI owner".into());
    let other_workspace = Workspace::new(WorkspaceId::new(), other, "AI other".into());
    reader
        .save_workspace(None, workspace.clone())
        .await
        .unwrap();
    reader
        .save_workspace(None, other_workspace.clone())
        .await
        .unwrap();
    let article = Article {
        id: ArticleId::new(),
        key: DedupKey {
            location: ArticleLocation::from(url::Url::parse("https://example.com/same").unwrap()),
            title: "Title".into(),
            description: Some("Exact introduction.".into()),
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
    reader
        .save_article(other_workspace.id(), None, article.clone())
        .await
        .unwrap();
    let owner = owner.as_uuid();
    let other = other.as_uuid();
    let ws = workspace.id().as_uuid();
    let a = article.id.as_uuid();
    let provider = Arc::new(Provider {
        calls: AtomicUsize::new(0),
        modes: Mutex::new(Vec::new()),
        models: Mutex::new(Vec::new()),
    });
    let service = AiService::new(
        store.clone(),
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        policy(owner),
    );
    service
        .save_key(owner, "owner-secret-token".into())
        .await
        .unwrap();
    assert!(matches!(
        service.save_key(owner, "bad".into()).await,
        Err(AiError::InvalidKey)
    ));
    assert!(
        service.profile(owner).await.unwrap().configured,
        "bad replacement preserves the previous key"
    );
    assert!(!service.profile(other).await.unwrap().configured);
    let encrypted: Vec<u8> =
        sqlx::query_scalar("SELECT encrypted_key FROM ai_profiles WHERE owner=$1")
            .bind(owner)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(!encrypted.windows(18).any(|v| v == b"owner-secret-token"));
    assert_eq!(
        service
            .profile(owner)
            .await
            .unwrap()
            .balance
            .unwrap()
            .balances[0]
            .total,
        "1.000000001"
    );

    let op = Uuid::new_v4();
    let initial = record(owner, ws, a, op);
    let competing = record(owner, ws, a, Uuid::new_v4());
    let (left, right) = tokio::join!(
        store.create_chat(initial.clone(), op, false),
        store.create_chat(competing.clone(), competing.operations[0].id, false)
    );
    let chat = left.unwrap();
    assert_eq!(
        chat.view.id,
        right.unwrap().view.id,
        "two devices create only one initial conversation"
    );
    let id = chat.view.id;
    assert_eq!(store.chats(owner, ws, a).await.unwrap().len(), 1);
    assert!(matches!(
        store.chat(other, id).await,
        Err(AiError::NotFound)
    ));
    assert!(matches!(
        store.chats(other, ws, a).await,
        Err(AiError::NotFound)
    ));
    assert!(matches!(
        store
            .append(other, id, Uuid::new_v4(), OperationKind::Retry)
            .await,
        Err(AiError::NotFound)
    ));
    assert!(matches!(
        store.stop(other, id).await,
        Err(AiError::NotFound)
    ));
    assert!(matches!(
        store.article_input(other, ws, a).await,
        Err(AiError::NotFound)
    ));
    assert!(
        store
            .chats(other, other_workspace.id().as_uuid(), a)
            .await
            .unwrap()
            .is_empty(),
        "same source never shares chats"
    );

    assert!(service.work_once().await.unwrap());
    assert_eq!(
        service.chat(owner, id).await.unwrap().status,
        ChatStatus::Completed
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert!(!service.work_once().await.unwrap());
    assert_eq!(
        service
            .start(owner, ws, a, Uuid::new_v4(), false)
            .await
            .unwrap()
            .id,
        id
    );
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        2,
        "reopening never calls provider"
    );
    let message_op = Uuid::new_v4();
    let sent = service
        .message(owner, id, message_op, "Explain the number".into())
        .await
        .unwrap();
    assert_eq!(sent.messages.len(), 3);
    assert_eq!(
        service
            .message(owner, id, message_op, "Explain the number".into())
            .await
            .unwrap()
            .messages
            .len(),
        3
    );
    assert!(matches!(
        service
            .message(owner, id, message_op, "Different command".into())
            .await,
        Err(AiError::Conflict)
    ));
    let claim = store.claim(5).await.unwrap().unwrap();
    store
        .update_claim(
            &claim,
            ChatStatus::Generating,
            Some("retained partial"),
            None,
            None,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE ai_chats SET lease_until=now()-interval '1 second' WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    assert!(store.claim(5).await.unwrap().is_none());
    let resumed_store = PostgresAiStore::new(pool.clone(), reader.clone());
    let interrupted = resumed_store.chat(owner, id).await.unwrap();
    assert_eq!(interrupted.view.status, ChatStatus::Interrupted);
    assert_eq!(
        interrupted.view.messages.last().unwrap().content,
        "retained partial"
    );
    assert_eq!(
        interrupted.snapshot.unwrap().source_revision,
        "pinned-version"
    );
    assert!(matches!(
        store
            .update_claim(&claim, ChatStatus::Completed, None, None, None)
            .await,
        Err(AiError::Cancelled)
    ));
    let mut changed_config = policy(owner).config().clone();
    changed_config.model = "deepseek-v4-pro".into();
    changed_config.input_usd_per_million_tokens = "1.32".into();
    changed_config.cached_input_usd_per_million_tokens = "0.044".into();
    changed_config.output_usd_per_million_tokens = "3.96".into();
    changed_config.generation_mode = GenerationMode::Thinking {
        effort: ReasoningEffort::High,
    };
    let changed_service = AiService::new(
        store.clone(),
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        AiPolicy::new(
            changed_config,
            "Changed runtime prompt".into(),
            "Changed review prompt".into(),
        )
        .unwrap(),
    );
    let retry = changed_service
        .retry(owner, id, Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(
        retry.messages.len(),
        4,
        "retry preserves prior attempt and original question"
    );
    changed_service.work_once().await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        *provider.modes.lock().unwrap(),
        vec![GenerationMode::standard(0.3).unwrap(); 3],
        "retry retains the original mode after a deployment config switch"
    );
    assert_eq!(
        *provider.models.lock().unwrap(),
        vec!["deepseek-flash"; 3],
        "an old Flash conversation still calls Flash on a Pro-configured worker"
    );
    let calls = service.chat(owner, id).await.unwrap().provider_calls;
    assert_eq!(calls.len(), 3);
    for call in calls {
        assert_eq!(
            call.usage.unwrap().estimated_cost_usd.as_deref(),
            Some("0.000008412"),
            "both stages and retry retain the saved Flash rates"
        );
    }
    assert_eq!(
        store.chat(owner, id).await.unwrap().cost_rates,
        policy(owner).cost_rates().clone()
    );

    service
        .message(owner, id, Uuid::new_v4(), "Cancel this".into())
        .await
        .unwrap();
    let claim = store.claim(5).await.unwrap().unwrap();
    service.stop(owner, id).await.unwrap();
    assert!(!store.active(owner, id, claim.lease).await.unwrap());
    assert!(matches!(
        store
            .update_claim(
                &claim,
                ChatStatus::Generating,
                Some("must not publish"),
                None,
                None
            )
            .await,
        Err(AiError::Cancelled)
    ));

    let new_op = Uuid::new_v4();
    let mut newer = record(owner, ws, a, new_op);
    newer.snapshot = None;
    newer.view.status = ChatStatus::WaitingContent;
    newer.operations[0].kind = OperationKind::Start { regenerate: true };
    let waiting = store.create_chat(newer, new_op, true).await.unwrap();
    service.work_once().await.unwrap();
    assert_eq!(
        service.chat(owner, waiting.view.id).await.unwrap().status,
        ChatStatus::WaitingContent
    );
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        3,
        "missing full text never sends the RSS excerpt"
    );
    service.delete_key(owner).await.unwrap();
    assert_eq!(
        service.chat(owner, waiting.view.id).await.unwrap().status,
        ChatStatus::Cancelled
    );
    assert_eq!(
        store.chats(owner, ws, a).await.unwrap().len(),
        2,
        "key deletion retains all versions"
    );
    assert_eq!(service.chat(owner, id).await.unwrap().messages.len(), 6);
    assert_eq!(
        service.chat(owner, id).await.unwrap().provider_calls.len(),
        3
    );

    // Exercise the real manifest/chunk input path, including exact source
    // revision and failure on missing chunks, independently of chat fixtures.
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse("https://example.com/ai-feed").unwrap(),
        "AI source".into(),
    );
    reader
        .save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let source_record = SourceRecordId::new();
    sqlx::query("INSERT INTO library_origins(workspace_id,article_id,subscription_id,source_record_id) VALUES($1,$2,$3,$4)").bind(ws.to_string()).bind(a.to_string()).bind(subscription.id().as_uuid().to_string()).bind(source_record.as_uuid().to_string()).execute(pool).await.unwrap();
    let refresh = Uuid::new_v4();
    let pointer = reader_ingest::ContentManifestPointer {
        record_id: source_record,
        source_revision: 7,
        refresh_id: refresh,
        raw_chunks: 1,
        safe_html_chunks: 1,
        fetched_at: Utc::now(),
        final_url: url::Url::parse("https://example.com/same").unwrap(),
    };
    sqlx::query("INSERT INTO content_manifests(id,revision,document) VALUES($1,0,$2)")
        .bind(source_record.as_uuid().to_string())
        .bind(serde_json::to_string(&pointer).unwrap())
        .execute(pool)
        .await
        .unwrap();
    let html = "<p>Exact source 12.5%.</p>";
    sqlx::query("INSERT INTO staged_content_chunks(record_id,refresh_id,representation,ordinal,bytes) VALUES($1,$2,'safe',0,$3)").bind(source_record.as_uuid().to_string()).bind(refresh.to_string()).bind(serde_json::to_string(html.as_bytes()).unwrap()).execute(pool).await.unwrap();
    let ArticleInput::Ready(snapshot) = store.article_input(owner, ws, a).await.unwrap() else {
        panic!("ready manifest must produce full snapshot")
    };
    assert_eq!(snapshot.text, "Exact source 12.5%.");
    assert_eq!(snapshot.safe_html, html);
    assert_eq!(
        snapshot.source_revision,
        format!("{}/7/{refresh}", source_record.as_uuid())
    );
    sqlx::query("UPDATE content_manifests SET document=jsonb_set(document::jsonb,'{safe_html_chunks}','2')::text WHERE id=$1").bind(source_record.as_uuid().to_string()).execute(pool).await.unwrap();
    assert!(
        matches!(
            store.article_input(owner, ws, a).await,
            Err(AiError::Storage)
        ),
        "incomplete chunks fail closed"
    );
    assert_eq!(
        store
            .chat(owner, id)
            .await
            .unwrap()
            .snapshot
            .unwrap()
            .source_revision,
        "pinned-version",
        "old conversations stay attached to their saved source"
    );

    // A balance request for an old key cannot overwrite the replacement key's
    // balance after a concurrent key replacement.
    service
        .save_key(owner, "replacement-key".into())
        .await
        .unwrap();
    assert!(matches!(
        store
            .save_balance(owner, &encrypted, None, Some("stale error".into()))
            .await,
        Err(AiError::Conflict)
    ));
    assert!(service.profile(owner).await.unwrap().error.is_none());

    sqlx::query("UPDATE content_manifests SET document=$2 WHERE id=$1")
        .bind(source_record.as_uuid().to_string())
        .bind(serde_json::to_string(&pointer).unwrap())
        .execute(pool)
        .await
        .unwrap();
    translation::verify(pool, store.clone(), owner, other, ws, a).await;
    definitions::verify(pool, store.clone(), owner, other, ws, a).await;
    let thinking = changed_service
        .start(owner, ws, a, Uuid::new_v4(), true)
        .await
        .unwrap();
    assert_eq!(
        store
            .chat(owner, thinking.id)
            .await
            .unwrap()
            .generation_mode,
        GenerationMode::Thinking {
            effort: ReasoningEffort::High
        }
    );
    service.work_once().await.unwrap();
    assert_eq!(
        provider.modes.lock().unwrap().iter().rev().nth(1),
        Some(&GenerationMode::Thinking {
            effort: ReasoningEffort::High
        }),
        "new conversation retains its thinking mode even on a worker configured for standard mode"
    );
    assert_eq!(
        provider
            .models
            .lock()
            .unwrap()
            .iter()
            .rev()
            .nth(1)
            .map(String::as_str),
        Some("deepseek-v4-pro")
    );
    let calls = service
        .chat(owner, thinking.id)
        .await
        .unwrap()
        .provider_calls;
    assert_eq!(
        calls[0]
            .usage
            .as_ref()
            .unwrap()
            .estimated_cost_usd
            .as_deref(),
        Some("0.000030448")
    );
    assert_eq!(
        calls[1]
            .usage
            .as_ref()
            .unwrap()
            .estimated_cost_usd
            .as_deref(),
        Some("0.000008412"),
        "review has its own saved model/rates"
    );

    // A configured key does not bypass the unapproved-prompt gate, including a
    // durable operation queued before the deployment disabled approval.
    let mut unapproved_config = policy(owner).config().clone();
    unapproved_config.prompt_approved = false;
    let unapproved = AiService::new(
        store.clone(),
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        AiPolicy::new(unapproved_config, String::new(), String::new()).unwrap(),
    );
    let calls_before = provider.calls.load(Ordering::SeqCst);
    let profile = unapproved.profile(owner).await.unwrap();
    assert!(profile.configured);
    assert!(!profile.enabled);
    assert_eq!(
        profile.availability_reason.as_deref(),
        Some(AiError::PromptPending.to_string().as_str())
    );
    assert!(matches!(
        unapproved.start(owner, ws, a, Uuid::new_v4(), true).await,
        Err(AiError::PromptPending)
    ));
    assert!(matches!(
        unapproved
            .message(owner, thinking.id, Uuid::new_v4(), "Blocked message".into())
            .await,
        Err(AiError::PromptPending)
    ));
    assert!(matches!(
        unapproved.retry(owner, thinking.id, Uuid::new_v4()).await,
        Err(AiError::PromptPending)
    ));
    assert_eq!(
        unapproved
            .start(owner, ws, a, Uuid::new_v4(), false)
            .await
            .unwrap()
            .id,
        thinking.id,
        "existing chats remain readable without approval"
    );
    service
        .message(
            owner,
            thinking.id,
            Uuid::new_v4(),
            "Queued before disabling approval".into(),
        )
        .await
        .unwrap();
    assert!(unapproved.work_once().await.unwrap());
    let blocked = unapproved.chat(owner, thinking.id).await.unwrap();
    assert_eq!(blocked.status, ChatStatus::Failed);
    assert_eq!(
        blocked.error.as_deref(),
        Some(AiError::PromptPending.to_string().as_str())
    );
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        calls_before,
        "neither API entry points nor queued jobs call the provider for an unapproved prompt"
    );
    two_stage::verify(pool, store.clone(), owner, ws, a).await;
}
