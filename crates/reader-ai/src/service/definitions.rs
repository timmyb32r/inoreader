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
        self.policy.generation_allowed(owner)?;
        self.key(owner).await?;
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
            .create_definitions(record, operation, regenerate)
            .await
    }
    pub async fn define_once(&self) -> Result<bool, AiError> {
        let Some(claim) = self
            .store
            .claim_definitions(self.policy.config().lease_seconds)
            .await?
        else {
            return Ok(false);
        };
        let response = async {
            self.policy.generation_allowed(claim.record.owner())?;
            let key = self.key(claim.record.owner()).await?;
            self.provider
                .definitions(&key, claim.record.input().clone())
                .await
        }
        .await;
        let (state, usage, reply) = match response {
            Ok(reply) => {
                let mut usage = reply.usage();
                if let Some(value) = &mut usage {
                    value.estimated_cost_usd = Some(claim.record.cost_rates().cost(value)?);
                }
                let state = match reply.result(claim.record.input().snapshot()) {
                    Ok(result) => DefinitionState::Completed { result },
                    Err(error) => DefinitionState::Failed {
                        error: error.to_string(),
                    },
                };
                (state, usage, Some(reply))
            }
            Err(error) => (
                DefinitionState::Failed {
                    error: error.to_string(),
                },
                None,
                None,
            ),
        };
        self.store
            .finish_definitions(&claim, state, usage, reply)
            .await?;
        Ok(true)
    }
}
