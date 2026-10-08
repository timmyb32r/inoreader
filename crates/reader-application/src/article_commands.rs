//! Account ownership is resolved before an article read or write. Workspace owner
//! is immutable; the scope cannot be constructed from a caller-supplied UUID alone.
use crate::{ArticleRepository, RepositoryError, WorkspaceRepository};
use reader_core::{AccountId, Article, ArticleId, WorkspaceId};

pub struct OwnedWorkspace {
    id: WorkspaceId,
}
impl OwnedWorkspace {
    pub async fn resolve<R: WorkspaceRepository>(
        repository: &R,
        owner: AccountId,
        id: WorkspaceId,
    ) -> Result<Self, RepositoryError> {
        if repository.workspace(id).await?.owner() != owner {
            return Err(RepositoryError::NotFound);
        }
        Ok(Self { id })
    }
    pub fn id(&self) -> WorkspaceId {
        self.id
    }
}

/// Patch semantics are shared by HTTP and other application callers. A missing
/// field is unchanged; explicitly marking unread protects it from automatic rules.
#[derive(Default)]
pub struct ArticlePatch {
    pub read: Option<bool>,
    pub later: Option<bool>,
}
impl ArticlePatch {
    fn apply(self, value: &mut Article) {
        if let Some(read) = self.read {
            value.state.read = read;
            if !read {
                value.state.protect_unread = true;
            }
        }
        if let Some(later) = self.later {
            value.state.later = later;
        }
    }
}
pub async fn update_article<R: ArticleRepository>(
    repository: &R,
    scope: &OwnedWorkspace,
    id: ArticleId,
    patch: ArticlePatch,
) -> Result<(), RepositoryError> {
    let mut article = repository.article(scope.id, id).await?;
    let revision = article.revision;
    article.revision = revision
        .checked_add(1)
        .ok_or_else(|| RepositoryError::Storage("article revision exhausted".into()))?;
    patch.apply(&mut article);
    repository
        .save_article(scope.id, Some(revision), article)
        .await
}

/// One atomic owned snapshot, streamed in configured batches. New arrivals after
/// the transaction snapshot belong to the next operation. Any failed batch rolls
/// back every write and read event; the batch size never caps the total selection.
pub async fn mark_articles_read<R: ArticleRepository + crate::SubscriptionRepository>(
    repository: &R,
    workspace: &OwnedWorkspace,
    scope: crate::ArticleScope,
    batch: crate::SelectionLimit,
) -> Result<(), RepositoryError> {
    if let Some(id) = scope.subscription() {
        if repository.subscription(id).await?.workspace_id() != workspace.id {
            return Err(RepositoryError::NotFound);
        }
    }
    repository
        .mark_scope_read_atomic(workspace.id, scope, batch)
        .await
}
