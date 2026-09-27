use super::*;

#[test]
fn stored_recipes_cannot_bypass_constructor_invariants() {
    let good = WebFeedRecipe::new("article a".into(), WebLoading::Static).unwrap();
    let json = serde_json::to_value(&good).unwrap();
    assert_eq!(
        serde_json::from_value::<WebFeedRecipe>(json.clone()).unwrap(),
        good
    );
    for pointer in ["/selector/expression", "/extraction/url_pattern"] {
        let mut broken = json.clone();
        *broken.pointer_mut(pointer).unwrap() = "[".into();
        assert!(serde_json::from_value::<WebFeedRecipe>(broken).is_err());
    }
    let mut broken = json.clone();
    broken["max_pages"] = 0.into();
    assert!(serde_json::from_value::<WebFeedRecipe>(broken).is_err());
    let mut broken = json.clone();
    broken["selector"]["language"] = "xpath".into();
    broken["selector"]["expression"] = "//a".into();
    assert!(serde_json::from_value::<WebFeedRecipe>(broken).is_err());
    let mut broken = json;
    broken["actions"]["scrolls"] = serde_json::json!(usize::MAX);
    broken["actions"]["load_more_clicks"] = 1.into();
    assert!(serde_json::from_value::<WebFeedRecipe>(broken).is_err());
}

#[test]
fn prepared_draft_retains_authored_fields_and_rejects_invalid_policy() {
    let raw = serde_json::json!({"workspaceId":Uuid::new_v4(),"url":"https://example.com/path?x=1","selector":"article > a","loading":"browser","preview":false,"maxPages":2,"scrolls":2});
    let draft: WebFeedRecipeDraft = serde_json::from_value(raw).unwrap();
    let original = serde_json::to_value(&draft).unwrap();
    let prepared = PreparedWebFeed::new(draft.clone(), 2, 2).unwrap();
    assert_eq!(serde_json::to_value(prepared.draft()).unwrap(), original);
    assert_eq!(
        serde_json::to_value(prepared.clone().draft()).unwrap(),
        original
    );
    assert!(PreparedWebFeed::new(draft.clone(), 1, 2).is_err());
    assert!(PreparedWebFeed::new(draft.clone(), 2, 1).is_err());
    assert!(PreparedWebFeed::new(draft.clone(), 0, 2).is_err());
    let mut bad = draft.clone();
    bad.url = "file:///private/file".into();
    assert!(PreparedWebFeed::new(bad, 2, 2).is_err());
    let mut bad = draft;
    bad.workspace_id = Uuid::nil();
    assert!(PreparedWebFeed::new(bad, 2, 2).is_err());
}
