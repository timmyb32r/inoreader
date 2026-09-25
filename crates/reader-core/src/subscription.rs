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
    #[serde(default)]
    source_url_exact: Option<String>,
    #[serde(rename = "title")]
    source_title: String,
    #[serde(default)]
    custom_name: Option<String>,
    #[serde(default)]
    personal_note: String,
    #[serde(default)]
    created_at: Option<DateTime<Utc>>,
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
            source_url_exact: None,
            source_title: title,
            custom_name: None,
            personal_note: String::new(),
            created_at: Some(Utc::now()),
            status: SubscriptionStatus::Active,
            revision: 0,
            history: vec![],
        }
    }
    pub fn new_with_exact_url(
        id: SubscriptionId,
        workspace_id: WorkspaceId,
        source_url: Url,
        source_url_exact: String,
        title: String,
    ) -> Result<Self, StateValidationError> {
        if Url::parse(&source_url_exact).ok().as_ref() != Some(&source_url) {
            return Err(StateValidationError::SourceUrlMismatch);
        }
        let mut value = Self::new(id, workspace_id, source_url, title);
        value.source_url_exact = Some(source_url_exact);
        Ok(value)
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
    pub fn source_url_exact(&self) -> &str {
        self.source_url_exact
            .as_deref()
            .unwrap_or_else(|| self.source_url.as_str())
    }
    pub fn title(&self) -> &str {
        self.custom_name.as_deref().unwrap_or(&self.source_title)
    }
    pub fn source_title(&self) -> &str {
        &self.source_title
    }
    pub fn custom_name(&self) -> Option<&str> {
        self.custom_name.as_deref()
    }
    pub fn personal_note(&self) -> &str {
        &self.personal_note
    }
    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        self.created_at
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
        let custom_name = (!title.is_empty()).then_some(title);
        if self.custom_name != custom_name {
            self.custom_name = custom_name;
            self.revision += 1;
        }
    }
    pub fn set_personal_note(&mut self, note: String) {
        if self.personal_note != note {
            self.personal_note = note;
            self.revision += 1;
        }
    }
    pub fn replace_source(
        &mut self,
        source_url: Url,
        exact_source_url: String,
        source_title: String,
    ) -> Result<(), StateValidationError> {
        if Url::parse(&exact_source_url).ok().as_ref() != Some(&source_url) {
            return Err(StateValidationError::SourceUrlMismatch);
        }
        if self.source_url != source_url
            || self.source_url_exact() != exact_source_url
            || self.source_title != source_title
        {
            self.source_url = source_url;
            self.source_url_exact = Some(exact_source_url);
            self.source_title = source_title;
            self.revision += 1;
        }
        Ok(())
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
        if self
            .source_url_exact
            .as_deref()
            .is_some_and(|exact| Url::parse(exact).ok().as_ref() != Some(&self.source_url))
        {
            return Err(StateValidationError::SourceUrlMismatch);
        }
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
