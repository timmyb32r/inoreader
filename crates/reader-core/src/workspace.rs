use crate::{AccountId, ActorId, Reason, ReasonPolicy, StateValidationError, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceStateEvent {
    pub reason: Reason,
    pub actor: ActorId,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceStatus {
    Active,
    Archived(WorkspaceStateEvent),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    id: WorkspaceId,
    owner: AccountId,
    name: String,
    status: WorkspaceStatus,
    revision: u64,
    history: Vec<WorkspaceStateEvent>,
}

impl Workspace {
    pub fn new(id: WorkspaceId, owner: AccountId, name: String) -> Self {
        Self {
            id,
            owner,
            name,
            status: WorkspaceStatus::Active,
            revision: 0,
            history: vec![],
        }
    }
    pub fn id(&self) -> WorkspaceId {
        self.id
    }
    pub fn owner(&self) -> AccountId {
        self.owner
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn status(&self) -> &WorkspaceStatus {
        &self.status
    }
    pub fn accepts_delivery(&self) -> bool {
        matches!(self.status, WorkspaceStatus::Active)
    }
    pub fn archive(&mut self, event: WorkspaceStateEvent) {
        if self.accepts_delivery() {
            self.status = WorkspaceStatus::Archived(event.clone());
            self.history.push(event);
            self.revision += 1;
        }
    }
    pub fn restore(&mut self) {
        if !self.accepts_delivery() {
            self.status = WorkspaceStatus::Active;
            self.revision += 1;
        }
    }
    pub fn rename(&mut self, name: String) {
        if self.name != name {
            self.name = name;
            self.revision += 1;
        }
    }
    pub fn history(&self) -> &[WorkspaceStateEvent] {
        &self.history
    }
    pub fn validate(&self, policy: ReasonPolicy) -> Result<(), StateValidationError> {
        for event in &self.history {
            policy.validate(event.reason.as_str().to_owned())?;
        }
        if let WorkspaceStatus::Archived(current) = &self.status {
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
