use crate::*;
use serde_json::json;

#[test]
fn paragraphs_preserve_full_explanations_without_crossing_boundaries() {
    let source = styled_html("<b>Microsoft Fabric</b> становится платформой<br><br><b>Wi-Fi</b> - протокол.<br>Ещё строка.<br> - Пункт<br><br>В середине <b>CDC</b> — не определение.<br><br><b>CDC</b> — Change Data Capture.").unwrap();
    let defs = definitions(&source).unwrap();
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].term(), "Wi-Fi");
    assert_eq!(
        defs[0].paragraph().text(),
        "Wi-Fi - протокол.\nЕщё строка.\n - Пункт"
    );
    assert_eq!(defs[1].term(), "CDC");
    let encoded = serde_json::to_string(&defs).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<Definition>>(&encoded).unwrap(),
        defs
    );
}

#[test]
fn supports_three_separators_and_nested_marks_without_normalization() {
    for separator in ["-", "–", "—"] {
        let html = format!("<p><strong><a href=\"https://example.org\"><i>数据库</i></a></strong>&nbsp;{separator}&nbsp;база &amp; данные 😎</p>");
        let defs = definitions(&styled_html(&html).unwrap()).unwrap();
        assert_eq!(defs[0].term(), "数据库");
        assert_eq!(
            defs[0].paragraph().text(),
            format!("数据库\u{a0}{separator}\u{a0}база & данные 😎")
        );
        assert_eq!(defs[0].paragraph().marks().len(), 3);
    }
}

#[test]
fn telegram_utf16_is_validated_not_reinterpreted_as_utf8() {
    let value =
        json!({"text":"😎\n\nCDC — описание", "entities":[{"type":"bold","offset":4,"length":3}]});
    let rich = telegram_text(&value).unwrap();
    let defs = definitions(&rich).unwrap();
    assert_eq!(defs[0].term(), "CDC");
    assert_eq!(defs[0].position(), 4);
    assert!(telegram_text(
        &json!({"text":"😎CDC", "entities":[{"type":"bold","offset":1,"length":3}]})
    )
    .is_err());
    assert!(serde_json::from_value::<StyledText>(
        json!({"text":"😎", "marks":[{"style":{"kind":"bold"},"start":0,"end":1}]})
    )
    .is_err());
    assert!(StyledText::new(
        "a".into(),
        vec![TextMark {
            start: 1,
            end: 1,
            style: MarkKind::Bold
        }]
    )
    .is_err());
}

#[test]
fn unknown_formatting_is_preserved_as_unindexed_post() {
    let raw = json!({"id":3,"channel":CHANNEL_USERNAME,"permalink":"https://t.me/reading_data_news/3","timestamp":null,"formatted_html":"<img src=\"x\">","future_field":{"keep":true}}).to_string();
    let post = ObservedPost::archive(raw.clone()).unwrap();
    assert_eq!(post.raw(), raw);
    assert!(post.published_at().is_none());
    assert!(matches!(
        post.projection(),
        PostProjection::Unindexed { .. }
    ));
    assert!(ObservedPost::archive(raw.replace(CHANNEL_USERNAME, "wrong")).is_err());
}

#[test]
fn serialized_definition_cannot_forge_term_or_bold_contract() {
    let wire = json!({"term":"wrong","position":0,"paragraph":{"text":"CDC — изменения","marks":[{"style":{"kind":"bold"},"start":0,"end":3}]}});
    assert!(serde_json::from_value::<Definition>(wire).is_err());
    assert!(
        definitions(&styled_html("CDC — без bold<br><br><b>CDC</b> — ").unwrap())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn blank_line_runs_and_adjacent_bold_spans_preserve_exact_paragraphs() {
    let rich=styled_html("intro<br><br><br><b>CD</b><strong>C</strong> — объяснение<br><br><br><br><b>SQL</b> - запросы").unwrap();
    let defs = definitions(&rich).unwrap();
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].term(), "CDC");
    assert_eq!(defs[0].paragraph().text(), "CDC — объяснение");
    assert_eq!(defs[1].paragraph().text(), "SQL - запросы");
}
