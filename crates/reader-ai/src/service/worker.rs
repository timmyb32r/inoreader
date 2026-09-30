use super::*;
use async_trait::async_trait;
use std::time::Duration;

impl AiService {
    pub fn spawn_workers(self: &Arc<Self>, supervisor: &mut reader_runtime::TaskSupervisor) {
        let scheduler = self.clone();
        supervisor.spawn("ai-scheduler", move |mut stop| async move {
            while !stop.requested() {
                if let Err(error) = scheduler.schedule_once().await {
                    log::error!("ai_scheduler outcome=failed classification={error}");
                }
                stop.sleep(Duration::from_millis(
                    scheduler.policy.config().poll_milliseconds,
                ))
                .await;
            }
            Ok(())
        });
        // Separate work classes share one FIFO permit pool: slow definitions do
        // not force every chat to wait behind a definition and a translation.
        let slots = Arc::new(tokio::sync::Semaphore::new(self.policy.config().workers));
        for class in ["definitions", "translation", "chat"] {
            for _ in 0..self.policy.config().workers {
                let service = self.clone();
                let slots = slots.clone();
                supervisor.spawn(class,move |mut stop|async move {
                    while !stop.requested() {
                        let permit=tokio::select! {
                            _=stop.wait()=>break,
                            permit=slots.acquire()=>permit.map_err(|_|"AI admission closed".to_string())?,
                        };
                        if stop.requested(){break;}
                        let result=match class {
                            "definitions"=>service.define_once().await,
                            "translation"=>service.translate_once().await,
                            _=>service.work_once().await,
                        };
                        drop(permit);
                        if let Err(error)=result {log::error!("ai_worker class={class} outcome=failed classification={error}");}
                        stop.sleep(Duration::from_millis(service.policy.config().poll_milliseconds)).await;
                    }
                    Ok(())
                });
            }
        }
    }

    /// Expired claims require an explicit retry, including a saved draft waiting
    /// for verification. There is no automatic repeat of an uncertain request.
    pub async fn work_once(&self) -> Result<bool, AiError> {
        let Some(mut claim) = self.store.claim(self.policy.config().lease_seconds).await? else {
            return Ok(false);
        };
        reader_runtime::Context::job(
            claim.record.operations.last().ok_or(AiError::Storage)?.id,
            claim.record.view.id,
        )
        .scope(async {
            if let Err(error) = self.generate_claim(&mut claim).await {
                if matches!(error, AiError::Budget)
                    && matches!(
                        claim.record.operations.last().map(|op| &op.task),
                        Some(AttemptTask::Summary { .. })
                    )
                {
                    self.store.defer_budget(&claim).await?;
                    return Ok(true);
                }
                if !matches!(error, AiError::Cancelled) {
                    let state = if matches!(
                        error,
                        AiError::Provider | AiError::Protocol | AiError::Storage
                    ) {
                        ChatStatus::Interrupted
                    } else {
                        ChatStatus::Failed
                    };
                    match self
                        .store
                        .update_claim(&claim, state, None, None, Some(&error.to_string()))
                        .await
                    {
                        Ok(()) | Err(AiError::Cancelled) => {}
                        Err(error) => return Err(error),
                    }
                }
            }
            Ok(true)
        })
        .await
    }

