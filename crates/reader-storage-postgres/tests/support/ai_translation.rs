use super::*;
struct Translator(AtomicUsize);
#[async_trait]
impl AiProvider for Translator {
    async fn definitions(&self, _: &str, _: DefinitionsInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn balance(&self, _: &str) -> Result<Balance, AiError> {
        unreachable!()
    }
    async fn generate(
        &self,
        _: &str,
        _: GenerationInput,
        _: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError> {
        unreachable!()
    }
    async fn translate(&self, _: &str, input: TranslationInput) -> Result<ProviderReply, AiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        assert_eq!(input.source(), "Exact source 12.5%.");
        Ok(ProviderReply {
            status:200,interrupted:false,
            body:serde_json::to_vec(&serde_json::json!({
                "choices":[{"finish_reason":"stop","message":{"content":r#"{"translation":"Точный источник 12.5%.","words":[{"source":"Exact","pinyin":null,"translation":"точный"},{"source":"source","pinyin":null,"translation":"источник"},{"source":"12.5","pinyin":null,"translation":"12.5"}]}"#}}],
                "usage":{"prompt_tokens":10,"completion_tokens":20,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":10}
            })).unwrap(),
        })
    }
}
pub async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    other: Uuid,
    workspace: Uuid,
    article: Uuid,
) {
    for text in ["Title", "Exact introduction."] {
        assert!(store
            .article_intro_contains(owner, workspace, article, text)
            .await
            .unwrap());
    }
    for text in ["Tit", "Exact introduction", "Injected paragraph", ""] {
        assert!(!store
            .article_intro_contains(owner, workspace, article, text)
            .await
            .unwrap());
    }
    assert!(store
        .article_intro_contains(other, workspace, article, "Title")
        .await
        .is_err());
    let provider = Arc::new(Translator(AtomicUsize::new(0)));
    let service = AiService::new(
        store.clone(),
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        policy(owner),
    );
    assert!(matches!(
        service
            .translate(
                owner,
                workspace,
                article,
                Uuid::new_v4(),
                "Injected paragraph".into()
            )
            .await,
        Err(AiError::Message)
    ));
    assert!(matches!(
        service
            .translate(
                owner,
                Uuid::new_v4(),
                article,
                Uuid::new_v4(),
                "Exact source 12.5%.".into()
            )
            .await,
        Err(AiError::NotFound)
    ));
    let operation = Uuid::new_v4();
    let (a, b) = tokio::join!(
        service.translate(
            owner,
            workspace,
            article,
            operation,
            "Exact source 12.5%.".into()
        ),
        service.translate(
            owner,
            workspace,
            article,
            Uuid::new_v4(),
            "Exact source 12.5%.".into()
        )
    );
    let job = a.unwrap();
    assert_eq!(job.id, b.unwrap().id);
    assert!(matches!(
        store.translations(other, workspace, article).await,
        Err(AiError::NotFound)
    ));
    assert!(service.translate_once().await.unwrap());
    assert!(!service.translate_once().await.unwrap());
    assert_eq!(provider.0.load(Ordering::SeqCst), 1);
    let saved = service
        .translations(owner, workspace, article)
        .await
        .unwrap();
    assert_eq!(saved.len(), 1);
    assert!(
        matches!(&saved[0].state,TranslationState::Completed{result} if result.source()=="Exact source 12.5%.")
    );
    assert!(saved[0]
        .usage
        .as_ref()
        .unwrap()
        .estimated_cost_usd
        .is_some());
    let cached = service
        .translate(
            owner,
            workspace,
            article,
            Uuid::new_v4(),
            "Exact source 12.5%.".into(),
        )
        .await
        .unwrap();
    assert_eq!(cached.id, job.id);
    assert_eq!(provider.0.load(Ordering::SeqCst), 1);
    let (raw,status,interrupted,document):(Vec<u8>,i32,bool,String)=sqlx::query_as("SELECT raw_response,response_status,response_interrupted,document FROM ai_translations WHERE id=$1").bind(job.id).fetch_one(pool).await.unwrap();
    let retained = ProviderReply {
        status: u16::try_from(status).unwrap(),
        body: raw,
        interrupted,
    };
    assert_eq!(
        retained
            .translation_result("Exact source 12.5%.")
            .unwrap()
            .source(),
        "Exact source 12.5%."
    );
    assert_eq!(
        provider.0.load(Ordering::SeqCst),
        1,
        "offline replay must not bill again"
    );
    let record: TranslationRecord = serde_json::from_str(&document).unwrap();
    let mut other_model = policy(owner).config().clone();
    other_model.model = "another-model".into();
    let mut changed = record.clone();
    changed.job.id = Uuid::new_v4();
    changed.job.model = other_model.model.clone();
    changed.job.state = TranslationState::Queued;
    changed.input = Some(TranslationInput::new(&other_model, &changed.job.source).unwrap());
    let changed_id = changed.job.id;
    assert_eq!(
        store.create_translation(changed).await.unwrap().id,
        changed_id,
        "cache must include execution input"
    );
    sqlx::query("UPDATE ai_translations SET status='failed',document=jsonb_set(document::jsonb,'{job}',document::jsonb->'job'||jsonb_build_object('status','failed','error','test-only'))::text WHERE id=$1").bind(changed_id).execute(pool).await.unwrap();
    // Corrupt one completed payload deliberately; the rest of the list survives
    // and no original bytes are rewritten by the read/diagnostic path.
    sqlx::query("UPDATE ai_translations SET document=jsonb_set(document::jsonb,'{job,result,segments}','[]'::jsonb)::text WHERE id=$1").bind(job.id).execute(pool).await.unwrap();
    let broken = store.translations(owner, workspace, article).await.unwrap();
    assert!(broken
        .iter()
        .any(|r| r.id == job.id && matches!(r.state, TranslationState::Failed { .. })));
    sqlx::query("UPDATE ai_translations SET document=$2 WHERE id=$1")
        .bind(job.id)
        .bind(document)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM ai_translations WHERE id=$1")
        .bind(changed_id)
        .execute(pool)
        .await
        .unwrap();
    // Simulate an expired in-flight attempt: never repeat an uncertain paid call.
    sqlx::query("UPDATE ai_translations SET status='generating',lease=$2,lease_until=now()-interval '1 second' WHERE id=$1").bind(job.id).bind(Uuid::new_v4()).execute(pool).await.unwrap();
    assert!(store.claim_translation(5).await.unwrap().is_none());
    assert!(matches!(
        store.translations(owner, workspace, article).await.unwrap()[0].state,
        TranslationState::Failed { .. }
    ));
    let retry = service
        .translate(
            owner,
            workspace,
            article,
            Uuid::new_v4(),
            "Exact source 12.5%.".into(),
        )
        .await
        .unwrap();
    assert_ne!(retry.id, job.id);
    let claim = store.claim_translation(5).await.unwrap().unwrap();
    sqlx::query("UPDATE ai_translations SET lease_until=now()-interval '1 second' WHERE id=$1")
        .bind(retry.id)
        .execute(pool)
        .await
        .unwrap();
    assert!(matches!(
        store
            .finish_translation(
                &claim,
                TranslationState::Failed {
                    error: "stale".into()
                },
                None,
                None
            )
            .await,
        Err(AiError::Cancelled)
    ));
    store.claim_translation(5).await.unwrap();
    assert_eq!(
        store
            .translations(owner, workspace, article)
            .await
            .unwrap()
            .len(),
        2,
        "failed attempts preserved"
    );
    // Corrupt identity metadata cannot be used to claim a paid job.
    let saved: String = sqlx::query_scalar("SELECT document FROM ai_translations WHERE id=$1")
        .bind(retry.id)
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_translations SET status='queued',document=jsonb_set(document::jsonb,'{owner}',to_jsonb($2::text))::text WHERE id=$1").bind(retry.id).bind(other.to_string()).execute(pool).await.unwrap();
    assert!(
        matches!(
            store.translations(owner, workspace, article).await,
            Err(AiError::Storage)
        ),
        "identity corruption must fail closed before exposing source text"
    );
    let corrupt: String = sqlx::query_scalar("SELECT document FROM ai_translations WHERE id=$1")
        .bind(retry.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(store.claim_translation(5).await.unwrap().is_none());
    let quarantined: (String, String) =
        sqlx::query_as("SELECT status,document FROM ai_translations WHERE id=$1")
            .bind(retry.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(quarantined, ("failed".into(), corrupt));
    assert!(
        store.claim_translation(5).await.unwrap().is_none(),
        "corrupt row must not block subsequent queue scans"
    );
    sqlx::query("UPDATE ai_translations SET status='failed',document=$2 WHERE id=$1")
        .bind(retry.id)
        .bind(saved)
        .execute(pool)
        .await
        .unwrap();
}
