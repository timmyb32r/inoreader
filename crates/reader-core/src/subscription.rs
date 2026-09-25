use crate::{ActorId, Reason, ReasonPolicy, StateValidationError, SubscriptionId, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StateEvent {
    pub reason: Reason,
    pub actor: ActorId,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    Active,
    Paused(StateEvent),
    Archived,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    id: SubscriptionId,
    workspace_id: WorkspaceId,
    source_url: Url,
    title: String,
    status: SubscriptionStatus,
    revision: u64,
    history: Vec<StateEvent>,
}

impl Subscription {
    pub fn new(
        id: SubscriptionId,
        workspace_id: WorkspaceId,
        source_url: Url,
        title: String,
    ) -> Self {
        Self {
            id,
            workspace_id,
            source_url,
            title,
            status: SubscriptionStatus::Active,
            revision: 0,
            history: vec![],
        }
    }
    pub fn id(&self) -> SubscriptionId {
        self.id
    }
    pub fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub fn source_url(&self) -> &Url {
        &self.source_url
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn status(&self) -> &SubscriptionStatus {
        &self.status
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn history(&self) -> &[StateEvent] {
        &self.history
    }
    pub fn rename(&mut self, title: String) {
        if self.title != title {
            self.title = title;
            self.revision += 1;
        }
    }
    pub fn pause(&mut self, event: StateEvent) {
        if matches!(self.status, SubscriptionStatus::Paused(_)) {
            return;
        }
        self.status = SubscriptionStatus::Paused(event.clone());
        self.history.push(event);
        self.revision += 1;
    }
    pub fn resume(&mut self) {
        if matches!(self.status, SubscriptionStatus::Paused(_)) {
            self.status = SubscriptionStatus::Active;
            self.revision += 1;
        }
    }
    pub fn archive(&mut self) {
        if !matches!(self.status, SubscriptionStatus::Archived) {
            self.status = SubscriptionStatus::Archived;
            self.revision += 1;
        }
    }
    pub fn restore(&mut self) {
        if matches!(self.status, SubscriptionStatus::Archived) {
            self.status = SubscriptionStatus::Active;
            self.revision += 1;
        }
    }
    pub fn validate(&self, policy: ReasonPolicy) -> Result<(), StateValidationError> {
        for event in &self.history {
            policy.validate(event.reason.as_str().to_owned())?;
        }
        if let SubscriptionStatus::Paused(current) = &self.status {
            policy.validate(current.reason.as_str().to_owned())?;
            if self.history.last() != Some(current) {
                return Err(StateValidationError::StatusHistoryMismatch);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
