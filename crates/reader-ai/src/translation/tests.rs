use super::*;
#[test]
fn exact_segments_reject_loss_normalization_missing_pinyin_and_fake_literals() {
    let valid = r#"{"translation":"Диск читает.","words":[{"source":"磁盘","pinyin":"cípán","translation":"диск"},{"source":"读取","pinyin":"dúqǔ","translation":"читает"}]}"#;
    let result = ParagraphTranslation::from_response("磁盘 读取。", valid).unwrap();
    let stored = serde_json::to_string(&result).unwrap();
    assert_eq!(
        serde_json::from_str::<ParagraphTranslation>(&stored)
            .unwrap()
            .source(),
        "磁盘 读取。"
    );
    for source in ["磁盘 读取。more", "磁盘 OTHER 读取。", ""] {
        assert!(ParagraphTranslation::from_response(source, valid).is_err());
    }
    for response in [
        valid.replace("\"cípán\"", "null"),
        valid.replace("\"dúqǔ\"", "\"\""),
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

#[test]
fn dictionary_annotations_preserve_original_spacing_and_punctuation() {
    let response = r#"{"translation":"Тревога из-за нового измерения.","words":[{"source":"被","pinyin":"bèi","translation":"подвергнуться"},{"source":"升维","pinyin":"shēngwéi","translation":"повышение размерности"},{"source":"的","pinyin":"de","translation":"частица"},{"source":"焦虑","pinyin":"jiāolǜ","translation":"тревога"}]}"#;
    for source in [
        "被 “升维” 的焦虑。",
        "被\u{a0}“升维”\t的\n焦虑。",
        "被“升维”的焦虑。",
    ] {
        let result = ParagraphTranslation::from_response(source, response).unwrap();
        assert_eq!(
            result
                .segments
                .iter()
                .map(TranslationSegment::source)
                .collect::<String>(),
            source
        );
        assert_eq!(result.source(), source);
        let stored = serde_json::to_string(&result).unwrap();
        assert_eq!(
            serde_json::from_str::<ParagraphTranslation>(&stored)
                .unwrap()
                .source(),
            source
        );
    }
    for source in [
        "被 “升维” OTHER 的焦虑。",
        "被 “降维” 的焦虑。",
        "被 的 “升维” 焦虑。",
        "被 “升维” 的焦虑。more",
    ] {
        assert!(matches!(
            ParagraphTranslation::from_response(source, response),
            Err(AiError::Translation(_))
        ));
    }
}

#[test]
fn repeated_terms_and_punctuation_inside_terms_keep_their_positions() {
    let response = r#"{"translation":"C++ и C++.","words":[{"source":"C++","pinyin":null,"translation":"C++"},{"source":"C++","pinyin":null,"translation":"C++"}]}"#;
    let result = ParagraphTranslation::from_response(" C++ — C++!", response).unwrap();
    assert_eq!(
        result
            .segments
            .iter()
            .map(TranslationSegment::source)
            .collect::<String>(),
        " C++ — C++!"
    );
    assert!(ParagraphTranslation::from_response("C++ OTHER C++", response).is_err());
    assert!(ParagraphTranslation::from_response("C++", response).is_err());
}

#[test]
fn real_failed_introduction_accepts_the_new_dictionary_without_changing_source() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/chinese-introduction.json")).unwrap();
    let source = fixture["source"].as_str().unwrap();
    let result =
        ParagraphTranslation::from_response(source, &fixture["response"].to_string()).unwrap();
    assert_eq!(
        result
            .segments
            .iter()
            .map(TranslationSegment::source)
            .collect::<String>(),
        source
    );
    assert!(result.segments.iter().any(|s| s.source() == "的"));
}

#[test]
fn persisted_literals_cannot_hide_missing_words() {
    let corrupt = r#"{"source":"遗漏","translation":"Пропуск","segments":[{"kind":"literal","source":"遗漏"}]}"#;
    assert!(serde_json::from_str::<ParagraphTranslation>(corrupt).is_err());
    for response in [
        r#"{"translation":"x","words":[]}"#,
        r#"{"translation":"x","words":[{"source":" ","pinyin":null,"translation":"x"}]}"#,
        r#"{"translation":"x","words":[{"source":"遗漏","pinyin":null,"translation":"x"}]}"#,
        r#"{"translation":"x","words":[{"source":"遗漏","pinyin":"yílòu","translation":""}]}"#,
    ] {
        assert!(ParagraphTranslation::from_response("遗漏", response).is_err());
    }
}
#[test]
fn orphan_text_membership_is_exact_and_does_not_accept_partial_blocks_or_code() {
    let html = "\n  页面标题\n<a href='/x'></a><div><p>完整<strong>段落</strong></p>尾文<pre>代码</pre><code>code</code></div>";
    for source in ["\n  页面标题\n", "尾文", "完整段落"] {
        assert!(contains_paragraph(html, source), "{source}");
    }
    for source in ["页面标题", "完整", "段落", "代码", "code", "\n", "invented"] {
        assert!(!contains_paragraph(html, source), "{source}");
    }
}
