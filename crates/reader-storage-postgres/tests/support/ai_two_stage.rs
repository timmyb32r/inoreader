use super::*;
use std::collections::VecDeque;
use tokio::sync::Notify;

#[path = "ai_review.rs"]
mod review;
#[path = "ai_unchecked.rs"]
mod unchecked;

enum Step {
    Text(&'static str),
    ChangeModels(reader_ai::ModelPreferences, &'static str),
    Error(AiError),
    BadQuote,
    Review(&'static str),
    Stop(bool),
    Wait(Arc<Notify>),
}
struct Scripted {
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    chat: Uuid,
    steps: Mutex<VecDeque<Step>>,
    inputs: Mutex<Vec<GenerationInput>>,
}
fn usage() -> Usage {
    Usage {
        prompt_tokens: 10,
        completion_tokens: 5,
        prompt_cache_hit_tokens: 2,
        prompt_cache_miss_tokens: 8,
        estimated_cost_usd: None,
    }
}
#[async_trait]
impl AiProvider for Scripted {
    async fn definitions(&self, _: &str, _: DefinitionsInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn translate(&self, _: &str, _: TranslationInput) -> Result<ProviderReply, AiError> {
        Err(AiError::Unavailable)
    }
    async fn balance(&self, _: &str) -> Result<Balance, AiError> {
        unreachable!()
    }
    async fn generate(
        &self,
        _: &str,
        input: GenerationInput,
        progress: &mut (dyn GenerationProgress + Send),
    ) -> Result<CompletedGeneration, AiError> {
        progress.check_active().await?;
        self.inputs.lock().unwrap().push(input.clone());
        let step = self
            .steps
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected paid request");
        if input.review_draft.is_some() {
            let view = self.store.chat(self.owner, self.chat).await?.view;
            assert_eq!(view.status, ChatStatus::Verifying);
            assert!(
                view.messages.last().unwrap().content.is_empty(),
                "unverified draft never enters public messages"
            );
        }
        let text = match step {
            Step::ChangeModels(models, text) => {
                self.store
                    .save_model_preferences(self.owner, models)
                    .await?;
                text
            }
            Step::Error(error) => return Err(error),
            Step::Wait(notify) => {
                progress.publish("Partial verifier output").await?;
                notify.notify_one();
                std::future::pending().await
            }
            Step::Stop(delete_key) => {
                progress.usage(&usage()).await?;
                if delete_key {
                    self.store.delete_credential(self.owner).await?;
                } else {
                    self.store.stop(self.owner, self.chat).await?;
                }
                "Unverified draft retained after stop"
            }
            Step::BadQuote => {
                progress.usage(&usage()).await?;
                return CompletedGeneration::new(serde_json::json!({"segments":[{"kind":"quote","content":"fabricated quotation"}]}).to_string(), usage(), &input.article.text, input.max_response_bytes);
            }
            Step::Review(raw) => {
                progress.usage(&usage()).await?;
                progress.response(raw, &input.system).await?;
                return CompletedGeneration::reviewed(
                    raw.into(),
                    usage(),
                    input.review_draft.as_deref().unwrap(),
                    &input.article.title,
                    &input.article.text,
                    input.max_response_bytes,
                );
            }
            Step::Text(text) => {
                progress.publish(text).await?;
                text
            }
        };
        complete_text(&input, text, usage(), progress).await
    }
}

async fn fixture(
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    ws: Uuid,
    article: Uuid,
    steps: Vec<Step>,
) -> (Arc<AiService>, Arc<Scripted>, Uuid) {
    fixture_title(store, owner, ws, article, steps, "Title").await
}

async fn fixture_title(
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    ws: Uuid,
    article: Uuid,
    steps: Vec<Step>,
    title: &str,
) -> (Arc<AiService>, Arc<Scripted>, Uuid) {
    let operation = Uuid::new_v4();
    let mut initial = record(owner, ws, article, operation);
    initial.view.title = title.into();
    initial.snapshot.as_mut().unwrap().title = title.into();
    initial.operations[0].kind = OperationKind::Start { regenerate: true };
    let id = initial.view.id;
    store.create_chat(initial, operation, true).await.unwrap();
    let provider = Arc::new(Scripted {
        store: store.clone(),
        owner,
        chat: id,
        steps: Mutex::new(steps.into()),
        inputs: Mutex::new(vec![]),
    });
    let service = Arc::new(AiService::new(
        store,
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        policy(owner),
    ));
    (service, provider, id)
}

pub(super) async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    ws: Uuid,
    article: Uuid,
) {
    // A queued job follows current preferences; a preference change while the
    // first request is running affects the checker, but not that first bill.
    use reader_ai::{DeepSeekModel, ModelPreferences};
    assert_eq!(
        store.model_preferences(owner).await.unwrap(),
        ModelPreferences::default()
    );
    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::ChangeModels(ModelPreferences::default(), "Draft"),
            Step::Text("**Title**\n\nChecked summary"),
        ],
    )
    .await;
    service
        .save_models(
            owner,
            ModelPreferences {
                summary: DeepSeekModel::Pro,
                verification: Some(DeepSeekModel::Pro),
            },
        )
        .await
        .unwrap();
    let other = Uuid::new_v4();
    assert_eq!(
        store.model_preferences(other).await.unwrap(),
        ModelPreferences::default()
    );
    assert_eq!(
        service.profile(owner).await.unwrap().models.summary,
        DeepSeekModel::Pro
    );
    service.work_once().await.unwrap();
    let view = service.chat(owner, id).await.unwrap();
    assert_eq!(view.status, ChatStatus::Completed);
    assert_eq!(provider.inputs.lock().unwrap()[0].model, "deepseek-v4-pro");
    assert_eq!(provider.inputs.lock().unwrap()[1].model, "deepseek-flash");
    assert_eq!(
        view.provider_calls[0]
            .usage
            .as_ref()
            .unwrap()
            .estimated_cost_usd
            .as_deref(),
        Some("0.000030448")
    );
    assert_eq!(
        view.provider_calls[1]
            .usage
            .as_ref()
            .unwrap()
            .estimated_cost_usd
            .as_deref(),
        Some("0.000008412")
    );
    let selected: String = sqlx::query_scalar(
        "SELECT document::jsonb->>'model' FROM ai_call_models WHERE id=$1 AND owner=$2",
    )
    .bind(view.provider_calls[0].id)
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(selected, "deepseek-v4-pro");
    assert_eq!(
        service.profile(owner).await.unwrap().models,
        ModelPreferences::default()
    );

    // Second-request auth/rate errors preserve the paid draft and retry only the
    // checker, with the same source/prompt, even after worker configuration changes.
    for error in [AiError::InvalidKey, AiError::RateLimit, AiError::Provider] {
        let (service, provider, id) = fixture(
            store.clone(),
            owner,
            ws,
            article,
            vec![
                Step::Text("Unverified draft"),
                Step::Error(error),
                Step::Text("**Title**\n\nChecked summary"),
                Step::Text("Chat reply"),
            ],
        )
        .await;
        service.work_once().await.unwrap();
        let failed = service.chat(owner, id).await.unwrap();
        assert!(matches!(
            failed.status,
            ChatStatus::Failed | ChatStatus::Interrupted
        ));
        assert_eq!(failed.messages[0].content, "**Title**\n\nUnverified draft");
        assert_eq!(failed.messages[0].phase, Some(GenerationPhase::Verifying));
        assert_eq!(failed.provider_calls.len(), 2);
        assert_eq!(
            failed.provider_calls[0]
                .usage
                .as_ref()
                .unwrap()
                .estimated_cost_usd
                .as_deref(),
            Some("0.000008412")
        );
        assert!(
            failed.provider_calls[1].usage.is_none(),
            "unknown second bill is not zero"
        );
        assert!(!service.work_once().await.unwrap());
        let op = Uuid::new_v4();
        let (left, right) =
            tokio::join!(service.retry(owner, id, op), service.retry(owner, id, op));
        assert_eq!(left.unwrap().messages.len(), 2);
        assert_eq!(right.unwrap().messages.len(), 2);
        let mut changed = policy(owner).config().clone();
        changed.review.generation_mode = GenerationMode::Thinking {
            effort: ReasoningEffort::High,
        };
        changed.review.max_output_tokens = 200;
        let next = AiService::new(
            store.clone(),
            provider.clone(),
            CredentialCipher::new(&[5; 32]).unwrap(),
            AiPolicy::new(changed, "Different style".into(), "Different review".into()).unwrap(),
        );
        next.save_models(
            owner,
            ModelPreferences {
                summary: DeepSeekModel::Flash,
                verification: Some(DeepSeekModel::Pro),
            },
        )
        .await
        .unwrap();
        next.work_once().await.unwrap();
        let checked = service.chat(owner, id).await.unwrap();
        assert_eq!(checked.status, ChatStatus::Completed);
        assert_eq!(checked.messages[1].content, "**Title**\n\nChecked summary");
        assert_eq!(checked.provider_calls.len(), 3);
        assert_eq!(
            checked.provider_calls[2]
                .usage
                .as_ref()
                .unwrap()
                .estimated_cost_usd
                .as_deref(),
            Some("0.000030448")
        );
        {
            let inputs = provider.inputs.lock().unwrap();
            assert_eq!(inputs.len(), 3);
            assert_eq!(inputs[1].review_draft, inputs[2].review_draft);
            assert_ne!(inputs[1].system, inputs[2].system); // retry uses the current correction protocol
            assert_eq!(inputs[2].model, "deepseek-v4-pro");
            assert_eq!(
                inputs[2].generation_mode,
                GenerationMode::standard(0.3).unwrap()
            );
            assert_eq!(inputs[2].max_output_tokens, 100);
            assert_eq!(inputs[2].article.text, "Exact source 12.5%.");
        }
        service
            .message(owner, id, Uuid::new_v4(), "Explain it".into())
            .await
            .unwrap();
        service.work_once().await.unwrap();
        assert_eq!(
            provider.inputs.lock().unwrap().len(),
            4,
            "follow-up uses exactly one request"
        );
        assert!(provider.inputs.lock().unwrap()[3].review_draft.is_none());
        service
            .save_models(owner, ModelPreferences::default())
            .await
            .unwrap();
    }

    // Invalid verbatim quotes never become an accepted summary; known usage of
    // both requests survives a semantic transport rejection.
    let (service, _, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![Step::Text("Unverified"), Step::BadQuote],
    )
    .await;
    service.work_once().await.unwrap();
    let bad = service.chat(owner, id).await.unwrap();
    assert_eq!(bad.status, ChatStatus::Failed);
    assert_eq!(bad.messages[0].content, "**Title**\n\nUnverified");
    assert!(bad.provider_calls.iter().all(|c| c.usage.is_some()));

    // Stop in the durable stage handoff must prevent the second paid request,
    // while retaining the complete private draft and its already known bill.
    for delete_key in [false, true] {
        let (service, provider, id) = fixture(
            store.clone(),
            owner,
            ws,
            article,
            vec![Step::Stop(delete_key)],
        )
        .await;
        service.work_once().await.unwrap();
        let stopped = service.chat(owner, id).await.unwrap();
        assert_eq!(stopped.status, ChatStatus::Cancelled);
        assert_eq!(provider.inputs.lock().unwrap().len(), 1);
        assert_eq!(stopped.provider_calls.len(), 1);
        assert!(stopped.provider_calls[0].usage.is_some());
        assert_eq!(
            stopped.messages[0].content,
            "**Title**\n\nUnverified draft retained after stop"
        );
        let saved = store.chat(owner, id).await.unwrap();
        assert!(
            matches!(&saved.operations[0].task,AttemptTask::Summary{draft:Some(text)} if text.contains("Unverified draft"))
        );
        if delete_key {
            store
                .save_credential(
                    owner,
                    CredentialCipher::new(&[5; 32])
                        .unwrap()
                        .encrypt(owner, "restored-test-key")
                        .unwrap(),
                    Balance {
                        available: true,
                        balances: vec![],
                        updated_at: Utc::now(),
                    },
                )
                .await
                .unwrap();
        }
    }

    // A lost worker during verification is not repeated by another worker.
    // Explicit retry consumes the persisted draft; late billing from the old
    // request cannot overwrite the newer assistant or revive its expired lease.
    let entered = Arc::new(Notify::new());
    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("**Title**\n\nUnverified before crash"),
            Step::Wait(entered.clone()),
            Step::Text("**Title**\n\nVerified after restart"),
        ],
    )
    .await;
    let worker = {
        let service = service.clone();
        tokio::spawn(async move { service.work_once().await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    let preview = service.chat(owner, id).await.unwrap();
    assert_eq!(preview.status, ChatStatus::Verifying);
    assert_eq!(
        preview.messages[0].content,
        "**Title**\n\nUnverified before crash"
    );
    assert_ne!(preview.messages[0].status, MessageStatus::Complete);
    assert!(service.chat(Uuid::new_v4(), id).await.is_err());
    let listed = service.chats(owner, ws, article).await.unwrap();
    assert_eq!(
        listed.iter().find(|chat| chat.id == id).unwrap().messages[0].content,
        "**Title**\n\nUnverified before crash"
    );
    let old_record = store.chat(owner, id).await.unwrap();
    assert_eq!(
        old_record.view.messages[0].content,
        "Partial verifier output"
    );
    let mut corrupt = old_record.clone();
    corrupt.operations[0].task = AttemptTask::Summary {
        draft: Some("malformed".into()),
    };
    assert!(matches!(corrupt.into_public_view(), Err(AiError::Protocol)));
    let mut forged_quote = old_record.clone();
    forged_quote.operations[0].task = AttemptTask::Summary {
        draft: Some(
            serde_json::json!({"segments":[{"kind":"quote","content":"invented quote"}]})
                .to_string(),
        ),
    };
    assert!(matches!(
        forged_quote.into_public_view(),
        Err(AiError::Quote)
    ));

    let lease: Uuid = sqlx::query_scalar("SELECT lease FROM ai_chats WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    let old_claim = ClaimedChat {
        record: old_record.clone(),
        lease,
    };
    sqlx::query("UPDATE ai_chats SET lease_until=now()-interval '1 second' WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    assert!(store.claim(5).await.unwrap().is_none());
    worker.abort();
    let interrupted = service.chat(owner, id).await.unwrap();
    assert_eq!(interrupted.status, ChatStatus::Interrupted);
    assert_eq!(
        interrupted.messages[0].content,
        "**Title**\n\nUnverified before crash"
    );
    assert!(interrupted.provider_calls[0].usage.is_some());
    assert!(interrupted.provider_calls[1].usage.is_none());
    assert_eq!(
        interrupted.provider_calls[1].status,
        CallStatus::Interrupted
    );
    let retry = service.retry(owner, id, Uuid::new_v4()).await.unwrap();
    assert_eq!(
        retry.messages.last().unwrap().content,
        "**Title**\n\nUnverified before crash"
    );
    assert_eq!(
        retry.messages.last().unwrap().phase,
        Some(GenerationPhase::Verifying)
    );
    service.work_once().await.unwrap();
    assert_eq!(provider.inputs.lock().unwrap().len(), 3);
    let completed = service.chat(owner, id).await.unwrap();
    assert_eq!(
        completed.messages.last().unwrap().content,
        "**Title**\n\nVerified after restart"
    );
    let mut late = usage();
    late.estimated_cost_usd = Some(policy(owner).cost_rates().cost(&late).unwrap());
    store
        .update_call(
            &old_claim,
            old_record.view.provider_calls[1].id,
            CallUpdate::Usage(late),
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .update_claim(
                &old_claim,
                ChatStatus::Completed,
                Some("stale overwrite"),
                None,
                None
            )
            .await,
        Err(AiError::Cancelled)
    ));
    let final_view = service.chat(owner, id).await.unwrap();
    assert_eq!(final_view.status, ChatStatus::Completed);
    assert_eq!(
        final_view.messages.last().unwrap().content,
        "**Title**\n\nVerified after restart"
    );
    assert!(final_view.provider_calls.iter().all(|c| c.usage.is_some()));

    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("**Title**\n\nUnverified before stop"),
            Step::Stop(false),
        ],
    )
    .await;
    service.work_once().await.unwrap();
    let stopped = service.chat(owner, id).await.unwrap();
    assert_eq!(stopped.status, ChatStatus::Cancelled);
    assert_eq!(provider.inputs.lock().unwrap().len(), 2);
    assert_eq!(
        stopped.messages[0].content,
        "**Title**\n\nUnverified before stop"
    );
    assert!(stopped.provider_calls.iter().all(|c| c.usage.is_some()));

    // Repository boundaries independently reject publishing a raw draft,
    // bypassing review, starting it without a draft, forged billing and owners.
    let (service, _, id) = fixture(store.clone(), owner, ws, article, vec![]).await;
    let claim = store.claim(5).await.unwrap().unwrap();
    assert_eq!(claim.record.view.id, id);
    assert!(matches!(
        store
            .update_claim(
                &claim,
                ChatStatus::Generating,
                Some("raw draft"),
                None,
                None
            )
            .await,
        Err(AiError::Conflict)
    ));
    assert!(matches!(
        store
            .begin_call(
                &claim,
                GenerationPhase::Verifying,
                &reader_ai::SpendReservation::new(
                    "0.1".into(),
                    "3".into(),
                    reader_ai::SpendMode::Summary
                )
                .unwrap(),
                &reader_ai::CallModel::new(
                    reader_ai::DeepSeekModel::Flash,
                    claim.record.review.cost_rates.clone()
                ),
            )
            .await,
        Err(AiError::Conflict)
    ));
    let call = store
        .begin_call(
            &claim,
            GenerationPhase::Generating,
            &reader_ai::SpendReservation::new(
                "0.1".into(),
                "3".into(),
                reader_ai::SpendMode::Summary,
            )
            .unwrap(),
            &reader_ai::CallModel::new(
                reader_ai::DeepSeekModel::Flash,
                claim.record.cost_rates.clone(),
            ),
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .update_claim(&claim, ChatStatus::Completed, None, None, None)
            .await,
        Err(AiError::Conflict)
    ));
    let mut fabricated = usage();
    fabricated.estimated_cost_usd = Some("0".into());
    assert!(matches!(
        store
            .update_call(&claim, call, CallUpdate::Usage(fabricated.clone()))
            .await,
        Err(AiError::Protocol)
    ));
    let mut forged = claim.clone();
    forged.record.owner = Uuid::new_v4();
    assert!(matches!(
        store
            .update_call(&forged, call, CallUpdate::Usage(fabricated))
            .await,
        Err(AiError::NotFound)
    ));
    service.stop(owner, id).await.unwrap();
    assert!(matches!(
        store
            .begin_call(
                &claim,
                GenerationPhase::Verifying,
                &reader_ai::SpendReservation::new(
                    "0.1".into(),
                    "3".into(),
                    reader_ai::SpendMode::Summary
                )
                .unwrap(),
                &reader_ai::CallModel::new(
                    reader_ai::DeepSeekModel::Flash,
                    claim.record.review.cost_rates.clone()
                ),
            )
            .await,
        Err(AiError::Cancelled)
    ));
    unchecked::verify(pool, store.clone(), owner, ws, article).await;
    review::verify(pool, store, owner, ws, article).await;
}
