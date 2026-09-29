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
        if config.automatic_summaries && config.prompt_approved {
            self.store
                .enroll_summaries(&config.enabled_accounts)
                .await?;
        }
        Ok(())
    }
    pub async fn schedule_once(&self) -> Result<bool, AiError> {
        let config = self.policy.config();
        if !config.automatic_summaries || !config.prompt_approved {
            return Ok(false);
        }
        self.store
            .enroll_summaries(&config.enabled_accounts)
            .await?;
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
