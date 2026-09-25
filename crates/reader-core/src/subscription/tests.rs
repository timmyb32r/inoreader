use super::*;
use crate::ReasonPolicy;
use chrono::TimeZone;

fn event(reason: &str) -> StateEvent {
    StateEvent {
        reason: ReasonPolicy::new(128)
            .unwrap()
            .validate(reason.to_owned())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc.with_ymd_and_hms(2026, 9, 25, 12, 0, 0).unwrap(),
    }
}

fn subscription() -> Subscription {
    Subscription::new(
        SubscriptionId::new(),
        WorkspaceId::new(),
        Url::parse("https://example.test/feed.xml").unwrap(),
        "Example".into(),
    )
}

#[test]
fn repeated_pause_does_not_replace_reason_or_duplicate_history() {
    let mut value = subscription();
    let first = event("Editorial review");
    value.pause(first.clone());
    value.pause(event("retry reason"));
    assert_eq!(value.status(), &SubscriptionStatus::Paused(first.clone()));
    assert_eq!(value.history(), &[first]);
    assert_eq!(value.revision(), 1);
}

#[test]
fn resume_then_pause_keeps_both_historical_reasons() {
    let mut value = subscription();
    let first = event("First pause");
    let second = event("Second pause");
    value.pause(first.clone());
    value.resume();
    value.pause(second.clone());
    assert_eq!(value.history(), &[first, second.clone()]);
    assert_eq!(value.status(), &SubscriptionStatus::Paused(second));
    assert_eq!(value.revision(), 3);
}

#[test]
fn archived_subscription_cannot_be_resumed_as_a_pause() {
    let mut value = subscription();
    value.pause(event("Pause before unsubscribe"));
    value.archive();
    value.resume();
    assert_eq!(value.status(), &SubscriptionStatus::Archived);
    assert_eq!(value.revision(), 2);
    assert_eq!(value.history().len(), 1);
}

#[test]
fn paused_state_round_trips_without_losing_audit_event() {
    let mut value = subscription();
    let paused = event("Required reason");
    value.pause(paused.clone());
    let restored: Subscription =
        serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
    assert_eq!(
        restored.status(),
        &SubscriptionStatus::Paused(paused.clone())
    );
    assert_eq!(restored.history(), &[paused]);
}

#[test]
fn validation_rechecks_entire_pause_history_against_current_limit() {
    let mut value = subscription();
    value.pause(event("historical reason"));
    value.resume();
    assert!(value.validate(ReasonPolicy::new(8).unwrap()).is_err());
    assert!(value.validate(ReasonPolicy::new(64).unwrap()).is_ok());
}

#[test]
fn validation_rejects_paused_status_that_disagrees_with_history() {
    let mut value = subscription();
    value.status = SubscriptionStatus::Paused(event("current"));
    value.history.push(event("different history"));
    assert_eq!(
        value.validate(ReasonPolicy::new(64).unwrap()),
        Err(StateValidationError::StatusHistoryMismatch)
    );
}

#[test]
fn rename_and_restore_preserve_identity_source_and_pause_history() {
    let mut value = subscription();
    let id = value.id();
    let source = value.source_url().clone();
    value.pause(event("Historical pause"));
    value.archive();
    value.rename("Мой источник".into());
    value.restore();
    assert_eq!(value.id(), id);
    assert_eq!(value.source_url(), &source);
    assert_eq!(value.title(), "Мой источник");
    assert_eq!(value.status(), &SubscriptionStatus::Active);
    assert_eq!(value.history().len(), 1);
    assert_eq!(value.revision(), 4);
}
