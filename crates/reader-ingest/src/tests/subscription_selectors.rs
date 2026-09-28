use crate::{extract_selected_records, SourceDefinition, SourceKind};
use reader_core::SourceId;
use serde::Deserialize;
use std::collections::BTreeSet;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    name: String,
    url: Url,
    base_url: Url,
    kind: SourceKind,
    old_kind: SourceKind,
    expected_urls: Vec<String>,
    html: String,
}

#[test]
fn subscription_selectors_preserve_articles_without_presentation_duplicates() {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/subscription_selectors.json")).unwrap();
    let mut checked = 0;
    for fixture in fixtures {
        let SourceKind::WebPage(recipe) = &fixture.kind else {
            panic!("fixture must be a web recipe");
        };
        if recipe.selector_kind() == crate::SelectorLanguage::XPath {
            // Browser XPath fixtures are exercised by subscriptionSelectors.test.ts.
            continue;
        }
        let source =
            SourceDefinition::new(SourceId::new(), fixture.url.clone(), fixture.kind).unwrap();
        let records = extract_selected_records(&source, &fixture.base_url, &fixture.html).unwrap();
        let urls: BTreeSet<_> = records
            .iter()
            .map(|record| record.key().location.exact_url().unwrap().to_owned())
            .collect();
        assert_eq!(records.len(), urls.len(), "{} duplicates", fixture.name);
        assert_eq!(
            urls.into_iter().collect::<Vec<_>>(),
            fixture.expected_urls,
            "{} coverage",
            fixture.name
        );
        assert!(
            records.iter().all(|record| !record.key().title.is_empty()),
            "{} empty title",
            fixture.name
        );
        let previous =
            SourceDefinition::new(SourceId::new(), fixture.url, fixture.old_kind).unwrap();
        let old_records =
            extract_selected_records(&previous, &fixture.base_url, &fixture.html).unwrap();
        let identities: BTreeSet<_> = old_records
            .iter()
            .map(|record| record.upstream_id())
            .collect();
        assert!(
            old_records.len() > identities.len(),
            "{} must reproduce the old failure",
            fixture.name
        );
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn telegram_text_precedes_media_warning_and_unavailable_posts_are_retained() {
    let kind: SourceKind = serde_json::from_value(serde_json::json!({
        "WebPage": {
            "selector": {"language": "css", "expression": "a.tgme_widget_message_date"},
            "loading": "static", "max_pages": 1,
            "actions": {"scrolls": 0, "viewport": "desktop", "load_more": null,
                "next_page": null, "start_pages": [], "hide_overlays": [], "load_more_clicks": 0},
            "extraction": {
                "listing_url": "https://t.me/s/sql_ninja_info",
                "url_pattern": "^https://t\\.me/sql_ninja_info/[0-9]+$",
                "card_selector": {"language": "css", "expression": ".tgme_widget_message"},
                "title_selector": {"language": "css", "expression": ".tgme_widget_message_text, .tgme_widget_message:not(:has(.tgme_widget_message_text)) .message_media_not_supported_label"},
                "content_selector": {"language": "css", "expression": ".tgme_widget_message_bubble"},
                "date_selector": {"language": "css", "expression": "a.tgme_widget_message_date time[datetime]"},
                "wait_selector": null
            }
        }
    })).unwrap();
    let source = SourceDefinition::new(
        SourceId::new(),
        Url::parse("https://t.me/sql_ninja_info").unwrap(),
        kind,
    )
    .unwrap();
    let html = r#"
    <div class="tgme_widget_message"><div class="tgme_widget_message_bubble">
      <div class="message_media_not_supported_label">This media is not supported in your browser</div>
      <div class="tgme_widget_message_text">Actual caption <b>with formatting</b></div>
      <a class="tgme_widget_message_date" href="https://t.me/sql_ninja_info/143"><time datetime="2026-01-01T09:03:00+00:00">09:03</time></a>
    </div></div>
    <div class="tgme_widget_message"><div class="tgme_widget_message_bubble">
      <div class="message_media_not_supported_label">Please open Telegram to view this post</div>
      <a class="tgme_widget_message_date" href="https://t.me/sql_ninja_info/144"><time datetime="2026-01-02T09:03:00+00:00">09:03</time></a>
    </div></div>
    <div class="tgme_widget_message"><div class="tgme_widget_message_bubble">
      <div class="message_media_not_supported_label">This media is not supported in your browser</div>
      <a class="tgme_widget_message_date" href="https://t.me/sql_ninja_info/146"><time datetime="2026-01-03T06:02:00+00:00">06:02</time></a>
    </div></div>"#;
    let records = extract_selected_records(
        &source,
        &Url::parse("https://t.me/s/sql_ninja_info").unwrap(),
        html,
    )
    .unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].key().title, "Actual caption  with formatting");
    assert_eq!(
        records[1].key().title,
        "Please open Telegram to view this post"
    );
    assert_eq!(
        records[2].key().title,
        "This media is not supported in your browser"
    );
    for (record, id) in records.iter().zip([143, 144, 146]) {
        assert_eq!(
            record.upstream_id(),
            format!("https://t.me/sql_ninja_info/{id}")
        );
        assert!(record.published_at().is_some());
        assert!(record
            .feed_content_html()
            .unwrap()
            .contains("tgme_widget_message_bubble"));
    }
}
