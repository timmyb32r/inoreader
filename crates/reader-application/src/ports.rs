use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_core::{
    AccountId, Article, ArticleId, DurableJob, Rule, RuleId, Subscription, SubscriptionId,
    Workspace, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("record not found")]
    NotFound,
    #[error("concurrent revision conflict")]
    Conflict,
    #[error("storage failure: {0}")]
    Storage(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountRecord {
    pub id: AccountId,
    pub username: String,
    pub password_hash: String,
    pub admin: bool,
    pub auth_revision: u64,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InviteRecord {
    pub id: Uuid,
    pub token_hash: String,
    pub username: String,
    pub created_by: AccountId,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: Uuid,
    pub verifier_hash: String,
    pub account_id: AccountId,
    pub account_auth_revision: u64,
    pub expires_at: DateTime<Utc>,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PasswordResetRecord {
    pub id: Uuid,
    pub token_hash: String,
    pub account_id: AccountId,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub revision: u64,
}
#[derive(Clone, Debug)]
pub struct ArticlePresentation {
    pub article: Article,
    pub subscription_ids: Vec<SubscriptionId>,
    pub subscription_titles: Vec<String>,
    pub safe_html: Option<String>,
    pub full_text_status: &'static str,
    pub failure_reason: Option<String>,
}
#[derive(Clone, Debug)]
pub enum SeedSource {
    Feed,
    Imported,
}
#[derive(Clone, Debug)]
pub struct RuleApplicationProgress {
    pub operation_id: Uuid,
    pub workspace_id: WorkspaceId,
    pub rule_id: RuleId,
    pub rule_version: u64,
    pub status: String,
    pub evaluated: usize,
    pub cancel_reason: Option<String>,
}
#[derive(Clone, Debug)]
pub struct SubscriptionStats {
    pub article_count: usize,

    pub unread_count: usize,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub incomplete: bool,
    pub continuation: Option<String>,
    pub error: Option<String>,
    pub editable_web_feed: bool,

    pub source_type: String,
}
impl Default for SubscriptionStats {
    fn default() -> Self {
        Self {
            article_count: 0,
            unread_count: 0,
            last_success_at: None,
            last_error_at: None,
            consecutive_failures: 0,
            incomplete: false,
            continuation: None,
            error: None,
            editable_web_feed: false,
            source_type: "feed".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionActivity {
    pub id: Uuid,

    pub subscription_id: SubscriptionId,

    pub occurred_at: DateTime<Utc>,

    pub successful: bool,

    pub duration_ms: Option<u64>,

    pub discovered_items: Option<usize>,

    pub diagnostic: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceUrlPreviewRecord {
    pub id: Uuid,

    pub subscription_id: SubscriptionId,

    pub url: String,

    pub source_title: String,

    pub expires_at: DateTime<Utc>,

    pub revision: u64,
}

#[async_trait]
pub trait ReaderRepository: Send + Sync {
    async fn readiness(&self) -> Result<(), RepositoryError>;
    async fn save_job(
        &self,
        expected_revision: Option<u64>,
        value: DurableJob,
    ) -> Result<(), RepositoryError>;
    async fn account(&self, id: AccountId) -> Result<AccountRecord, RepositoryError>;
    async fn account_by_username(&self, username: &str) -> Result<AccountRecord, RepositoryError>;
    async fn save_account(
        &self,
        expected_revision: Option<u64>,
        value: AccountRecord,
    ) -> Result<(), RepositoryError>;
    /// Atomically creates the first administrator and their initial workspace.
    async fn create_account_and_workspace(
        &self,
        account: AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError>;
    async fn invite_by_token_hash(&self, token_hash: &str)
        -> Result<InviteRecord, RepositoryError>;
    async fn save_invite(
        &self,
        expected_revision: Option<u64>,
        value: InviteRecord,
    ) -> Result<(), RepositoryError>;
    async fn session_by_verifier_hash(
        &self,
        verifier_hash: &str,
    ) -> Result<SessionRecord, RepositoryError>;
    async fn save_session(
        &self,
        expected_revision: Option<u64>,
        value: SessionRecord,
    ) -> Result<(), RepositoryError>;
    async fn delete_session(
        &self,
        verifier_hash: &str,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    async fn password_reset_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<PasswordResetRecord, RepositoryError>;
    async fn save_password_reset(
        &self,
        expected_revision: Option<u64>,
        value: PasswordResetRecord,
    ) -> Result<(), RepositoryError>;
    async fn consume_invite_create_account_and_workspace(
        &self,
        expected_invite_revision: u64,
        invite: InviteRecord,
        account: AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError>;
    async fn consume_reset_and_update_account(
        &self,
        expected_reset_revision: u64,
        reset: PasswordResetRecord,
        expected_account_revision: u64,
        account: AccountRecord,
    ) -> Result<(), RepositoryError>;
    async fn workspace(&self, id: WorkspaceId) -> Result<Workspace, RepositoryError>;
    async fn workspaces_by_owner(
        &self,
        owner: AccountId,
    ) -> Result<Vec<Workspace>, RepositoryError>;
    async fn save_workspace(
        &self,
        expected_revision: Option<u64>,
        value: Workspace,
    ) -> Result<(), RepositoryError>;
    async fn restore_workspace_with_refreshes(
        &self,
        expected_revision: u64,
        value: Workspace,
        active_subscriptions: Vec<Subscription>,
    ) -> Result<(), RepositoryError>;
    async fn subscription(&self, id: SubscriptionId) -> Result<Subscription, RepositoryError>;
    async fn subscriptions_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Subscription>, RepositoryError>;
    async fn subscription_stats(
        &self,
        workspace: WorkspaceId,
        subscriptions: &[Subscription],
    ) -> Result<std::collections::HashMap<SubscriptionId, SubscriptionStats>, RepositoryError>;
    async fn subscription_activity(
        &self,
        owner: AccountId,
        subscription: SubscriptionId,
        since: DateTime<Utc>,
    ) -> Result<Vec<SubscriptionActivity>, RepositoryError> {
        let _ = (owner, subscription, since);
        Err(RepositoryError::Storage(
            "subscription activity is unavailable".into(),
        ))
    }
    async fn save_subscription(
        &self,
        expected_revision: Option<u64>,
        value: Subscription,
    ) -> Result<(), RepositoryError>;
    async fn replace_subscription_source(
        &self,
        expected_revision: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        let _ = (expected_revision, value);
        Err(RepositoryError::Storage(
            "source URL replacement is unavailable".into(),
        ))
    }
    async fn source_url_preview(
        &self,
        id: Uuid,
    ) -> Result<SourceUrlPreviewRecord, RepositoryError> {
        let _ = id;
        Err(RepositoryError::NotFound)
    }
    async fn save_source_url_preview(
        &self,
        value: SourceUrlPreviewRecord,
    ) -> Result<(), RepositoryError> {
        let _ = value;
        Err(RepositoryError::Storage(
            "source URL preview storage is unavailable".into(),
        ))
    }
    /// Atomically publishes an active subscription revision and its durable catch-up work.
    async fn activate_subscription_with_refresh(
        &self,
        expected_revision: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError>;
    async fn enqueue_subscription_refresh(
        &self,
        value: &Subscription,
    ) -> Result<(), RepositoryError>;
    async fn save_web_feed_subscription(
        &self,
        value: Subscription,
        recipe_json: String,
    ) -> Result<(), RepositoryError>;
    async fn web_feed_recipe(
        &self,
        subscription: SubscriptionId,
    ) -> Result<(u64, String), RepositoryError>;
    async fn update_web_feed_recipe(
        &self,
        subscription: SubscriptionId,
        expected_version: u64,
        recipe_json: String,
    ) -> Result<u64, RepositoryError>;
    async fn article(
        &self,
        workspace: WorkspaceId,
        id: ArticleId,
    ) -> Result<Article, RepositoryError>;
    async fn articles_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Article>, RepositoryError>;
    async fn article_presentations_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError>;
    async fn save_article(
        &self,
        workspace: WorkspaceId,
        expected_revision: Option<u64>,
        value: Article,
    ) -> Result<(), RepositoryError>;
    async fn enqueue_article_full_text_refresh(
        &self,
        workspace: WorkspaceId,
        value: &Article,
    ) -> Result<(), RepositoryError>;
    async fn rules_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Rule>, RepositoryError>;
    async fn rule(&self, workspace: WorkspaceId, id: RuleId) -> Result<Rule, RepositoryError>;
    async fn save_rule(
        &self,
        workspace: WorkspaceId,
        expected_revision: Option<u64>,
        value: Rule,
    ) -> Result<(), RepositoryError>;
    async fn delete_rule(
        &self,
        workspace: WorkspaceId,
        id: RuleId,
        expected_revision: u64,
    ) -> Result<(), RepositoryError>;
    async fn enqueue_rule_application(
        &self,
        workspace: WorkspaceId,
        rule: Rule,
    ) -> Result<Uuid, RepositoryError>;
    async fn rule_application_progress(
        &self,
        workspace: WorkspaceId,
        operation: Uuid,
    ) -> Result<RuleApplicationProgress, RepositoryError>;
    async fn record_login_attempt(
        &self,
        username: &str,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<bool, RepositoryError>;
    async fn mark_articles_read_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Article>,
    ) -> Result<(), RepositoryError>;
    async fn import_subscriptions_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Subscription>,
    ) -> Result<(), RepositoryError>;
    async fn apply_seed_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<(String, Subscription, SeedSource, serde_json::Value)>,
    ) -> Result<(), RepositoryError>;
}
