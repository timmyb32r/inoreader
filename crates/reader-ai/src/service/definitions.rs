use super::*;
use crate::{DefinitionState, DefinitionsInput, DefinitionsJob, DefinitionsRecord};

impl AiService {
    pub async fn definitions(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Option<DefinitionsJob>, AiError> {
        self.store.definitions(owner, workspace, article).await
    }
    pub async fn define(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        operation: Uuid,
        regenerate: bool,
    ) -> Result<DefinitionsJob, AiError> {
        self.define_internal(owner, workspace, article, operation, regenerate, false)
            .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn define_internal(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        operation: Uuid,
        regenerate: bool,
        automatic: bool,
    ) -> Result<DefinitionsJob, AiError> {
        log::info!("ai_submission operation_id={operation}");
        self.policy.generation_allowed(owner)?;
        self.key(owner).await?;
        self.manual_budget(owner).await?;
        let ArticleInput::Ready(snapshot) =
            self.store.article_input(owner, workspace, article).await?
        else {
            return Err(AiError::FullText);
        };
        let input = DefinitionsInput::new(self.policy.config(), snapshot)?;
        let record = DefinitionsRecord::new(
            owner,
            workspace,
            article,
            input,
            self.policy.cost_rates().clone(),
        )?;
        self.store
            .create_definitions(record, operation, regenerate, automatic)
            .await
            .inspect(|job| {
                log::info!(
                    "ai_operation_bound operation_id={operation} job_id={}",
                    job.id
                )
            })
    }
    pub async fn define_once(&self) -> Result<bool, AiError> {
        let Some(claim) = self
            .store
            .claim_definitions(self.policy.config().lease_seconds)
            .await?
        else {
            return Ok(false);
        };
        reader_runtime::Context::operation(claim.record.job().id)
            .scope(async {
                let response = async {
                    self.policy.generation_allowed(claim.record.owner())?;
                    let key = self.key(claim.record.owner()).await?;
                    let reservation = SpendReservation::new(
                        claim
                            .record
                            .input()
                            .budget_cost(claim.record.cost_rates())?,
                        self.policy.config().daily_limit_usd.clone(),
                        SpendMode::Terms,
                    )?;
                    self.store
                        .reserve(claim.record.owner(), claim.lease, &reservation)
                        .await?;
                    reader_runtime::observe(
                        reader_runtime::Stage::DefinitionsProvider,
                        self.provider
                            .definitions(&key, claim.record.input().clone()),
                    )
                    .await
                }
                .await;
                if matches!(response, Err(AiError::Budget | AiError::AutomaticPaused)) {
                    if let Some(interests) = &self.interests {
                        interests
                            .defer_terms(
                                &claim,
                                if matches!(response, Err(AiError::AutomaticPaused)) {
                                    crate::AiDeferral::PeakHours
                                } else {
                                    crate::AiDeferral::DailyBudget
                                },
                            )
                            .await?;
                        return Ok(true);
                    }
                }
                let (state, usage) = match response {
                    Ok(reply) => {
                        self.store
                            .retain_reply(
                                crate::ReplyKind::Definitions,
                                claim.record.owner(),
                                claim.record.job().id,
                                &reply,
                            )
                            .await?;
                        let mut usage = reply.usage();
                        if let Some(value) = &mut usage {
                            value.estimated_cost_usd = Some(claim.record.cost_rates().cost(value)?);
                            self.store
                                .settle(
                                    claim.record.owner(),
                                    claim.lease,
                                    value
                                        .estimated_cost_usd
                                        .as_deref()
                                        .ok_or(AiError::Storage)?,
                                )
                                .await?;
                        }
                        let state = match reader_runtime::observe_sync(
                            reader_runtime::Stage::DefinitionsValidation,
                            || reply.definition_result(claim.record.input().snapshot()),
                        ) {
                            Ok(result) => DefinitionState::Completed { result },
                            Err(error) => DefinitionState::Failed {
                                error: error.to_string(),
                            },
                        };
                        (state, usage)
                    }
                    Err(AiError::AutomaticDuplicate) => (
                        DefinitionState::Cancelled {
                            reason: AiError::AutomaticDuplicate.to_string(),
                        },
                        None,
                    ),
                    Err(error) => (
                        DefinitionState::Failed {
                            error: error.to_string(),
                        },
                        None,
                    ),
                };
                self.store.finish_definitions(&claim, state, usage).await?;
                Ok(true)
            })
            .await
    }
}
