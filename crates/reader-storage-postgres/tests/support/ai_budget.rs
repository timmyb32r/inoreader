use super::*;
use reader_ai::{AiError, AiStore, SpendMode, SpendReservation};
pub async fn verify(pool: &PgPool) {
    let reader = std::sync::Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(4096).unwrap(), 100, 86400)
            .unwrap(),
    );
    let store = std::sync::Arc::new(reader_storage_postgres::PostgresAiStore::new(
        pool.clone(),
        reader.clone(),
        std::num::NonZeroU32::new(2).unwrap(),
    ));
    let owner = Uuid::new_v4();
    let override_owner = Uuid::new_v4();
    sqlx::query("INSERT INTO ai_budget_overrides(owner,day,limit_usd) VALUES($1,(clock_timestamp() AT TIME ZONE 'Europe/Moscow')::date,5)").bind(override_owner).execute(pool).await.unwrap();
    let exception = store.spending(override_owner, "3").await.unwrap();
    assert_eq!(exception.daily_limit_usd, "5");
    assert_eq!(exception.remaining_usd, "5");
    store
        .reserve(
            override_owner,
            Uuid::new_v4(),
            &SpendReservation::new("4".into(), "3".into(), SpendMode::Summary).unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .reserve(
                override_owner,
                Uuid::new_v4(),
                &SpendReservation::new("1.01".into(), "3".into(), SpendMode::Chat).unwrap()
            )
            .await,
        Err(AiError::Budget)
    ));
    sqlx::query("UPDATE ai_budget_overrides SET day=day-1 WHERE owner=$1")
        .bind(override_owner)
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .spending(override_owner, "3")
            .await
            .unwrap()
            .daily_limit_usd,
        "3"
    );
    assert_eq!(
        store.spending(owner, "3").await.unwrap().daily_limit_usd,
        "3"
    );
    let mut jobs = tokio::task::JoinSet::new();
    for i in 0..20 {
        let store = store.clone();
        jobs.spawn(async move {
            let id = Uuid::new_v4();
            let mode = if i % 2 == 0 {
                SpendMode::Summary
            } else {
                SpendMode::Translation
            };
            let result = store
                .reserve(
                    owner,
                    id,
                    &SpendReservation::new("1".into(), "3".into(), mode).unwrap(),
                )
                .await;
            (id, result)
        });
    }
    let mut ids = Vec::new();
    while let Some(result) = jobs.join_next().await {
        let (id, result) = result.unwrap();
        match result {
            Ok(()) => ids.push(id),
            Err(AiError::Budget) => {}
            Err(error) => panic!("unexpected: {error}"),
        }
    }
    assert_eq!(
        ids.len(),
        3,
        "all request classes share one atomic daily limit"
    );
    let view = store.spending(owner, "3").await.unwrap();
    assert_eq!(view.remaining_usd, "0");
    assert_eq!(view.reserved_usd, "3");
    assert!(store
        .spending(Uuid::new_v4(), "3")
        .await
        .unwrap()
        .days
        .is_empty());
    store.settle(owner, ids[0], "0.25").await.unwrap();
    store.settle(owner, ids[0], "0.25").await.unwrap();
    assert!(matches!(
        store.settle(owner, ids[0], "0.30").await,
        Err(AiError::Conflict)
    ));
    assert!(matches!(
        store.settle(Uuid::new_v4(), ids[0], "0.25").await,
        Err(AiError::Conflict)
    ));
    assert!(matches!(
        store
            .reserve(
                owner,
                Uuid::new_v4(),
                &SpendReservation::new("0.76".into(), "3".into(), SpendMode::Chat).unwrap()
            )
            .await,
        Err(AiError::Budget)
    ));
    store
        .reserve(
            owner,
            Uuid::new_v4(),
            &SpendReservation::new("0.75".into(), "3".into(), SpendMode::Chat).unwrap(),
        )
        .await
        .unwrap();
    let view = store.spending(owner, "3").await.unwrap();
    assert_eq!(view.spent_usd, "0.25");
    assert_eq!(view.remaining_usd, "0");
    // Uncertain reservations persist across service reconstruction and midnight.
    sqlx::query("UPDATE ai_spending SET day=(clock_timestamp() AT TIME ZONE 'Europe/Moscow')::date-1 WHERE owner=$1").bind(owner).execute(pool).await.unwrap();
    let view = store.spending(owner, "3").await.unwrap();
    assert_eq!(view.remaining_usd, "3");
    assert!(!view.days.is_empty());
    let expected: String =
        sqlx::query_scalar("SELECT (clock_timestamp() AT TIME ZONE 'Europe/Moscow')::date::text")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(view.today, expected);
    store
        .reserve(
            owner,
            Uuid::new_v4(),
            &SpendReservation::new("3".into(), "3".into(), SpendMode::Verification).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(store.spending(owner, "3").await.unwrap().remaining_usd, "0");
    let account = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), account, "Automatic summaries".into());
    reader
        .save_workspace(None, workspace.clone())
        .await
        .unwrap();
    let article = Article {
        id: ArticleId::new(),
        key: DedupKey {
            location: ArticleLocation::from(url::Url::parse("https://example.com/budget").unwrap()),
            title: "Budget fixture".into(),
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
    let mut old_read = article.clone();
    old_read.id = ArticleId::new();
    old_read.state.read = true;
    reader
        .save_article(workspace.id(), None, old_read.clone())
        .await
        .unwrap();
    let account = account.as_uuid();
    let ws = workspace.id().as_uuid();
    store
        .save_credential(
            account,
            vec![1],
            reader_ai::Balance {
                available: true,
                balances: vec![],
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    store.enroll_summaries(&[account]).await.unwrap();
    store.enroll_summaries(&[account]).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_summary_queue WHERE owner=$1")
        .bind(account)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "one-time backfill excludes read archive");
    let mut later = old_read.clone();
    later.id = ArticleId::new();
    reader
        .save_article(workspace.id(), None, later)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_summary_queue WHERE owner=$1")
        .bind(account)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "new articles enroll even if already marked read");
    let record = ai_tests::record(account, ws, article.id.as_uuid(), Uuid::new_v4());
    let op = record.operations[0].id;
    let chat = store.create_chat(record, op, false).await.unwrap();
    store
        .reserve(
            account,
            Uuid::new_v4(),
            &SpendReservation::new("3".into(), "3".into(), SpendMode::Terms).unwrap(),
        )
        .await
        .unwrap();
    let claim = store.claim(60).await.unwrap().unwrap();
    assert_eq!(claim.record.view.id, chat.view.id);
    assert!(matches!(
        store
            .begin_call(
                &claim,
                reader_ai::GenerationPhase::Generating,
                &SpendReservation::new("0.1".into(), "3".into(), SpendMode::Summary).unwrap(),
                &reader_ai::CallModel::new(
                    reader_ai::DeepSeekModel::Flash,
                    claim.record.cost_rates.clone()
                ),
            )
            .await,
        Err(AiError::Budget)
    ));
    store.defer_budget(&claim).await.unwrap();
    assert!(
        store.claim(60).await.unwrap().is_none(),
        "no busy loop before Moscow midnight"
    );
    let waiting = store
        .public_chat(account, chat.view.id, None)
        .await
        .unwrap()
        .chat
        .unwrap();
    assert_eq!(waiting.status, reader_ai::ChatStatus::Queued);
    assert!(waiting.error.unwrap().contains("budget"));
    assert!(waiting.provider_calls.is_empty());
    sqlx::query("UPDATE ai_spending SET day=day-1 WHERE owner=$1")
        .bind(account)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_chats SET scheduled_at=now()-interval '1 second' WHERE id=$1")
        .bind(chat.view.id)
        .execute(pool)
        .await
        .unwrap();
    let resumed = store.claim(60).await.unwrap().unwrap();
    store
        .begin_call(
            &resumed,
            reader_ai::GenerationPhase::Generating,
            &SpendReservation::new("0.1".into(), "3".into(), SpendMode::Summary).unwrap(),
            &reader_ai::CallModel::new(
                reader_ai::DeepSeekModel::Flash,
                resumed.record.cost_rates.clone(),
            ),
        )
        .await
        .unwrap();
    assert!(
        store
            .public_chat(account, chat.view.id, None)
            .await
            .unwrap()
            .chat
            .unwrap()
            .error
            .is_none(),
        "resumption clears yesterday's budget explanation"
    );
    store.stop(account, chat.view.id).await.unwrap();
    // Exercise real automatic discovery -> persisted chat -> draft -> fact-check.
    let mut incoming = article.clone();
    incoming.id = ArticleId::new();
    incoming.key.title = "Automatically prepared".into();
    reader
        .save_article(workspace.id(), None, incoming.clone())
        .await
        .unwrap();
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse("https://example.com/automatic-feed").unwrap(),
        "Automatic".into(),
    );
    reader
        .save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let source = reader_core::SourceRecordId::new();
    let refresh = Uuid::new_v4();
    sqlx::query("INSERT INTO library_origins(workspace_id,article_id,subscription_id,source_record_id) VALUES($1,$2,$3,$4)").bind(ws.to_string()).bind(incoming.id.as_uuid().to_string()).bind(subscription.id().as_uuid().to_string()).bind(source.as_uuid().to_string()).execute(pool).await.unwrap();
    let pointer = reader_ingest::ContentManifestPointer {
        publication: None,
        record_id: source,
        source_revision: 0,
        refresh_id: refresh,
        raw_chunks: 1,
        safe_html_chunks: 1,
        fetched_at: Utc::now(),
        final_url: url::Url::parse("https://example.com/automatic").unwrap(),
    };
    sqlx::query("INSERT INTO content_manifests(id,revision,document) VALUES($1,0,$2)")
        .bind(source.as_uuid().to_string())
        .bind(serde_json::to_string(&pointer).unwrap())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staged_content_chunks(record_id,refresh_id,representation,ordinal,bytes) VALUES($1,$2,'safe',0,$3)").bind(source.as_uuid().to_string()).bind(refresh.to_string()).bind(b"<p>Entire article.</p>".as_slice()).execute(pool).await.unwrap();
    let cipher = reader_ai::CredentialCipher::new(&[5; 32]).unwrap();
    store
        .save_credential(
            account,
            cipher.encrypt(account, "test-key").unwrap(),
            reader_ai::Balance {
                available: true,
                balances: vec![],
                updated_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    let mut config = ai_tests::policy(account).config().clone();
    config.automatic_summaries = true;
    let service = reader_ai::AiService::new(
        store.clone(),
        std::sync::Arc::new(AutomaticProvider),
        cipher,
        reader_ai::AiPolicy::new(config, "Summarize".into(), "Verify".into()).unwrap(),
    );
    assert!(service.schedule_once().await.unwrap());
    assert!(
        !service.schedule_once().await.unwrap(),
        "dispatched candidates cannot generate another summary"
    );
    assert!(service.work_once().await.unwrap());
    let ready = service
        .chats(account, ws, incoming.id.as_uuid())
        .await
        .unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].status, reader_ai::ChatStatus::Completed);
    assert_eq!(ready[0].provider_calls.len(), 2);
    let again = service
        .start(account, ws, incoming.id.as_uuid(), Uuid::new_v4(), false)
        .await
        .unwrap();
    assert_eq!(again.id, ready[0].id);
    assert!(!service.work_once().await.unwrap());
    let stats = service.profile(account).await.unwrap().spending.unwrap();
    assert!(stats
        .days
        .iter()
        .any(|d| matches!(d.mode, SpendMode::Verification)));
}

struct AutomaticProvider;
#[async_trait::async_trait]
impl reader_ai::AiProvider for AutomaticProvider {
    async fn balance(&self, _: &str) -> Result<reader_ai::Balance, AiError> {
        Err(AiError::Unavailable)
    }
    async fn translate(
        &self,
        _: &str,
        _: reader_ai::TranslationInput,
    ) -> Result<reader_ai::ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn definitions(
        &self,
        _: &str,
        _: reader_ai::DefinitionsInput,
    ) -> Result<reader_ai::ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn generate(
        &self,
        _: &str,
        input: reader_ai::GenerationInput,
        progress: &mut (dyn reader_ai::GenerationProgress + Send),
    ) -> Result<reader_ai::CompletedGeneration, AiError> {
        let usage = reader_ai::Usage {
            prompt_tokens: 10,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 10,
            completion_tokens: 10,
            estimated_cost_usd: None,
        };
        progress.usage(&usage).await?;
        super::ai_tests::complete_text(&input, "Prepared summary.", usage, progress).await
    }
}
