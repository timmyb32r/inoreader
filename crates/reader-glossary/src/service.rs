use crate::*;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub struct GlossaryService {
    store: Arc<dyn GlossaryStore>,
    gateway: Arc<dyn TelegramGateway>,
    vault: Arc<dyn GlossaryVault>,
    policy: GlossaryPolicy,
    enabled_accounts: Vec<Uuid>,
}
impl GlossaryService {
    pub fn new(
        store: Arc<dyn GlossaryStore>,
        gateway: Arc<dyn TelegramGateway>,
        vault: Arc<dyn GlossaryVault>,
        policy: GlossaryPolicy,
        enabled_accounts: Vec<Uuid>,
    ) -> Self {
        Self {
            store,
            gateway,
            vault,
            policy,
            enabled_accounts,
        }
    }
    pub async fn status(
        &self,
        owner: Uuid,
        workspace: Uuid,
    ) -> Result<ChannelStatus, GlossaryError> {
        let mut status = self.store.status(owner, workspace).await?;
        status.generation_allowed = self.enabled_accounts.contains(&owner);
        Ok(status)
    }
    pub async fn configure(
        &self,
        owner: Uuid,
        workspace: Uuid,
        token: String,
    ) -> Result<ChannelStatus, GlossaryError> {
        if !self.enabled_accounts.contains(&owner) {
            return Err(GlossaryError::NotFound);
        }
        self.store.status(owner, workspace).await?;
        let token = BotToken::new(token)?;
        let binding = self.gateway.discover(&token).await?;
        self.store
            .configure(
                owner,
                workspace,
                &binding,
                self.vault.seal(owner, token.expose())?,
            )
            .await?;
        self.status(owner, workspace).await
    }
    pub async fn sync(&self, owner: Uuid, workspace: Uuid) -> Result<ChannelStatus, GlossaryError> {
        if !self.enabled_accounts.contains(&owner) {
            return Err(GlossaryError::NotFound);
        }
        self.store.request_sync(owner, workspace).await?;
        self.status(owner, workspace).await
    }
    pub async fn lookup(
        &self,
        owner: Uuid,
        workspace: Uuid,
        terms: &[String],
    ) -> Result<Vec<KnownDefinition>, GlossaryError> {
        self.store.lookup(owner, workspace, terms).await
    }
    pub async fn poll_once(&self) -> Result<bool, GlossaryError> {
        let Some(claim) = self
            .store
            .claim_poll(self.policy.config().lease_seconds)
            .await?
        else {
            return Ok(false);
        };
        let result = async {
            let token = BotToken::new(self.vault.open(claim.owner, &claim.encrypted_token)?)?;
            let batch = self.gateway.updates(&token, claim.offset).await?;
            self.store.commit_batch(&claim, &batch).await
        }
        .await;
        let error = result.as_ref().err().map(ToString::to_string);
        let retry = match &result {
            Err(GlossaryError::RateLimited(seconds)) => *seconds,
            Err(_) => self.policy.config().retry_initial_seconds,
            Ok(_) => 0,
        };
        self.store
            .finish_poll(&claim, error.as_deref(), retry)
            .await?;
        result.map(|_| true)
    }
    pub async fn history_once(&self) -> Result<bool, GlossaryError> {
        let Some(claim) = self
            .store
            .claim_history(self.policy.config().lease_seconds)
            .await?
        else {
            return Ok(false);
        };
        match self.gateway.history(claim.before).await {
            Ok(page) => {
                let delay = if page.before.is_none() {
                    self.policy.config().history_reconcile_interval_seconds
                } else {
                    self.policy
                        .config()
                        .public_page_interval_milliseconds
                        .div_ceil(1000)
                };
                self.store.commit_page(&claim, &page, delay).await?;
                Ok(true)
            }
            Err(error) => {
                self.store
                    .fail_history(
                        &claim,
                        &error.to_string(),
                        self.policy.config().retry_max_seconds,
                    )
                    .await?;
                Err(error)
            }
        }
    }
    pub fn spawn_workers(self: &Arc<Self>) -> Vec<tokio::task::JoinHandle<()>> {
        let mut workers = Vec::new();
        for _ in 0..self.policy.config().workers {
            let service = self.clone();
            workers.push(tokio::spawn(async move {
                loop {
                    if let Err(error) = service.poll_once().await {
                        log::error!("telegram_poll outcome=failed classification={error}");
                    }
                    tokio::time::sleep(Duration::from_millis(
                        service.policy.config().worker_poll_milliseconds,
                    ))
                    .await;
                }
            }));
        }
        let service = self.clone();
        workers.push(tokio::spawn(async move {
            loop {
                if let Err(error) = service
                    .store
                    .apply_pending(service.policy.config().batch_size)
                    .await
                {
                    log::error!("telegram_projection outcome=failed classification={error}");
                }
                tokio::time::sleep(Duration::from_millis(
                    service.policy.config().worker_poll_milliseconds,
                ))
                .await;
            }
        }));
        let service = self.clone();
        workers.push(tokio::spawn(async move {
            loop {
                for _ in 0..service.policy.config().history_pages_per_run {
                    match service.history_once().await {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(error) => {
                            log::error!("telegram_history outcome=failed classification={error}");
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(
                        service.policy.config().public_page_interval_milliseconds,
                    ))
                    .await;
                }
                tokio::time::sleep(Duration::from_millis(
                    service.policy.config().worker_poll_milliseconds,
                ))
                .await;
            }
        }));
        workers
    }
}
