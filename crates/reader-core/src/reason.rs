use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Reason(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReasonPolicy {
    max_bytes: usize,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ReasonError {
    #[error("reason must contain a non-whitespace character")]
    Blank,
    #[error("reason is {actual} UTF-8 bytes; maximum is {maximum}")]
    TooLong { actual: usize, maximum: usize },
    #[error("reason maximum must be greater than zero")]
    InvalidLimit,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum StateValidationError {
    #[error(transparent)]
    InvalidReason(#[from] ReasonError),
    #[error("current state event is inconsistent with aggregate history")]
    StatusHistoryMismatch,
}

impl ReasonPolicy {
    pub fn new(max_bytes: usize) -> Result<Self, ReasonError> {
        (max_bytes > 0)
            .then_some(Self { max_bytes })
            .ok_or(ReasonError::InvalidLimit)
    }
    pub const fn max_bytes(self) -> usize {
        self.max_bytes
    }
    pub fn validate(self, raw: String) -> Result<Reason, ReasonError> {
        if raw.chars().all(char::is_whitespace) {
            return Err(ReasonError::Blank);
        }
        let actual = raw.len();
        if actual > self.max_bytes {
            return Err(ReasonError::TooLong {
                actual,
                maximum: self.max_bytes,
            });
        }
        Ok(Reason(raw))
    }
}

impl Reason {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Reason {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.chars().all(char::is_whitespace) {
            return Err(serde::de::Error::custom(
                "reason must contain a non-whitespace character",
            ));
        }
        Ok(Self(raw))
    }
}

#[cfg(test)]
mod tests;
