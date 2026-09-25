use super::*;
use crate::ReasonPolicy;
use chrono::TimeZone;

fn event(reason: &str) -> WorkspaceStateEvent {
    WorkspaceStateEvent {
        reason: ReasonPolicy::new(128)
            .unwrap()
            .validate(reason.to_owned())
            .unwrap(),
        actor: ActorId::new(),
        at: Utc.with_ymd_and_hms(2026, 9, 25, 12, 0, 0).unwrap(),
    }
}

#[test]
fn archive_is_idempotent_and_restore_preserves_audit_history() {
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Reading".into());
    let archived = event("Maintenance");
    workspace.archive(archived.clone());
    workspace.archive(event("must not overwrite the first transition"));

    assert!(!workspace.accepts_delivery());
    assert_eq!(workspace.revision(), 1);
    assert_eq!(workspace.history(), &[archived]);

    workspace.restore();
    workspace.restore();
    assert!(workspace.accepts_delivery());
    assert_eq!(workspace.revision(), 2);
    assert_eq!(workspace.history().len(), 1);
}

#[test]
fn rename_only_advances_revision_for_an_actual_change() {
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Reading".into());
    workspace.rename("Reading".into());
    assert_eq!(workspace.revision(), 0);
    workspace.rename("Research".into());
    assert_eq!(workspace.name(), "Research");
    assert_eq!(workspace.revision(), 1);
}

#[test]
fn archived_state_round_trips_with_reason_actor_time_and_history() {
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Reading".into());
    let archived = event("Operator pause");
    workspace.archive(archived.clone());
    let json = serde_json::to_string(&workspace).unwrap();
    let restored: Workspace = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored.status(),
        &WorkspaceStatus::Archived(archived.clone())
    );
    assert_eq!(restored.history(), &[archived]);
}

#[test]
fn validation_rechecks_persisted_reasons_against_current_configuration() {
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Reading".into());
    workspace.archive(event("long reason"));
    assert!(workspace.validate(ReasonPolicy::new(4).unwrap()).is_err());
    assert!(workspace.validate(ReasonPolicy::new(32).unwrap()).is_ok());
}

#[test]
fn validation_rejects_archived_status_that_is_not_the_latest_history_event() {
    let mut workspace = Workspace::new(WorkspaceId::new(), AccountId::new(), "Reading".into());
    workspace.status = WorkspaceStatus::Archived(event("current"));
    workspace.history.push(event("different history"));
    assert_eq!(
        workspace.validate(ReasonPolicy::new(64).unwrap()),
        Err(StateValidationError::StatusHistoryMismatch)
    );
}
