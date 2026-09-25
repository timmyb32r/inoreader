use crate::{ReaderRepository, RepositoryError};
use chrono::{DateTime, Utc};
use reader_core::*;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    InvalidReason(#[from] ReasonError),
    #[error("workspace is archived")]
    WorkspaceArchived,
}

pub struct ReaderService<R> {
    repository: Arc<R>,
    reason_policy: ReasonPolicy,
}

impl<R: ReaderRepository> ReaderService<R> {
    pub fn new(repository: Arc<R>, reason_policy: ReasonPolicy) -> Self {
        Self {
            repository,
            reason_policy,
        }
    }
    pub async fn pause_subscription(
        &self,
        id: SubscriptionId,
        raw_reason: String,
        actor: ActorId,
        at: DateTime<Utc>,
    ) -> Result<Subscription, CommandError> {
        let reason = self.reason_policy.validate(raw_reason)?;
        let mut value = self.repository.subscription(id).await?;
        let revision = value.revision();
        value.pause(StateEvent { reason, actor, at });
        self.repository
            .save_subscription(Some(revision), value.clone())
            .await?;
        Ok(value)
    }
    pub async fn archive_workspace(
        &self,
        id: WorkspaceId,
        raw_reason: String,
        actor: ActorId,
        at: DateTime<Utc>,
    ) -> Result<Workspace, CommandError> {
        let reason = self.reason_policy.validate(raw_reason)?;
        let mut value = self.repository.workspace(id).await?;
        let revision = match value.status() {
            WorkspaceStatus::Active => value.revision(),
            WorkspaceStatus::Archived(_) => return Ok(value),
        };
        value.archive(WorkspaceStateEvent { reason, actor, at });
        self.repository
            .save_workspace(Some(revision), value.clone())
            .await?;
        Ok(value)
    }
    pub async fn restore_workspace(&self, id: WorkspaceId) -> Result<Workspace, CommandError> {
        let mut value = self.repository.workspace(id).await?;
        let revision = value.revision();
        value.restore();
        let active = self
            .repository
            .subscriptions_by_workspace(id)
            .await?
            .into_iter()
            .filter(|subscription| matches!(subscription.status(), SubscriptionStatus::Active))
            .collect();
        self.repository
            .restore_workspace_with_refreshes(revision, value.clone(), active)
            .await?;
        Ok(value)
    }
    pub async fn resume_subscription(
        &self,
        id: SubscriptionId,
    ) -> Result<Subscription, CommandError> {
        let mut value = self.repository.subscription(id).await?;
        let workspace = self.repository.workspace(value.workspace_id()).await?;
        if !workspace.accepts_delivery() {
            return Err(CommandError::WorkspaceArchived);
        }
        let revision = value.revision();
        value.resume();
        self.repository
            .activate_subscription_with_refresh(revision, value.clone())
            .await?;
        Ok(value)
    }
    pub async fn archive_subscription(
        &self,
        id: SubscriptionId,
    ) -> Result<Subscription, CommandError> {
        let mut value = self.repository.subscription(id).await?;
        let revision = value.revision();
        value.archive();
        self.repository
            .save_subscription(Some(revision), value.clone())
            .await?;
        Ok(value)
    }
    pub async fn restore_subscription(
        &self,
        id: SubscriptionId,
    ) -> Result<Subscription, CommandError> {
        let mut value = self.repository.subscription(id).await?;
        let workspace = self.repository.workspace(value.workspace_id()).await?;
        if !workspace.accepts_delivery() {
            return Err(CommandError::WorkspaceArchived);
        }
        let revision = value.revision();
        value.restore();
        self.repository
            .activate_subscription_with_refresh(revision, value.clone())
            .await?;
        Ok(value)
    }
    pub async fn list_articles(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Article>, CommandError> {
        Ok(self.repository.articles_by_workspace(workspace_id).await?)
    }
    pub async fn mark_read(
        &self,
        workspace_id: WorkspaceId,
        article_id: ArticleId,
        read: bool,
    ) -> Result<Article, CommandError> {
        if !self
            .repository
            .workspace(workspace_id)
            .await?
            .accepts_delivery()
        {
            return Err(CommandError::WorkspaceArchived);
        }
        let mut article = self.repository.article(workspace_id, article_id).await?;
        let expected = article.revision;
        article.state.read = read;
        if !read {
            article.state.protect_unread = true;
        }
        article.revision += 1;
        self.repository
            .save_article(workspace_id, Some(expected), article.clone())
            .await?;
        Ok(article)
    }
}

#[cfg(test)]
mod tests;