    async fn generate_claim(&self, claim: &mut ClaimedChat) -> Result<(), AiError> {
        self.policy.generation_allowed(claim.record.owner)?;
        if claim.record.snapshot.is_none() {
            match self
                .store
                .article_input(
                    claim.record.owner,
                    claim.record.view.workspace_id,
                    claim.record.view.article_id,
                )
                .await?
            {
                ArticleInput::Ready(snapshot) => {
                    self.store
                        .update_claim(claim, ChatStatus::Generating, None, Some(&snapshot), None)
                        .await?;
                    claim.record.snapshot = Some(snapshot);
                }
                ArticleInput::Waiting { .. } => {
                    self.store
                        .update_claim(claim, ChatStatus::WaitingContent, None, None, None)
                        .await?;
                    return Ok(());
                }
                ArticleInput::Failed => return Err(AiError::FullText),
            }
        }
        let snapshot = claim.record.snapshot.as_ref().ok_or(AiError::FullText)?;
        if snapshot.text.trim().is_empty() {
            return Err(AiError::FullText);
        }
        let task = claim
            .record
            .operations
            .last()
            .ok_or(AiError::Storage)?
            .task
            .clone();
        match task {
            AttemptTask::Reply => {
                self.run_call(claim, None, false).await?;
            }
            AttemptTask::Summary { draft } => {
                // Reject a review that cannot even fit its source before paying
                // for a draft. The actual full draft is checked again below.
                if self
                    .store
                    .model_preferences(claim.record.owner)
                    .await?
                    .verification
                    .is_some()
                {
                    self.input(claim, Some("{\"segments\":[]}".into()))?
                        .validate_context()?;
                }
                let draft = match draft {
                    Some(saved) => saved,
                    None => self.run_call(claim, None, true).await?,
                };
                if self.store.finish_unchecked(claim).await? {
                    return Ok(());
                }
                self.run_call(claim, Some(draft), false).await?;
            }
        }
        self.store
            .update_claim(claim, ChatStatus::Completed, None, None, None)
            .await
    }

    fn input(
        &self,
        claim: &ClaimedChat,
        draft: Option<String>,
    ) -> Result<GenerationInput, AiError> {
        let record = &claim.record;
        let review = draft.is_some();
        let settings = &record.review;
        Ok(GenerationInput {
            model: if review {
                settings.model.clone()
            } else {
                record.view.model.clone()
            },
            generation_mode: if review {
                settings.generation_mode
            } else {
                record.generation_mode
            },
            system: if review {
                self.policy.review_snapshot()?.system_prompt
            } else {
                format!("{}\nFor the initial summary, omit the heading/title segment: the application supplies the exact source title. All other style rules remain in force.", record.system_prompt)
            },
            article: record.snapshot.clone().ok_or(AiError::FullText)?,
            messages: if review {
                Vec::new()
            } else {
                record.view.messages.clone()
            },
            max_output_tokens: if review {
                settings.max_output_tokens
            } else {
                record.max_output_tokens
            },
            max_response_bytes: record.limits.max_response_bytes,
            context_tokens: record.limits.context_tokens,
            framing_tokens_per_message: record.limits.framing_tokens_per_message,
            framing_tokens_base: record.limits.framing_tokens_base,
            max_input_bytes: record.limits.max_input_bytes,
            review_draft: draft,
        })
    }

