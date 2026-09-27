use super::*;
struct Translator(AtomicUsize);
#[async_trait]
impl AiProvider for Translator {
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
    async fn translate(
        &self,
        _: &str,
        input: TranslationInput,
    ) -> Result<(ParagraphTranslation, Usage), AiError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        assert_eq!(input.source(), "Exact source 12.5%.");
        Ok((
            ParagraphTranslation::from_response(
                input.source(),
                r#"{"translation":"Точный источник 12.5%.","segments":[{"kind":"word","source":"Exact","pinyin":null,"translation":"точный"},{"kind":"literal","source":" "},{"kind":"word","source":"source","pinyin":null,"translation":"источник"},{"kind":"literal","source":" "},{"kind":"word","source":"12.5","pinyin":null,"translation":"12.5"},{"kind":"literal","source":"%."}]}"#,
            )?,
            Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                prompt_cache_hit_tokens: 0,
                prompt_cache_miss_tokens: 10,
                estimated_cost_usd: None,
            },
        ))
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
    assert!(matches!(
        store.claim_translation(5).await,
        Err(AiError::Storage)
    ));
    sqlx::query("UPDATE ai_translations SET status='failed',document=$2 WHERE id=$1")
        .bind(retry.id)
        .bind(saved)
        .execute(pool)
        .await
        .unwrap();
}
