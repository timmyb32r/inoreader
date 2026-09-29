use super::*;

impl AiService {
    pub async fn translations(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ParagraphJob>, AiError> {
        self.store.translations(owner, workspace, article).await
    }
    pub async fn translate(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        operation: Uuid,
        source: String,
    ) -> Result<ParagraphJob, AiError> {
        log::info!("ai_submission operation_id={operation}");
        self.policy.generation_allowed(owner)?;
        let input = TranslationInput::new(self.policy.config(), &source)?;
        self.key(owner).await?;
        self.manual_budget(owner).await?;
        let snapshot = match self.store.article_input(owner, workspace, article).await? {
            ArticleInput::Ready(s) => s,
            _ => return Err(AiError::FullText),
        };
        if !contains_paragraph(&snapshot.safe_html, &source)
            && !self
                .store
                .article_intro_contains(owner, workspace, article, &source)
                .await?
        {
            return Err(AiError::Message);
        }
        self.store
            .create_translation(TranslationRecord {
                owner,
                input: Some(input),
                source_revision: snapshot.source_revision,
                cost_rates: self.policy.cost_rates().clone(),
                job: ParagraphJob {
                    id: operation,
                    workspace_id: workspace,
                    article_id: article,
                    source,
                    model: crate::DeepSeekModel::Flash.id().into(),
                    state: TranslationState::Queued,
                    usage: None,
                },
            })
            .await
    }
    pub async fn translate_once(&self) -> Result<bool, AiError> {
        let Some(claim) = self
            .store
            .claim_translation(self.policy.config().lease_seconds)
            .await?
        else {
            return Ok(false);
        };
        reader_runtime::Context::operation(claim.record.job.id)
            .scope(async {
                let result = async {
                    self.policy.generation_allowed(claim.record.owner)?;
                    let key = self.key(claim.record.owner).await?;
                    let input = claim.record.input.clone().ok_or(AiError::Unavailable)?;
                    let reservation = SpendReservation::new(
                        input.budget_cost(&claim.record.cost_rates)?,
                        self.policy.config().daily_limit_usd.clone(),
                        SpendMode::Translation,
                    )?;
                    self.store
                        .reserve(claim.record.owner, claim.lease, &reservation)
                        .await?;
                    reader_runtime::observe(
                        reader_runtime::Stage::TranslationProvider,
                        self.provider.translate(&key, input),
                    )
                    .await
                }
                .await;
                let (state, usage) = match result {
                    Ok(reply) => {
                        self.store
                            .retain_reply(
                                crate::ReplyKind::Translation,
                                claim.record.owner,
                                claim.record.job.id,
                                &reply,
                            )
                            .await?;
                        let mut usage = reply.usage();
                        if let Some(usage) = &mut usage {
                            usage.estimated_cost_usd = Some(claim.record.cost_rates.cost(usage)?);
                            self.store
                                .settle(
                                    claim.record.owner,
                                    claim.lease,
                                    usage
                                        .estimated_cost_usd
                                        .as_deref()
                                        .ok_or(AiError::Storage)?,
                                )
                                .await?;
                        }
                        let state = match reader_runtime::observe_sync(
                            reader_runtime::Stage::TranslationValidation,
                            || reply.translation_result(&claim.record.job.source),
                        ) {
                            Ok(result) => TranslationState::Completed { result },
                            Err(error) => TranslationState::Failed {
                                error: error.to_string(),
                            },
                        };
                        (state, usage)
                    }
                    Err(error) => (
                        TranslationState::Failed {
                            error: error.to_string(),
                        },
                        None,
                    ),
                };
                self.store.finish_translation(&claim, state, usage).await?;
                Ok(true)
            })
            .await
    }
}
