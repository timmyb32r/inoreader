use super::*;

fn rule(field: RuleField, action: RuleAction) -> Rule {
    Rule {
        id: RuleId::new(),
        subscription_id: SubscriptionId::new(),
        version: 7,
        enabled: true,
        field,
        needles: vec!["Rust".into()],
        action,
    }
}

#[test]
fn matching_is_case_insensitive_and_respects_selected_field() {
    assert!(rule(RuleField::Title, RuleAction::MarkRead).matches("RUST release", Some("unrelated")));
    assert!(!rule(RuleField::Title, RuleAction::MarkRead).matches("release", Some("Rust inside")));
    assert!(rule(RuleField::Text, RuleAction::MarkRead).matches("release", Some("Rust inside")));
    assert!(rule(RuleField::Both, RuleAction::MarkRead).matches("release", Some("Rust inside")));
}

#[test]
fn disabled_or_empty_rules_never_match() {
    let mut value = rule(RuleField::Both, RuleAction::MarkRead);
    value.enabled = false;
    assert!(!value.matches("Rust", Some("Rust")));
    value.enabled = true;
    value.needles.clear();
    assert!(!value.matches("Rust", Some("Rust")));
}

#[test]
fn operational_rule_rejects_empty_multiple_or_space_padded_phrases() {
    let mut value = rule(RuleField::Both, RuleAction::MarkRead);
    assert!(value.validate().is_ok());
    value.needles.clear();
    assert_eq!(value.validate(), Err(RuleValidationError::InvalidPhrase));
    value.needles = vec!["one".into(), "two".into()];
    assert_eq!(value.validate(), Err(RuleValidationError::InvalidPhrase));
    value.needles = vec![" padded ".into()];
    assert_eq!(value.validate(), Err(RuleValidationError::InvalidPhrase));
}

#[test]
fn manual_unread_and_restore_guards_win_over_automatic_rules() {
    let mut unread = ArticleState {
        protect_unread: true,
        ..ArticleState::default()
    };
    rule(RuleField::Both, RuleAction::MarkRead).apply(&mut unread);
    assert!(!unread.read);

    let mut restored = ArticleState {
        protect_restored: true,
        ..ArticleState::default()
    };
    rule(RuleField::Both, RuleAction::MoveToTrash).apply(&mut restored);
    assert!(!restored.trashed);
}

#[test]
fn evaluation_is_pinned_to_rule_version_subscription_and_enabled_state() {
    let value = rule(RuleField::Both, RuleAction::MarkRead);
    let pending = PendingRuleEvaluation {
        rule_id: value.id,
        rule_version: value.version,
        subscription_id: value.subscription_id,
    };
    assert!(pending.still_valid_for(&value));
    let mut changed = value.clone();
    changed.version += 1;
    assert!(!pending.still_valid_for(&changed));
    changed = value.clone();
    changed.enabled = false;
    assert!(!pending.still_valid_for(&changed));
    changed = value.clone();
    changed.subscription_id = SubscriptionId::new();
    assert!(!pending.still_valid_for(&changed));
}
