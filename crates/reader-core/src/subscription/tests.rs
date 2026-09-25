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
    assert_eq!(value.source_title(), "Example");
    assert_eq!(value.custom_name(), Some("Мой источник"));
    assert_eq!(value.status(), &SubscriptionStatus::Active);
    assert_eq!(value.history().len(), 1);
    assert_eq!(value.revision(), 4);
}

#[test]
fn note_and_custom_name_are_independent_and_clearable() {
    let mut value = subscription();
    value.rename("My feed".into());
    value.set_personal_note("Check weekly".into());
    assert_eq!(value.title(), "My feed");
    assert_eq!(value.personal_note(), "Check weekly");
    value.rename(String::new());
    assert_eq!(value.title(), "Example");
    assert_eq!(value.source_title(), "Example");
    assert_eq!(value.personal_note(), "Check weekly");
}

#[test]
fn persisted_subscription_without_detail_fields_loads_losslessly() {
    let value = subscription();
    let mut document = serde_json::to_value(&value).unwrap();
    let object = document.as_object_mut().unwrap();
    object.remove("custom_name");
    object.remove("personal_note");
    object.remove("created_at");
    let restored: Subscription = serde_json::from_value(document).unwrap();
    assert_eq!(restored.source_title(), "Example");
    assert_eq!(restored.title(), "Example");
    assert_eq!(restored.custom_name(), None);
    assert_eq!(restored.personal_note(), "");
    assert_eq!(restored.created_at(), None);
}

#[test]
fn source_replacement_preserves_user_owned_metadata() {
    let mut value = subscription();
    value.rename("Custom".into());
    value.set_personal_note("Private note".into());
    value
        .replace_source(
            Url::parse("https://example.test/replacement.xml").unwrap(),
            "https://example.test/replacement.xml".into(),
            "Discovered replacement".into(),
        )
        .unwrap();
    assert_eq!(value.title(), "Custom");
    assert_eq!(value.source_title(), "Discovered replacement");
    assert_eq!(value.personal_note(), "Private note");
    assert_eq!(
        value.source_url().as_str(),
        "https://example.test/replacement.xml"
    );
}

#[test]
fn exact_source_url_is_preserved_without_hidden_normalization() {
    let exact = "HTTPS://EXAMPLE.TEST:443/feed";
    let value = Subscription::new_with_exact_url(
        SubscriptionId::new(),
        WorkspaceId::new(),
        Url::parse(exact).unwrap(),
        exact.into(),
        "Feed".into(),
    )
    .unwrap();
    assert_eq!(value.source_url_exact(), exact);
    assert_ne!(value.source_url().as_str(), exact);
}

#[test]
fn equal_source_urls_never_share_user_owned_subscription_state() {
    let url = Url::parse("https://example.test/feed").unwrap();
    let mut first = Subscription::new(
        SubscriptionId::new(),
        WorkspaceId::new(),
        url.clone(),
        "First".into(),
    );
    let second = Subscription::new(
        SubscriptionId::new(),
        WorkspaceId::new(),
        url,
        "Second".into(),
    );
    first.set_personal_note("private".into());
    first.rename("custom".into());
    assert_eq!(second.personal_note(), "");
    assert_eq!(second.title(), "Second");
    assert_ne!(first.workspace_id(), second.workspace_id());
}
