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

#[test]
fn favicon_is_an_embedded_multi_resolution_icon() {
    let icon = asset("/favicon.ico").expect("embedded favicon");
    assert_eq!(icon.content_type, "image/x-icon");
    assert_eq!(&icon.bytes[..6], &[0, 0, 1, 0, 4, 0]);
    let index = asset("/").unwrap();
    assert!(std::str::from_utf8(index.bytes)
        .unwrap()
        .contains("/favicon.ico"));
}
