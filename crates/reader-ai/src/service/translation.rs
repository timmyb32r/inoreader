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
        self.policy.generation_allowed(owner)?;
        TranslationInput::new(self.policy.config(), &source)?;
        self.key(owner).await?;
        let snapshot = match self.store.article_input(owner, workspace, article).await? {
            ArticleInput::Ready(s) => s,
            _ => return Err(AiError::FullText),
        };
        if !contains_paragraph(&snapshot.safe_html, &source) {
            return Err(AiError::Message);
        }
        self.store
            .create_translation(TranslationRecord {
                owner,
                source_revision: snapshot.source_revision,
                cost_rates: self.policy.cost_rates().clone(),
                job: ParagraphJob {
                    id: operation,
                    workspace_id: workspace,
                    article_id: article,
                    source,
                    model: self.policy.config().model.clone(),
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
        let result = async {
            self.policy.generation_allowed(claim.record.owner)?;
            let key = self.key(claim.record.owner).await?;
            let mut config = self.policy.config().clone();
            config.model = claim.record.job.model.clone();
            let input = TranslationInput::new(&config, &claim.record.job.source)?;
            self.provider.translate(&key, input).await
        }
        .await;
        let (state, usage) = match result {
            Ok((result, mut usage)) => {
                usage.estimated_cost_usd = Some(claim.record.cost_rates.cost(&usage)?);
                (TranslationState::Completed { result }, Some(usage))
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
    }
}
