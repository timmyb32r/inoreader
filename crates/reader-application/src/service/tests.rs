use super::*;
use crate::{
    AccountRecord, ArticlePresentation, InviteRecord, PasswordResetRecord, RuleApplicationProgress,
    SeedSource, SessionRecord,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use url::Url;
use uuid::Uuid;

struct Repository {
    workspace: Workspace,
    subscription: Subscription,
    saved_workspace: Mutex<Vec<(Option<u64>, Workspace)>>,
    saved_subscription: Mutex<Vec<(Option<u64>, Subscription)>>,
    refreshes: Mutex<Vec<SubscriptionId>>,
    created_account_workspace: Mutex<Vec<(AccountRecord, Workspace)>>,
}

impl Repository {
    fn new(workspace: Workspace, subscription: Subscription) -> Self {
        Self {
            workspace,
            subscription,
            saved_workspace: Mutex::new(vec![]),
            saved_subscription: Mutex::new(vec![]),
            refreshes: Mutex::new(vec![]),
            created_account_workspace: Mutex::new(vec![]),
        }
    }
}

fn unused<T>() -> Result<T, RepositoryError> {
    Err(RepositoryError::NotFound)
}

#[tokio::test]
async fn bootstrap_admin_creates_an_owned_initial_workspace() {
    let owner = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "fixture".into());
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    let repository = Arc::new(Repository::new(workspace, subscription));
    let service = crate::AuthService::new(
        repository.clone(),
        crate::AuthPolicy {
            session_lifetime_seconds: 60,
            invite_lifetime_seconds: 60,
            reset_lifetime_seconds: 60,
            argon2id: crate::Argon2idPolicy::new(8, 1, 1).unwrap(),
        },
    );

    let account = service
        .bootstrap_admin("admin".into(), "correct horse battery staple")
        .await
        .unwrap();

    let writes = repository.created_account_workspace.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0.id, account.id);
    assert_eq!(writes[0].1.owner(), account.id);
    assert_eq!(writes[0].1.name(), "Personal");
}

