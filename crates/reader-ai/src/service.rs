use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use crate::*;

mod translation;
mod worker;

pub struct AiService {
    store: Arc<dyn AiStore>,
    provider: Arc<dyn AiProvider>,
    cipher: CredentialCipher,
    policy: AiPolicy,
}

impl AiService {
    pub fn new(
        store: Arc<dyn AiStore>,
        provider: Arc<dyn AiProvider>,
        cipher: CredentialCipher,
        policy: AiPolicy,
    ) -> Self {
        Self {
            store,
            provider,
            cipher,
            policy,
        }
    }
    pub async fn profile(&self, owner: Uuid) -> Result<AiProfile, AiError> {
        let (configured, balance, error) = self.store.profile(owner).await?;
        let availability = self
            .policy
            .generation_allowed(owner)
            .err()
            .or_else(|| (!configured).then_some(AiError::MissingKey));
        Ok(AiProfile {
            configured,
            enabled: availability.is_none(),
            balance,
            error,
            availability_reason: availability.map(|v| v.to_string()),
        })
    }
    pub async fn save_key(&self, owner: Uuid, key: String) -> Result<AiProfile, AiError> {
        if !self.policy.allowed(owner) {
            return Err(AiError::Unavailable);
        }
        if key.is_empty()
            || key
                .bytes()
                .any(|v| v.is_ascii_whitespace() || v.is_ascii_control())
        {
            return Err(AiError::InvalidKey);
        }
        let balance = self.provider.balance(&key).await?;
        self.store
            .save_credential(owner, self.cipher.encrypt(owner, &key)?, balance)
            .await?;
        self.profile(owner).await
    }
    pub async fn delete_key(&self, owner: Uuid) -> Result<AiProfile, AiError> {
        self.store.delete_credential(owner).await?;
        self.profile(owner).await
    }
    pub async fn balance(&self, owner: Uuid) -> Result<AiProfile, AiError> {
        if !self.policy.allowed(owner) {
            return Err(AiError::Unavailable);
        }
        let encrypted = self
            .store
            .credential(owner)
            .await?
            .ok_or(AiError::MissingKey)?;
        let key = self.cipher.decrypt(owner, &encrypted)?;
        match self.provider.balance(&key).await {
            Ok(value) => {
                self.store
                    .save_balance(owner, &encrypted, Some(value), None)
                    .await?
            }
            Err(error) => {
                self.store
                    .save_balance(owner, &encrypted, None, Some(error.to_string()))
                    .await?
            }
        }
        self.profile(owner).await
    }
    async fn key(&self, owner: Uuid) -> Result<String, AiError> {
        if !self.policy.allowed(owner) {
            return Err(AiError::Unavailable);
        }
        self.cipher.decrypt(
            owner,
            &self
                .store
                .credential(owner)
                .await?
                .ok_or(AiError::MissingKey)?,
        )
    }
    pub async fn chats(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ArticleChat>, AiError> {
        self.store
            .chats(owner, workspace, article)
            .await?
            .into_iter()
            .map(ChatRecord::into_public_view)
            .collect()
    }
    pub async fn chat(&self, owner: Uuid, id: Uuid) -> Result<ArticleChat, AiError> {
        self.store.chat(owner, id).await?.into_public_view()
    }
    pub async fn start(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        operation: Uuid,
        regenerate: bool,
    ) -> Result<ArticleChat, AiError> {
        let prior = self.store.chats(owner, workspace, article).await?;
        // Retried starts and reopening never need a key or consume model credit.
        if let Some(existing) = prior
            .iter()
            .find(|v| v.operations.iter().any(|op| op.id == operation))
        {
            if !existing
                .operations
                .iter()
                .any(|op| op.id == operation && op.kind == (OperationKind::Start { regenerate }))
            {
                return Err(AiError::Conflict);
            }
            return existing.clone().into_public_view();
        }
        if !regenerate {
            if let Some(existing) = prior.first() {
                return existing.clone().into_public_view();
            }
        }
        self.policy.generation_allowed(owner)?;
        self.key(owner).await?;
        let (snapshot, title, source_url, status, error) =
            match self.store.article_input(owner, workspace, article).await? {
                ArticleInput::Ready(snapshot) => (
                    Some(snapshot.clone()),
                    snapshot.title,
                    snapshot.source_url,
                    ChatStatus::Queued,
                    None,
                ),
                ArticleInput::Waiting { title, source_url } => {
                    (None, title, source_url, ChatStatus::WaitingContent, None)
                }
                ArticleInput::Failed => return Err(AiError::FullText),
            };
        let now = Utc::now();
        let assistant_id = Uuid::new_v4();
        let record = ChatRecord {
            owner,
            view: ArticleChat {
                id: Uuid::new_v4(),
                article_id: article,
                workspace_id: workspace,
                title,
                source_url,
                created_at: now,
                model: self.policy.config().model.clone(),
                prompt_version: self.policy.config().prompt_version.clone(),
                status,
                provider_calls: Vec::new(),
                messages: vec![ChatMessage {
                    id: assistant_id,
                    role: MessageRole::Assistant,
                    content: String::new(),
                    status: MessageStatus::Pending,
                    created_at: now,
                    phase: Some(GenerationPhase::Generating),
                    purpose: Some(MessagePurpose::Summary),
                }],
                error,
            },
            snapshot,
            system_prompt: format!("{}\n{}", self.policy.prompt(), TRANSPORT_INSTRUCTION),
            generation_mode: self.policy.config().generation_mode,
            cost_rates: self.policy.cost_rates().clone(),
            max_output_tokens: self.policy.config().max_output_tokens,
            limits: InputLimits::from(self.policy.config()),
            review: self.policy.review_snapshot()?,
            operations: vec![Operation {
                id: operation,
                kind: OperationKind::Start { regenerate },
                assistant_id,
                task: AttemptTask::Summary { draft: None },
            }],
        };
        self.store
            .create_chat(record, operation, regenerate)
            .await?
            .into_public_view()
    }
    pub async fn message(
        &self,
        owner: Uuid,
        id: Uuid,
        operation: Uuid,
        content: String,
    ) -> Result<ArticleChat, AiError> {
        if content.trim().is_empty() || content.len() > self.policy.config().max_message_bytes {
            return Err(AiError::Message);
        }
        self.enqueue(owner, id, operation, OperationKind::Message { content })
            .await
    }
    pub async fn retry(
        &self,
        owner: Uuid,
        id: Uuid,
        operation: Uuid,
    ) -> Result<ArticleChat, AiError> {
        self.enqueue(owner, id, operation, OperationKind::Retry)
            .await
    }
    async fn enqueue(
        &self,
        owner: Uuid,
        id: Uuid,
        operation: Uuid,
        kind: OperationKind,
    ) -> Result<ArticleChat, AiError> {
        let prior = self.store.chat(owner, id).await?;
        if let Some(op) = prior.operations.iter().find(|op| op.id == operation) {
            if op.kind != kind {
                return Err(AiError::Conflict);
            }
            return prior.into_public_view();
        }
        self.policy.generation_allowed(owner)?;
        self.key(owner).await?;
        self.store
            .append(owner, id, operation, kind)
            .await?
            .into_public_view()
    }
    pub async fn stop(&self, owner: Uuid, id: Uuid) -> Result<ArticleChat, AiError> {
        self.store.stop(owner, id).await?.into_public_view()
    }
}
