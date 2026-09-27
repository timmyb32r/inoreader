use reader_ai::*;
use reader_application::SubscriptionRepository;
use reader_core::*;
use reader_storage_postgres::{PostgresAiStore, PostgresRepository};
use sqlx::PgPool;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use uuid::Uuid;

struct BrowserProvider(AtomicUsize);
#[async_trait::async_trait]
impl AiProvider for BrowserProvider {
    async fn balance(&self, _: &str) -> Result<Balance, AiError> {
        Ok(Balance {
            available: true,
            balances: vec![],
            updated_at: chrono::Utc::now(),
        })
    }
    async fn definitions(&self, _: &str, _: DefinitionsInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn generate(
        &self,
        _: &str,
        _: GenerationInput,
        _: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError> {
        Err(AiError::Unavailable)
    }
    async fn translate(&self, _: &str, _: TranslationInput) -> Result<ProviderReply, AiError> {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let body = if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            b"invalid provider JSON".to_vec()
        } else {
            serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::json!({"translation":"Точный исходный текст.","words":[{"source":"Exact","pinyin":null,"translation":"точный"},{"source":"source","pinyin":null,"translation":"исходный"},{"source":"text","pinyin":null,"translation":"текст"}]}).to_string()}}]}).to_string().into_bytes()
        };
        Ok(ProviderReply {
            status: 200,
            body,
            interrupted: false,
        })
    }
}

pub async fn service(
    pool: &PgPool,
    repository: Arc<PostgresRepository>,
    account: AccountId,
    workspace: WorkspaceId,
    article: ArticleId,
) -> Arc<AiService> {
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace,
        url::Url::parse("https://example.test/browser-feed").unwrap(),
        "Browser AI".into(),
    );
    repository
        .save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let record = SourceRecordId::new();
    let refresh = Uuid::new_v4();
    sqlx::query("INSERT INTO library_origins(workspace_id,article_id,subscription_id,source_record_id) VALUES($1,$2,$3,$4)").bind(workspace.as_uuid().to_string()).bind(article.as_uuid().to_string()).bind(subscription.id().as_uuid().to_string()).bind(record.as_uuid().to_string()).execute(pool).await.unwrap();
    let manifest = reader_ingest::ContentManifestPointer {
        record_id: record,
        source_revision: 1,
        refresh_id: refresh,
        raw_chunks: 1,
        safe_html_chunks: 1,
        fetched_at: chrono::Utc::now(),
        final_url: url::Url::parse("https://example.test/browser").unwrap(),
    };
    sqlx::query("INSERT INTO content_manifests(id,revision,document) VALUES($1,0,$2)")
        .bind(record.as_uuid().to_string())
        .bind(serde_json::to_string(&manifest).unwrap())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staged_content_chunks(record_id,refresh_id,representation,ordinal,bytes) VALUES($1,$2,'safe',0,$3)").bind(record.as_uuid().to_string()).bind(refresh.to_string()).bind(b"<p>Exact source text</p>".as_slice()).execute(pool).await.unwrap();
    let store = Arc::new(PostgresAiStore::new(
        pool.clone(),
        repository,
        std::num::NonZeroU32::new(2).unwrap(),
    ));
    let service = Arc::new(AiService::new(
        store,
        Arc::new(BrowserProvider(AtomicUsize::new(0))),
        CredentialCipher::new(&[42; 32]).unwrap(),
        super::super::ai_tests::policy(account.as_uuid()),
    ));
    service
        .save_key(account.as_uuid(), "fixture-only-key".into())
        .await
        .unwrap();
    service
}
