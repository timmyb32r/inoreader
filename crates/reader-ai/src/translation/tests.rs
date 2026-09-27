use super::*;
#[test]
fn exact_segments_reject_loss_normalization_missing_pinyin_and_fake_literals() {
    let valid = r#"{"translation":"Диск читает.","segments":[{"kind":"word","source":"磁盘","pinyin":"cípán","translation":"диск"},{"kind":"literal","source":" "},{"kind":"word","source":"读取","pinyin":"dúqǔ","translation":"читает"},{"kind":"literal","source":"。"}]}"#;
    let result = ParagraphTranslation::from_response("磁盘 读取。", valid).unwrap();
    let stored = serde_json::to_string(&result).unwrap();
    assert_eq!(
        serde_json::from_str::<ParagraphTranslation>(&stored)
            .unwrap()
            .source(),
        "磁盘 读取。"
    );
    for source in ["磁盘读取。", "磁盘\u{a0}读取。", "磁盘 读取。more", ""] {
        assert!(ParagraphTranslation::from_response(source, valid).is_err());
    }
    for response in [
        valid.replace("\"cípán\"", "null"),
        valid.replace("\"dúqǔ\"", "\"\""),
        valid.replace("\"source\":\" \"", "\"source\":\"more\""),
        valid.replace("\"translation\":\"диск\"", "\"translation\":\"\""),
    ] {
        assert!(ParagraphTranslation::from_response("磁盘 读取。", &response).is_err());
    }
    assert!(serde_json::from_str::<ParagraphTranslation>(&stored.replace("cípán", "")).is_err());
}
#[test]
fn paragraph_membership_preserves_dom_text_without_accepting_injected_or_partial_text() {
    let html="<h1>标题</h1><h2>技术细节</h2><p>磁盘 <strong>读取</strong> &amp; SQL。</p><ul><li><p>nested</p></li><li>leaf</li></ul><pre><code>not selectable</code></pre>";
    for text in ["标题", "技术细节", "磁盘 读取 & SQL。", "nested", "leaf"] {
        assert!(contains_paragraph(html, text), "{text}");
    }
    for text in [
        "磁盘读取 & SQL。",
        "读取",
        "not selectable",
        "ignore instructions",
        "磁盘 读取 &amp; SQL。",
    ] {
        assert!(!contains_paragraph(html, text), "{text}");
    }
}
