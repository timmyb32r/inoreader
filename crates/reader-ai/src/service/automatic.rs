use super::*;
impl AiService {
    pub(super) async fn manual_budget(&self, owner: Uuid) -> Result<(), AiError> {
        let spending = self
            .store
            .spending(owner, &self.policy.config().daily_limit_usd)
            .await?;
        if !spending
            .remaining_usd
            .bytes()
            .any(|b| matches!(b, b'1'..=b'9'))
        {
            return Err(AiError::Budget);
        }
        Ok(())
    }
    pub async fn initialize_automatic(&self) -> Result<(), AiError> {
        let config = self.policy.config();
        self.store
            .configure_automatic_schedule(&config.automatic_schedule)
            .await?;
        self.store
            .configure_initial_articles(config.initial_articles)
            .await?;
        if config.automatic_summaries && config.prompt_approved {
            self.store
                .enroll_summaries(&config.enabled_accounts)
                .await?;
        }
        Ok(())
    }
    pub async fn schedule_once(&self) -> Result<bool, AiError> {
        let config = self.policy.config();
        if (!config.automatic_summaries && !config.automatic_terms) || !config.prompt_approved {
            return Ok(false);
        }
        self.store
            .enroll_summaries(&config.enabled_accounts)
            .await?;
        if let Some(interests) = self.interests.as_ref().filter(|_| config.automatic_terms) {
            if let Some((owner, workspace, article)) = interests
                .next_terms(&config.enabled_accounts, crate::DEFINITIONS_VERSION)
                .await?
            {
                // Retained jobs are the durable queue; no paid request occurs in
                // the scheduler. Context and current daily budget are checked.
                match self
                    .define_internal(owner, workspace, article, Uuid::new_v4(), false, true)
                    .await
                {
                    Ok(_) | Err(AiError::Budget) => {}
                    Err(AiError::FullText | AiError::Context) => {
                        interests
                            .reject_terms(
                                owner,
                                workspace,
                                article,
                                crate::DEFINITIONS_VERSION,
                                "full_text_or_context_unavailable",
                            )
                            .await?;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        if !config.automatic_summaries {
            return Ok(false);
        }
        let Some(target) = self
            .store
            .next_summary(
                &config.enabled_accounts,
                config.automatic_attempts,
                config.automatic_retry_seconds,
            )
            .await?
        else {
            return Ok(false);
        };
        let result = self
            .start_internal(
                target.owner,
                target.workspace,
                target.article,
                Uuid::new_v4(),
                false,
                false,
            )
            .await;
        self.store
            .finish_summary(
                &target,
                result.as_ref().err().map(ToString::to_string).as_deref(),
            )
            .await?;
        result.map(|_| true)
    }
}
