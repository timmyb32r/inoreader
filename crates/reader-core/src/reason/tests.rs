use super::*;

#[test]
fn rejects_blank_and_preserves_valid_text_exactly() {
    let policy = ReasonPolicy::new(32).unwrap();
    assert_eq!(policy.validate(" \n".into()), Err(ReasonError::Blank));
    assert_eq!(
        policy.validate("  maintenance  ".into()).unwrap().as_str(),
        "  maintenance  "
    );
}

#[test]
fn deserialization_cannot_construct_a_blank_reason() {
    assert!(serde_json::from_str::<Reason>(r#""   ""#).is_err());
    let restored = serde_json::from_str::<Reason>(r#""  maintenance  ""#).unwrap();
    assert_eq!(restored.as_str(), "  maintenance  ");
}
