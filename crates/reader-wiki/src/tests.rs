use super::*;
use uuid::Uuid;
fn limits() -> Limits {
    LimitsInput {
        name_bytes: 100,
        markdown_bytes: 1000,
        search_excerpt_characters: 200,
        search_bytes: 100,
        page_size: 20,
        draft_save_delay_ms: 500,
    }
    .try_into()
    .unwrap()
}
#[test]
fn names_are_exact_and_invalid_input_is_rejected() {
    let l = limits();
    for name in ["", "a\nb", "[[link]]", "a\0b"] {
        assert!(l.name(name).is_err());
    }
    for name in [" 页面 ", "Page", "page", "Страница"] {
        assert!(l.name(name).is_ok());
    }
    let name = " 页面 ";
    let w = Write::new(
        WriteInput {
            operation: Uuid::new_v4(),
            page: Uuid::new_v4(),
            expected_revision: None,
            change: ChangeInput::Save {
                name: name.into(),
                markdown: " exact\n".into(),
            },
        },
        &l,
    )
    .unwrap();
    match &w.input().change {
        ChangeInput::Save { name: n, markdown } => {
            assert_eq!(n, name);
            assert_eq!(markdown, " exact\n");
        }
        _ => panic!("save"),
    }
}
#[test]
fn deserialized_commands_cross_the_same_validation_boundary() {
    let raw = serde_json::json!({"operation":Uuid::new_v4(),"page":Uuid::new_v4(),"expected_revision":null,"change":{"action":"trash"}});
    let input = serde_json::from_value(raw).unwrap();
    assert!(Write::new(input, &limits()).is_err());
    let mut raw = limits().input().clone();
    raw.page_size = 0;
    assert!(Limits::try_from(raw).is_err());
}
#[test]
fn links_preserve_names_without_creating_pages() {
    assert_eq!(
        link_names("[[ 页面 ]] [[ 页面 ]] [[Another]] [[bad\nname]]"),
        vec![" 页面 ", "Another"]
    );
}
#[test]
fn only_editors_and_owners_can_edit() {
    assert!(!Role::Reader.can_edit());
    assert!(Role::Owner.can_edit());
    assert!(Role::Editor.can_edit());
}

#[test]
fn configured_limits_reject_unsupported_storage_and_timer_capacities() {
    let mut raw = limits().input().clone();
    raw.name_bytes = 2001;
    assert!(Limits::try_from(raw).is_err());
    let mut raw = limits().input().clone();
    raw.draft_save_delay_ms = i32::MAX as u64 + 1;
    assert!(Limits::try_from(raw).is_err());
    let mut raw = limits().input().clone();
    raw.search_excerpt_characters = 0;
    assert!(Limits::try_from(raw).is_err());
}

#[test]
fn parent_changes_require_revision_and_reject_self_before_storage() {
    let id = Uuid::new_v4();
    for revision in [None, Some(Uuid::new_v4())] {
        let raw = serde_json::json!({"operation":Uuid::new_v4(),"page":id,"expected_revision":revision,"change":{"action":"set_parent","parent":id}});
        assert!(Write::new(serde_json::from_value(raw).unwrap(), &limits()).is_err());
    }
    let valid = WriteInput {
        operation: Uuid::new_v4(),
        page: id,
        expected_revision: Some(Uuid::new_v4()),
        change: ChangeInput::SetParent { parent: None },
    };
    assert!(Write::new(valid, &limits()).is_ok());
}
