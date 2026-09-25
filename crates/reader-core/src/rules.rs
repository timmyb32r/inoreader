use crate::{ArticleState, RuleId, SubscriptionId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RuleField {
    Title,
    Text,
    Both,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RuleAction {
    MarkRead,
    MoveToTrash,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: RuleId,
    pub subscription_id: SubscriptionId,
    pub version: u64,
    pub enabled: bool,
    pub field: RuleField,
    pub needles: Vec<String>,
    pub action: RuleAction,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuleValidationError {
    #[error("a v1 rule must contain exactly one non-blank literal phrase")]
    InvalidPhrase,
}

impl Rule {
    pub fn validate(&self) -> Result<(), RuleValidationError> {
        if self.needles.len() != 1
            || self.needles[0].trim().is_empty()
            || self.needles[0].trim() != self.needles[0]
        {
            Err(RuleValidationError::InvalidPhrase)
        } else {
            Ok(())
        }
    }
    pub fn matches(&self, title: &str, text: Option<&str>) -> bool {
        if !self.enabled || self.needles.is_empty() {
            return false;
        }
        let title = title.to_lowercase();
        let text = text.unwrap_or_default().to_lowercase();
        self.needles.iter().any(|n| {
            let n = n.to_lowercase();
            match self.field {
                RuleField::Title => title.contains(&n),
                RuleField::Text => text.contains(&n),
                RuleField::Both => title.contains(&n) || text.contains(&n),
            }
        })
    }
    pub fn apply(&self, state: &mut ArticleState) {
        match self.action {
            RuleAction::MarkRead if !state.protect_unread => state.read = true,
            RuleAction::MoveToTrash if !state.protect_restored => state.trashed = true,
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PendingRuleEvaluation {
    pub rule_id: RuleId,
    pub rule_version: u64,
    pub subscription_id: SubscriptionId,
}

impl PendingRuleEvaluation {
    pub fn still_valid_for(&self, rule: &Rule) -> bool {
        rule.enabled
            && self.rule_id == rule.id
            && self.rule_version == rule.version
            && self.subscription_id == rule.subscription_id
    }
}

#[cfg(test)]
mod tests;