#[async_trait]
impl ReaderRepository for Repository {
    async fn readiness(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
    async fn save_job(&self, _: Option<u64>, _: DurableJob) -> Result<(), RepositoryError> {
        unused()
    }
    async fn account(&self, _: AccountId) -> Result<AccountRecord, RepositoryError> {
        unused()
    }
    async fn account_by_username(&self, _: &str) -> Result<AccountRecord, RepositoryError> {
        unused()
    }
    async fn save_account(&self, _: Option<u64>, _: AccountRecord) -> Result<(), RepositoryError> {
        unused()
    }
    async fn create_account_and_workspace(
        &self,
        account: AccountRecord,
        workspace: Workspace,
    ) -> Result<(), RepositoryError> {
        self.created_account_workspace
            .lock()
            .unwrap()
            .push((account, workspace));
        Ok(())
    }
    async fn invite_by_token_hash(&self, _: &str) -> Result<InviteRecord, RepositoryError> {
        unused()
    }
    async fn save_invite(&self, _: Option<u64>, _: InviteRecord) -> Result<(), RepositoryError> {
        unused()
    }
    async fn session_by_verifier_hash(&self, _: &str) -> Result<SessionRecord, RepositoryError> {
        unused()
    }
    async fn save_session(&self, _: Option<u64>, _: SessionRecord) -> Result<(), RepositoryError> {
        unused()
    }
    async fn delete_session(&self, _: &str, _: u64) -> Result<(), RepositoryError> {
        unused()
    }
    async fn password_reset_by_token_hash(
        &self,
        _: &str,
    ) -> Result<PasswordResetRecord, RepositoryError> {
        unused()
    }
    async fn save_password_reset(
        &self,
        _: Option<u64>,
        _: PasswordResetRecord,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn consume_invite_create_account_and_workspace(
        &self,
        _: u64,
        _: InviteRecord,
        _: AccountRecord,
        _: Workspace,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn consume_reset_and_update_account(
        &self,
        _: u64,
        _: PasswordResetRecord,
        _: u64,
        _: AccountRecord,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn workspace(&self, id: WorkspaceId) -> Result<Workspace, RepositoryError> {
        if id == self.workspace.id() {
            Ok(self.workspace.clone())
        } else {
            unused()
        }
    }
    async fn workspaces_by_owner(&self, _: AccountId) -> Result<Vec<Workspace>, RepositoryError> {
        unused()
    }
    async fn save_workspace(
        &self,
        expected: Option<u64>,
        value: Workspace,
    ) -> Result<(), RepositoryError> {
        self.saved_workspace.lock().unwrap().push((expected, value));
        Ok(())
    }
    async fn restore_workspace_with_refreshes(
        &self,
        expected: u64,
        value: Workspace,
        active: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        self.saved_workspace
            .lock()
            .unwrap()
            .push((Some(expected), value));
        self.refreshes
            .lock()
            .unwrap()
            .extend(active.into_iter().map(|subscription| subscription.id()));
        Ok(())
    }
    async fn subscription(&self, id: SubscriptionId) -> Result<Subscription, RepositoryError> {
        if id == self.subscription.id() {
            Ok(self.subscription.clone())
        } else {
            unused()
        }
    }
    async fn subscriptions_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<Subscription>, RepositoryError> {
        if workspace == self.workspace.id() {
            Ok(vec![self.subscription.clone()])
        } else {
            unused()
        }
    }
    async fn subscription_stats(
        &self,
        _: WorkspaceId,
        values: &[Subscription],
    ) -> Result<std::collections::HashMap<SubscriptionId, crate::SubscriptionStats>, RepositoryError>
    {
        Ok(values
            .iter()
            .map(|v| (v.id(), crate::SubscriptionStats::default()))
            .collect())
    }
    async fn save_subscription(
        &self,
        expected: Option<u64>,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        self.saved_subscription
            .lock()
            .unwrap()
            .push((expected, value));
        Ok(())
    }
    async fn activate_subscription_with_refresh(
        &self,
        expected: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        self.saved_subscription
            .lock()
            .unwrap()
            .push((Some(expected), value.clone()));
        self.refreshes.lock().unwrap().push(value.id());
        Ok(())
    }
    async fn enqueue_subscription_refresh(
        &self,
        value: &Subscription,
    ) -> Result<(), RepositoryError> {
        self.refreshes.lock().unwrap().push(value.id());
        Ok(())
    }
    async fn save_web_feed_subscription(
        &self,
        _: Subscription,
        _: String,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn web_feed_recipe(&self, _: SubscriptionId) -> Result<(u64, String), RepositoryError> {
        unused()
    }
    async fn update_web_feed_recipe(
        &self,
        _: SubscriptionId,
        _: u64,
        _: String,
    ) -> Result<u64, RepositoryError> {
        unused()
    }
    async fn article(&self, _: WorkspaceId, _: ArticleId) -> Result<Article, RepositoryError> {
        unused()
    }
    async fn articles_by_workspace(&self, _: WorkspaceId) -> Result<Vec<Article>, RepositoryError> {
        unused()
    }
    async fn article_presentations_by_workspace(
        &self,
        _: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        unused()
    }
    async fn save_article(
        &self,
        _: WorkspaceId,
        _: Option<u64>,
        _: Article,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn enqueue_article_full_text_refresh(
        &self,
        _: WorkspaceId,
        _: &Article,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn rules_by_workspace(&self, _: WorkspaceId) -> Result<Vec<Rule>, RepositoryError> {
        unused()
    }
    async fn rule(&self, _: WorkspaceId, _: RuleId) -> Result<Rule, RepositoryError> {
        unused()
    }
    async fn save_rule(
        &self,
        _: WorkspaceId,
        _: Option<u64>,
        _: Rule,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn delete_rule(&self, _: WorkspaceId, _: RuleId, _: u64) -> Result<(), RepositoryError> {
        unused()
    }
    async fn enqueue_rule_application(
        &self,
        _: WorkspaceId,
        _: Rule,
    ) -> Result<Uuid, RepositoryError> {
        unused()
    }
    async fn rule_application_progress(
        &self,
        _: WorkspaceId,
        _: Uuid,
    ) -> Result<RuleApplicationProgress, RepositoryError> {
        unused()
    }
    async fn record_login_attempt(
        &self,
        _: &str,
        _: DateTime<Utc>,
        _: u32,
    ) -> Result<bool, RepositoryError> {
        unused()
    }
    async fn mark_articles_read_atomic(
        &self,
        _: WorkspaceId,
        _: Vec<Article>,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn import_subscriptions_atomic(
        &self,
        _: WorkspaceId,
        _: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        unused()
    }
    async fn apply_seed_atomic(
        &self,
        _: WorkspaceId,
        _: Vec<(String, Subscription, SeedSource, serde_json::Value)>,
    ) -> Result<(), RepositoryError> {
        unused()
    }
}

fn fixture() -> (Arc<Repository>, ReaderService<Repository>) {
    let owner = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "Reading".into());
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        Url::parse("https://example.test/feed").unwrap(),
        "Feed".into(),
    );
    let repository = Arc::new(Repository::new(workspace, subscription));
    let service = ReaderService::new(repository.clone(), ReasonPolicy::new(32).unwrap());
    (repository, service)
}

#[tokio::test]
async fn invalid_pause_reason_fails_before_any_repository_write() {
    let (repository, service) = fixture();
    let result = service
        .pause_subscription(
            repository.subscription.id(),
            "   ".into(),
            ActorId::new(),
            Utc::now(),
        )
        .await;
    assert!(matches!(result, Err(CommandError::InvalidReason(_))));
    assert!(repository.saved_subscription.lock().unwrap().is_empty());
}

#[tokio::test]
async fn pause_uses_the_loaded_revision_as_the_cas_precondition() {
    let (repository, service) = fixture();
    service
        .pause_subscription(
            repository.subscription.id(),
            "Editorial review".into(),
            ActorId::new(),
            Utc::now(),
        )
        .await
        .unwrap();
    let saved = repository.saved_subscription.lock().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].0, Some(0));
    assert_eq!(saved[0].1.revision(), 1);
    assert_eq!(saved[0].1.history()[0].reason.as_str(), "Editorial review");
}

#[tokio::test]
async fn archived_workspace_blocks_resume_without_mutating_subscription() {
    let (owner_workspace, _service) = fixture();
    let mut archived = owner_workspace.workspace.clone();
    archived.archive(WorkspaceStateEvent {
        reason: ReasonPolicy::new(32)
            .unwrap()
            .validate("Maintenance".into())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc::now(),
    });
    let subscription = owner_workspace.subscription.clone();
    let repository = Arc::new(Repository::new(archived, subscription));
    let service = ReaderService::new(repository.clone(), ReasonPolicy::new(32).unwrap());
    assert!(matches!(
        service
            .resume_subscription(repository.subscription.id())
            .await,
        Err(CommandError::WorkspaceArchived)
    ));
    assert!(repository.saved_subscription.lock().unwrap().is_empty());
    drop(service);
}

#[tokio::test]
async fn archive_and_restore_preserve_expected_revision_sequence() {
    let (repository, service) = fixture();
    service
        .archive_workspace(
            repository.workspace.id(),
            "Maintenance".into(),
            ActorId::new(),
            Utc::now(),
        )
        .await
        .unwrap();
    {
        let saved = repository.saved_workspace.lock().unwrap();
        assert_eq!(saved[0].0, Some(0));
        assert_eq!(saved[0].1.revision(), 1);
    }

    let archived = repository.saved_workspace.lock().unwrap()[0].1.clone();
    let replacement = Arc::new(Repository::new(archived, repository.subscription.clone()));
    ReaderService::new(replacement.clone(), ReasonPolicy::new(32).unwrap())
        .restore_workspace(replacement.workspace.id())
        .await
        .unwrap();
    let restored = replacement.saved_workspace.lock().unwrap();
    assert_eq!(restored[0].0, Some(1));
    assert_eq!(restored[0].1.revision(), 2);
    assert!(restored[0].1.accepts_delivery());
    assert_eq!(restored[0].1.history().len(), 1);
}

#[tokio::test]
async fn archive_uses_current_revision_after_a_prior_rename() {
    let (repository, _service) = fixture();
    let mut renamed = repository.workspace.clone();
    renamed.rename("Renamed".into());
    let replacement = Arc::new(Repository::new(renamed, repository.subscription.clone()));
    ReaderService::new(replacement.clone(), ReasonPolicy::new(32).unwrap())
        .archive_workspace(
            replacement.workspace.id(),
            "Maintenance".into(),
            ActorId::new(),
            Utc::now(),
        )
        .await
        .unwrap();
    let saved = replacement.saved_workspace.lock().unwrap();
    assert_eq!(saved[0].0, Some(1));
    assert_eq!(saved[0].1.revision(), 2);
}

#[tokio::test]
async fn unsubscribe_preserves_the_subscription_and_restore_queues_catch_up() {
    let (repository, service) = fixture();
    let archived = service
        .archive_subscription(repository.subscription.id())
        .await
        .unwrap();
    assert_eq!(archived.status(), &SubscriptionStatus::Archived);
    assert!(repository.refreshes.lock().unwrap().is_empty());

    let replacement = Arc::new(Repository::new(repository.workspace.clone(), archived));
    let restored = ReaderService::new(replacement.clone(), ReasonPolicy::new(32).unwrap())
        .restore_subscription(replacement.subscription.id())
        .await
        .unwrap();
    assert_eq!(restored.status(), &SubscriptionStatus::Active);
    assert_eq!(
        replacement.refreshes.lock().unwrap().as_slice(),
        &[restored.id()]
    );
    assert_eq!(replacement.saved_subscription.lock().unwrap()[0].0, Some(1));
}

#[tokio::test]
async fn workspace_restore_does_not_resume_an_individually_paused_subscription() {
    let (base, _) = fixture();
    let mut workspace = base.workspace.clone();
    workspace.archive(WorkspaceStateEvent {
        reason: ReasonPolicy::new(32)
            .unwrap()
            .validate("Maintenance".into())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc::now(),
    });
    let mut subscription = base.subscription.clone();
    subscription.pause(StateEvent {
        reason: ReasonPolicy::new(32)
            .unwrap()
            .validate("Personal pause".into())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc::now(),
    });
    let repository = Arc::new(Repository::new(workspace, subscription));
    ReaderService::new(repository.clone(), ReasonPolicy::new(32).unwrap())
        .restore_workspace(repository.workspace.id())
        .await
        .unwrap();
    assert!(repository.refreshes.lock().unwrap().is_empty());
    assert!(repository.saved_subscription.lock().unwrap().is_empty());
}
