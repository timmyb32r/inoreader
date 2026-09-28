use super::*;
fn source() -> SourceDefinition {
    SourceDefinition::new(
        reader_core::SourceId::new(),
        Url::parse("https://t.me/cdo_club").unwrap(),
        SourceKind::BuiltIn(BuiltInAdapter::Telegram {
            max_pages: std::num::NonZeroUsize::new(2).unwrap(),
        }),
    )
    .unwrap()
}
const PHOTO: &str = r#"<div class="tgme_channel_history"><div class="tgme_widget_message" data-post="cdo_club/3078"><div class="tgme_widget_message_bubble"><a class="tgme_widget_message_photo_wrap" style="background-image:url('https://cdn.example/photo.jpg')"></a><a class="tgme_widget_message_date"><time datetime="2026-09-27T14:53:00+00:00"></time></a></div></div><a class="tme_messages_more" data-before="3078" href="/s/cdo_club?before=3078"></a></div>"#;
#[test]
fn media_only_post_keeps_absent_title_permalink_date_and_visible_photo() {
    let (records, next) = parse_page(&source(), "cdo_club", PHOTO).unwrap();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.upstream_id(), "https://t.me/cdo_club/3078");
    assert!(record.key().title.is_empty());
    assert!(record.key().description.is_none());
    assert!(record.published_at().is_some());
    let safe = reader_web_runtime::SafeRenderedContent::from_untrusted_html(
        record.feed_content_html().unwrap(),
    );
    assert!(safe.html().contains("<img"));
    assert!(safe.html().contains("https://cdn.example/photo.jpg"));
    assert_eq!(next.as_deref(), Some("/s/cdo_club?before=3078"));
}
#[test]
fn caption_edit_keeps_upstream_identity_and_preserves_full_caption() {
    let src = source();
    let one=PHOTO.replace("<a class=\"tgme_widget_message_photo_wrap\"", "<div class=\"tgme_widget_message_text\"><b>Exact</b> full caption</div><a class=\"tgme_widget_message_photo_wrap\"");
    let two = one.replace("full caption", "edited caption");
    let old = parse_page(&src, "cdo_club", &one).unwrap().0.remove(0);
    let new = parse_page(&src, "cdo_club", &two).unwrap().0.remove(0);
    assert_eq!(old.upstream_id(), new.upstream_id());
    assert_eq!(
        new.key().description.as_deref(),
        Some("Exact edited caption")
    );
    assert!(new
        .feed_content_html()
        .unwrap()
        .contains("<b>Exact</b> edited caption"));
}
#[test]
fn wrong_channel_and_missing_history_fail_without_dropping_posts() {
    assert!(parse_page(&source(), "other", PHOTO).is_err());
    assert!(parse_page(&source(), "cdo_club", "<html>Login</html>").is_err());
    assert!(serde_json::from_str::<BuiltInAdapter>(r#"{"telegram":{"max_pages":0}}"#).is_err());
}

struct Pages {
    bodies: std::sync::Mutex<std::collections::VecDeque<String>>,
    urls: std::sync::Mutex<Vec<Url>>,
}
#[async_trait::async_trait]
impl crate::BrowserHttpClient for Pages {
    async fn execute(
        &self,
        request: PreparedRequest,
    ) -> Result<crate::BrowserHttpResponse, FetchError> {
        self.urls.lock().unwrap().push(request.url.clone());
        Ok(crate::BrowserHttpResponse {
            final_url: request.url,
            status: 200,
            headers: http::HeaderMap::new(),
            body: self
                .bodies
                .lock()
                .unwrap()
                .pop_front()
                .expect("bounded expected page")
                .into_bytes(),
        })
    }
}
#[tokio::test]
async fn configured_history_cursor_preserves_all_posts_and_exact_identity() {
    let older = PHOTO.replace("3078", "3077");
    let http = Arc::new(Pages {
        bodies: std::sync::Mutex::new([PHOTO.to_owned(), older].into()),
        urls: std::sync::Mutex::new(vec![]),
    });
    let collector = BuiltInAdapterCollector::new(http.clone());
    let records = collector.collect(&source()).await.unwrap();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].upstream_id(), records[1].upstream_id());
    assert_eq!(
        http.urls.lock().unwrap()[1].as_str(),
        "https://t.me/s/cdo_club?before=3078"
    );
    assert!(http.bodies.lock().unwrap().is_empty());
}
#[test]
fn identical_public_bytes_do_not_merge_user_source_record_ownership() {
    let first = source();
    let second = source();
    let a = parse_page(&first, "cdo_club", PHOTO).unwrap().0.remove(0);
    let b = parse_page(&second, "cdo_club", PHOTO).unwrap().0.remove(0);
    assert_eq!(a.upstream_id(), b.upstream_id());
    assert_ne!(a.source_id(), b.source_id());
    assert_ne!(a.id(), b.id());
}

#[test]
fn telegram_source_constructor_deserialization_and_mutation_validate_the_channel() {
    let original = source();
    for url in [
        "https://other.example/cdo_club",
        "https://t.me/s/cdo_club",
        "http://t.me/cdo_club",
    ] {
        assert!(original.with_url(Url::parse(url).unwrap()).is_err());
        let mut raw = serde_json::to_value(&original).unwrap();
        raw["url"] = serde_json::Value::String(url.into());
        assert!(serde_json::from_value::<SourceDefinition>(raw).is_err());
    }
    let roundtrip: SourceDefinition =
        serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    assert_eq!(roundtrip, original);
}

#[test]
fn quoted_photo_url_parentheses_are_preserved_exactly() {
    let html = PHOTO.replace("photo.jpg", "photo(1).jpg");
    let record = parse_page(&source(), "cdo_club", &html)
        .unwrap()
        .0
        .remove(0);
    assert!(record
        .feed_content_html()
        .unwrap()
        .contains("src=\"https://cdn.example/photo(1).jpg\""));
}