    async fn run_call(
        &self,
        claim: &ClaimedChat,
        draft: Option<String>,
        private_draft: bool,
    ) -> Result<String, AiError> {
        self.policy.generation_allowed(claim.record.owner)?;
        let key = self.key(claim.record.owner).await?;
        let phase = if draft.is_some() {
            GenerationPhase::Verifying
        } else {
            GenerationPhase::Generating
        };
        let mut input = self.input(claim, draft)?;
        let call_system = input.system.clone();
        let preferences = self.store.model_preferences(claim.record.owner).await?;
        let model = if phase == GenerationPhase::Verifying {
            preferences.verification.ok_or(AiError::Conflict)?
        } else {
            preferences.summary
        };
        let selection = CallModel::new(model, self.policy.config().models.get(model).clone());
        input.model = model.id().into();
        input.validate_context()?;
        let rates = selection.rates();
        let mode = if phase == GenerationPhase::Verifying {
            SpendMode::Verification
        } else if matches!(
            claim.record.operations.last().map(|op| &op.task),
            Some(AttemptTask::Reply)
        ) {
            SpendMode::Chat
        } else {
            SpendMode::Summary
        };
        let amount = crate::budget::bound_cost(
            &input.messages_json()?,
            input.framing_tokens_per_message,
            input.framing_tokens_base,
            input.max_output_tokens,
            rates,
        )?;
        let reservation =
            SpendReservation::new(amount, self.policy.config().daily_limit_usd.clone(), mode)?;
        let call = self
            .store
            .begin_call(claim, phase, &reservation, &selection)
            .await?;
        let mut progress = StoredProgress {
            store: self.store.as_ref(),
            claim,
            call,
            phase,
            rates,
            private_draft,
        };
        match reader_runtime::observe(
            reader_runtime::Stage::ChatProvider,
            self.provider.generate(&key, input, &mut progress),
        )
        .await
        {
            Ok(output) => {
                progress.usage(&output.usage).await?;
                progress.response(&output.envelope, &call_system).await?;
                if phase == GenerationPhase::Verifying {
                    progress.check_active().await?;
                }
                let output = if private_draft {
                    let source = claim.record.snapshot.as_ref().ok_or(AiError::FullText)?;
                    output.with_source_title(
                        &source.title,
                        &source.text,
                        claim.record.limits.max_response_bytes,
                    )?
                } else {
                    output
                };
                if !private_draft {
                    progress.publish(&output.content).await?;
                }
                self.store
                    .update_call(
                        claim,
                        call,
                        CallUpdate::Finished {
                            status: CallStatus::Completed,
                            draft: private_draft.then(|| output.envelope.clone()),
                        },
                    )
                    .await?;
                Ok(output.envelope)
            }
            Err(error) => {
                let state = match error {
                    AiError::Cancelled => CallStatus::Cancelled,
                    AiError::Provider | AiError::Protocol | AiError::Storage => {
                        CallStatus::Interrupted
                    }
                    _ => CallStatus::Failed,
                };
                self.store
                    .update_call(
                        claim,
                        call,
                        CallUpdate::Finished {
                            status: state,
                            draft: None,
                        },
                    )
                    .await?;
                Err(error)
            }
        }
    }
}

struct StoredProgress<'a> {
    store: &'a dyn AiStore,
    claim: &'a ClaimedChat,
    call: Uuid,
    phase: GenerationPhase,
    rates: &'a CostRates,
    private_draft: bool,
}
#[async_trait]
impl GenerationProgress for StoredProgress<'_> {
    async fn check_active(&mut self) -> Result<(), AiError> {
        if self
            .store
            .active(
                self.claim.record.owner,
                self.claim.record.view.id,
                self.claim.lease,
            )
            .await?
        {
            Ok(())
        } else {
            Err(AiError::Cancelled)
        }
    }
    async fn publish(&mut self, content: &str) -> Result<(), AiError> {
        self.check_active().await?;
        if self.private_draft {
            return Ok(());
        }
        self.store
            .update_claim(
                self.claim,
                match self.phase {
                    GenerationPhase::Generating => ChatStatus::Generating,
                    GenerationPhase::Verifying => ChatStatus::Verifying,
                },
                Some(content),
                None,
                None,
            )
            .await
    }
    async fn response(&mut self, content: &str, system: &str) -> Result<(), AiError> {
        self.store
            .update_call(
                self.claim,
                self.call,
                CallUpdate::Response {
                    content: content.into(),
                    system: system.into(),
                },
            )
            .await
    }
    async fn usage(&mut self, usage: &Usage) -> Result<(), AiError> {
        let mut usage = usage.clone();
        usage.estimated_cost_usd = Some(self.rates.cost(&usage)?);
        self.store
            .settle(
                self.claim.record.owner,
                self.call,
                usage
                    .estimated_cost_usd
                    .as_deref()
                    .ok_or(AiError::Storage)?,
            )
            .await?;
        self.store
            .update_call(self.claim, self.call, CallUpdate::Usage(usage))
            .await
    }
}
