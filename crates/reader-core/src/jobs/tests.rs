use super::*;
use chrono::{Duration, Utc};
use uuid::Uuid;

#[test]
fn only_lease_owner_can_complete() {
    let now = Utc::now();
    let mut job = DurableJob {
        id: Uuid::new_v4(),
        kind: JobKind::PollSource,
        idempotency_key: "x".into(),
        payload: vec![],
        status: JobStatus::Ready,
        attempts: 0,
        revision: 0,
    };
    assert!(job.lease(JobLease {
        owner: "a".into(),
        acquired_at: now,
        expires_at: now + Duration::seconds(5),
    }));
    assert!(!job.complete("b"));
    assert!(job.complete("a"));
}
