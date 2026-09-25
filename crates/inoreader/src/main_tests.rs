use super::*;

fn manifest() -> SeedManifest {
    SeedManifest {
        schema_version: 1,
        owner_account_id: uuid::Uuid::new_v4(),
        workspace_id: uuid::Uuid::new_v4(),
        items: vec![SeedItem {
            source_inventory_id: "example".into(),
            idempotency_key: "personal_feed:example".into(),
            selected: true,
            status: "reviewed".into(),
            configuration: serde_json::json!({"name":"Example","url":"https://example.test/blog","link_selector":"article a"}),
            note: None,
        }],
    }
}

#[test]
fn web_recipe_is_preserved_for_validated_import_mapping() {
    let values = seed_values(manifest()).unwrap();
    assert!(matches!(values[0].2, SeedSource::Imported));
    assert_eq!(values[0].3["link_selector"], "article a");
}

#[test]
fn feed_url_selects_feed_ingest_instead_of_web_recipe() {
    let mut value = manifest();
    value.items[0].configuration = serde_json::json!({"name":"Example feed","url":"https://example.test/blog","feed_url":"https://example.test/feed.xml"});
    let values = seed_values(value).unwrap();
    assert_eq!(
        values[0].1.source_url().as_str(),
        "https://example.test/feed.xml"
    );
    assert!(matches!(values[0].2, SeedSource::Feed));
}

#[test]
fn feed_seed_identity_is_stable_for_owner_workspace_and_key() {
    let mut first = manifest();
    first.items[0].configuration = serde_json::json!({"name":"Example feed","url":"https://example.test/blog","feed_url":"https://example.test/feed.xml"});
    let owner = first.owner_account_id;
    let workspace = first.workspace_id;
    let mut second = manifest();
    second.owner_account_id = owner;
    second.workspace_id = workspace;
    second.items[0].configuration = first.items[0].configuration.clone();
    let first = seed_values(first).unwrap();
    let second = seed_values(second).unwrap();
    assert_eq!(first[0].1.id(), second[0].1.id());
    assert_eq!(first[0].0, "personal_feed:example");
}

#[test]
fn visual_selection_chooses_the_smallest_repeated_group_at_the_coordinate() {
    let rect = |x, y, width, height| reader_ingest::VisualRect {
        x,
        y,
        width,
        height,
    };
    let state = VisualSnapshotState {
        session_id: uuid::Uuid::new_v4(),
        workspace_id: uuid::Uuid::new_v4(),
        expires_at: chrono::Utc::now(),
        width: 100,
        height: 100,
        groups: vec![
            reader_ingest::VisualCandidateGroup {
                id: "outer".into(),
                selector: "main.cards".into(),
                boxes: vec![rect(0.0, 0.0, 80.0, 80.0), rect(80.0, 0.0, 20.0, 20.0)],
            },
            reader_ingest::VisualCandidateGroup {
                id: "inner".into(),
                selector: "article.card".into(),
                boxes: vec![rect(10.0, 10.0, 20.0, 20.0), rect(40.0, 10.0, 20.0, 20.0)],
            },
        ],
    };
    let selected = select_visual_group(&state, 15.0, 15.0).unwrap();
    assert_eq!(selected.selector.expression, "article.card");
    assert_eq!(selected.count, 2);
    assert!(select_visual_group(&state, 99.0, 99.0).is_err());
}

#[test]
fn spa_auth_links_resolve_to_embedded_index_without_masking_api_or_assets() {
    assert_eq!(ui_asset("/invite").map(|v| v.path), Some("/index.html"));
    assert_eq!(
        ui_asset("/reset-password").map(|v| v.path),
        Some("/index.html")
    );
    assert!(ui_asset("/api/not-found").is_none());
    assert!(ui_asset("/missing.js").is_none());
}

#[test]
fn web_feed_draft_preserves_extraction_and_validated_page_depth() {
    let draft:WebFeedRecipeDraft=serde_json::from_value(serde_json::json!({"workspaceId":uuid::Uuid::new_v4(),"url":"https://example.test/news","selector":"a.story","loading":"browser","preview":true,"selectorLanguage":"css","viewport":"desktop","listingUrl":"https://example.test/archive","cardSelector":"article.card","titleSelector":"h2","dateSelector":"time","contentSelector":".body","waitSelector":"main","urlPattern":"/story/","maxPages":3,"nextPage":{"language":"css","expression":"a.next"}})).unwrap();
    let recipe = recipe_from_draft(&draft, 5, 20).unwrap();
    assert_eq!(recipe.max_pages(), 3);
    assert_eq!(
        recipe.extraction().card_selector().unwrap().expression(),
        "article.card"
    );
    assert_eq!(
        recipe.extraction().title_selector().unwrap().expression(),
        "h2"
    );
    assert_eq!(
        recipe.extraction().listing_url().unwrap().as_str(),
        "https://example.test/archive"
    );
    assert!(recipe_from_draft(&draft, 2, 20).is_err());
}
