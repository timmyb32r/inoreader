use super::*;

#[test]
fn index_and_client_routes_use_non_cached_html() {
    let root = asset("/").expect("embedded index");
    assert_eq!(root.path, "/index.html");
    assert_eq!(cache_control(root), "no-cache");
    assert_eq!(asset("/workspace/example").unwrap().path, "/index.html");
}

#[test]
fn api_and_missing_static_assets_never_fall_back_to_html() {
    assert!(asset("/api/articles").is_none());
    assert!(asset("/assets/missing.js").is_none());
}
