use super::*;

#[test]
fn malicious_markup_is_data_and_raw_source_is_unchanged() {
    let raw = r#"<article><script>steal()</script><p onclick="steal()">hello</p><img src="https://tracker.test/x"><a href="javascript:steal()">bad</a></article>"#;
    let original = raw.to_owned();
    let rendered = SafeRenderedContent::from_untrusted_html(raw);
    assert_eq!(raw, original);
    assert!(!rendered.html().contains("script"));
    assert!(!rendered.html().contains("onclick"));
    assert!(!rendered.html().contains("tracker.test"));
    assert!(!rendered.html().contains("javascript:"));
}

#[test]
fn iframe_contract_has_an_opaque_origin_and_no_execution() {
    let contract = SandboxContract;
    assert!(!contract.allows_scripts());
    assert!(contract.has_opaque_origin());
    assert!(!SandboxContract::DOCUMENT_CSP.contains("unsafe"));
}
