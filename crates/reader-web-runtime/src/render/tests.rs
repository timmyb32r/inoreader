use super::*;

#[test]
fn malicious_markup_is_data_and_raw_source_is_unchanged() {
    let raw = r#"<article><script>steal()</script><p onclick="steal()">hello</p><img src="https://tracker.test/x"><a href="javascript:steal()">bad</a></article>"#;
    let original = raw.to_owned();
    let rendered = SafeRenderedContent::from_untrusted_html(raw);
    assert_eq!(raw, original);
    assert!(!rendered.html().contains("script"));
    assert!(!rendered.html().contains("onclick"));
    assert!(rendered.html().contains("https://tracker.test/x"));
    assert!(!rendered.html().contains("onerror"));
    assert!(!rendered.html().contains("javascript:"));
}

#[test]
fn escaped_article_markup_is_decoded_sanitized_and_resolved() {
    let raw = r#"&amp;lt;picture&amp;gt;&amp;lt;source srcset=&amp;quot;/hero.webp 2x&amp;quot; type=&amp;quot;image/webp&amp;quot;&amp;gt;&amp;lt;img src=&amp;quot;/hero.png&amp;quot; alt=&amp;quot;Diagram&amp;quot; onerror=&amp;quot;steal()&amp;quot;&amp;gt;&amp;lt;/picture&amp;gt;&amp;lt;h2&amp;gt;Section&amp;lt;/h2&amp;gt;&amp;lt;p&amp;gt;Body &amp;amp;amp; details.&amp;lt;/p&amp;gt;"#;
    let rendered = SafeRenderedContent::from_untrusted_html_with_base(
        raw,
        Some("https://example.test/articles/one"),
    );
    assert!(rendered.html().contains("<picture>"));
    assert!(rendered.html().contains("https://example.test/hero.png"));
    assert!(rendered
        .html()
        .contains("srcset=\"https://example.test/hero.webp 2x\""));
    assert!(rendered.html().contains("<h2>Section</h2>"));
    assert!(rendered.html().contains("Body &amp; details."));
    assert!(!rendered.html().contains("onerror"));
}

#[test]
fn every_responsive_image_candidate_is_resolved_against_the_article() {
    let raw = r#"<picture><source srcset="/small.webp 384w, /large.webp 1536w"><img src="/fallback.png" srcset="thumb.png 1x, hero.png 2x"></picture>"#;
    let rendered = SafeRenderedContent::from_untrusted_html_with_base(
        raw,
        Some("https://publisher.test/posts/article"),
    );

    assert!(rendered.html().contains(
        "srcset=\"https://publisher.test/small.webp 384w, https://publisher.test/large.webp 1536w\""
    ));
    assert!(rendered.html().contains(
        "srcset=\"https://publisher.test/posts/thumb.png 1x, https://publisher.test/posts/hero.png 2x\""
    ));
    assert!(!rendered.html().contains("srcset=\"/"));
}

#[test]
fn unsafe_or_malformed_responsive_image_candidates_are_removed() {
    let raw = r#"<img src="/fallback.png" srcset="javascript:steal() 1x, /hero.png 2x"><source srcset="/hero.webp nonsense">"#;
    let rendered = SafeRenderedContent::from_untrusted_html_with_base(
        raw,
        Some("https://publisher.test/posts/article"),
    );

    assert!(rendered
        .html()
        .contains("https://publisher.test/fallback.png"));
    assert!(!rendered.html().contains("srcset"));
    assert!(!rendered.html().contains("javascript:"));
}

#[test]
fn iframe_contract_has_an_opaque_origin_and_no_execution() {
    let contract = SandboxContract;
    assert!(!contract.allows_scripts());
    assert!(contract.has_opaque_origin());
    assert!(!SandboxContract::DOCUMENT_CSP.contains("unsafe"));
}
