//! Explicit personal-value ratings. Ordinary read operations do not create ratings.
use crate::RepositoryError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_core::{AccountId, ArticleId, WorkspaceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A deliberately selected integer, never inferred from read state or absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct ArticleRating(u8);
impl schemars::JsonSchema for ArticleRating {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ArticleRating".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"integer","minimum":1,"maximum":10})
    }
}
impl TryFrom<u8> for ArticleRating {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (1..=10).contains(&value) {
            Ok(Self(value))
        } else {
            Err("rating must be an integer from 1 to 10")
        }
    }
}
impl From<ArticleRating> for u8 {
    fn from(value: ArticleRating) -> Self {
        value.0
    }
}

/// User-authored explanation, retained exactly (including whitespace and line
/// breaks). Absence is represented by Option::None on the command/state; no
/// explanation is inferred from a score. U+0000 cannot be stored in PostgreSQL
/// TEXT and is rejected at construction/deserialization, never stripped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RatingReason(String);
impl RatingReason {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RatingReason {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.contains('\0') {
            return Err("rating explanation cannot contain U+0000");
        }
        Ok(Self(value))
    }
}
impl From<RatingReason> for String {
    fn from(value: RatingReason) -> Self {
        value.0
    }
}
impl schemars::JsonSchema for RatingReason {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RatingReason".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string"})
    }
}

/// Exact decimal wire revision: avoids JavaScript integer precision loss. Fits PG BIGINT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct ReadingRevision(u64);
impl ReadingRevision {
    pub fn new(value: u64) -> Result<Self, &'static str> {
        if value <= i64::MAX as u64 {
            Ok(Self(value))
        } else {
            Err("article revision is outside storage range")
        }
    }
    pub fn get(self) -> u64 {
        self.0
    }
}
impl TryFrom<String> for ReadingRevision {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let number = value
            .parse::<u64>()
            .map_err(|_| "invalid article revision")?;
        if number.to_string() != value {
            return Err("revision must be an exact decimal integer");
        }
        Self::new(number)
    }
}
impl From<ReadingRevision> for String {
    fn from(value: ReadingRevision) -> Self {
        value.0.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingState {
    pub revision: ReadingRevision,
    pub read: bool,
    pub rating: Option<ArticleRating>,
    pub rated_at: Option<DateTime<Utc>>,
    pub reason: Option<RatingReason>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompleteReading {
    pub operation_id: Uuid,
    pub expected_revision: ReadingRevision,
    pub rating: ArticleRating,
    pub reason: Option<RatingReason>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingCompletion {
    pub operation_id: Uuid,
    pub article_id: Uuid,
    pub state: ReadingState,
    pub undone: bool,
}

/// Implementations recheck ownership inside each transaction, retain receipts,
/// and atomically update read/rating/event. Undo conflicts with later revisions.
#[async_trait]
pub trait FocusedReadingRepository: Send + Sync {
    async fn reading_state(
        &self,
        owner: AccountId,
        workspace: WorkspaceId,
        article: ArticleId,
    ) -> Result<ReadingState, RepositoryError>;
    async fn complete_reading(
        &self,
        owner: AccountId,
        workspace: WorkspaceId,
        article: ArticleId,
        command: CompleteReading,
    ) -> Result<ReadingCompletion, RepositoryError>;
    async fn undo_reading(
        &self,
        owner: AccountId,
        workspace: WorkspaceId,
        article: ArticleId,
        operation: Uuid,
    ) -> Result<ReadingCompletion, RepositoryError>;
}

#[cfg(test)]
#[path = "tests/focused_reading.rs"]
mod tests;
