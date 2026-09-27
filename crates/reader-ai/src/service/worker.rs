use super::*;
use async_trait::async_trait;
use std::time::Duration;

impl AiService {
    pub fn spawn_workers(self: &Arc<Self>) {
        for _ in 0..self.policy.config().workers {
            let service = self.clone();
            tokio::spawn(async move {
                loop {
                    if let Err(error) = service.work_once().await {
                        log::error!("ai_worker outcome=failed classification={error}");
                    }
                    tokio::time::sleep(Duration::from_millis(
                        service.policy.config().poll_milliseconds,
                    ))
                    .await;
                }
            });
        }
    }

    /// Expired claims require an explicit retry, including a saved draft waiting
    /// for verification. There is no automatic repeat of an uncertain request.
    pub async fn work_once(&self) -> Result<bool, AiError> {
        let Some(mut claim) = self.store.claim(self.policy.config().lease_seconds).await? else {
            return Ok(false);
        };
        if let Err(error) = self.generate_claim(&mut claim).await {
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
                self.input(claim, Some("{\"segments\":[]}".into()))?
                    .validate_context()?;
                let draft = match draft {
                    Some(saved) => saved,
                    None => self.run_call(claim, None, true).await?,
                };
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
                settings.system_prompt.clone()
            } else {
                record.system_prompt.clone()
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
        let input = self.input(claim, draft)?;
        input.validate_context()?;
        let rates = if phase == GenerationPhase::Verifying {
            &claim.record.review.cost_rates
        } else {
            &claim.record.cost_rates
        };
        let call = self.store.begin_call(claim, phase).await?;
        let mut progress = StoredProgress {
            store: self.store.as_ref(),
            claim,
            call,
            phase,
            rates,
            private_draft,
        };
        match self.provider.generate(&key, input, &mut progress).await {
            Ok(output) => {
                progress.usage(&output.usage).await?;
                if phase == GenerationPhase::Verifying {
                    progress.check_active().await?;
                    if let Err(error) = validate_summary_title(
                        &output.content,
                        &claim
                            .record
                            .snapshot
                            .as_ref()
                            .ok_or(AiError::FullText)?
                            .title,
                    ) {
                        self.store
                            .update_call(
                                claim,
                                call,
                                CallUpdate::Finished {
                                    status: CallStatus::Failed,
                                    draft: None,
                                },
                            )
                            .await?;
                        return Err(error);
                    }
                }
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
    async fn usage(&mut self, usage: &Usage) -> Result<(), AiError> {
        let mut usage = usage.clone();
        usage.estimated_cost_usd = Some(self.rates.cost(&usage)?);
        self.store
            .update_call(self.claim, self.call, CallUpdate::Usage(usage))
            .await
    }
}
