use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JobKind {
    PollSource,
    DeliverRecord,
    ExtractFullText,
    EvaluateRule,
    ApplyRuleToArchive,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Ready,
    Leased(JobLease),
    RetryAt(DateTime<Utc>),
    Completed,
    Failed { diagnostic: String },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct JobLease {
    pub owner: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableJob {
    pub id: Uuid,
    pub kind: JobKind,
    pub idempotency_key: String,
    pub payload: Vec<u8>,
    pub status: JobStatus,
    pub attempts: u32,
    pub revision: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub id: Uuid,
    pub aggregate_id: Uuid,
    pub aggregate_revision: u64,
    pub kind: String,
    pub payload: Vec<u8>,
    pub published_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentManifest {
    pub logical_id: Uuid,
    pub current_revision: u64,
    pub blob_ids: Vec<String>,
    pub committed_at: DateTime<Utc>,
}

impl DurableJob {
    pub fn lease(&mut self, lease: JobLease) -> bool {
        if !matches!(self.status, JobStatus::Ready | JobStatus::RetryAt(_)) {
            return false;
        }
        self.status = JobStatus::Leased(lease);
        self.revision += 1;
        true
    }
    pub fn complete(&mut self, owner: &str) -> bool {
        if matches!(&self.status,JobStatus::Leased(v) if v.owner==owner) {
            self.status = JobStatus::Completed;
            self.revision += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests;
