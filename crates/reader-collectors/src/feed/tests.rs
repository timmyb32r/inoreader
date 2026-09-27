use super::*;

#[test]
fn parses_relative_json_feed_url_without_rewriting_query_or_fragment() {
    let records = parse_json(
        br#"{"items":[{"id":"1","url":"/a?q=1#x","title":"T","summary":""}]}"#,
        &Url::parse("https://example.test/feed").unwrap(),
    )
    .unwrap();
    assert_eq!(
        records[0].absolute_url.as_ref().unwrap().as_str(),
        "https://example.test/a?q=1#x"
    );
    assert_eq!(records[0].description.as_deref(), Some(""));
}

#[test]
fn preserves_url_less_entry_when_stable_identity_exists() {
    let records = parse_json(
        br#"{"items":[{"id":"stable","title":"T"}]}"#,
        &Url::parse("https://example.test/feed").unwrap(),
    )
    .unwrap();
    assert_eq!(records[0].upstream_id, "stable");
    assert_eq!(records[0].absolute_url, None);
}

#[test]
fn declared_summary_format_and_original_text_survive_parsing() {
    let url = Url::parse("https://example.test/feed").unwrap();
    for (kind, value, expected) in [
        ("text", "A &lt;b&gt;literal&lt;/b&gt;", "text/plain"),
        (
            "html",
            "&lt;p&gt;A &amp;amp; &lt;b&gt;B&lt;/b&gt;&lt;/p&gt;",
            "text/html",
        ),
    ] {
        let xml = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><id>f</id><title>f</title><updated>2026-01-01T00:00:00Z</updated><entry><id>a</id><title>A</title><summary type="{kind}">{value}</summary></entry></feed>"#
        );
        let record = parse_xml(xml.as_bytes(), &url).unwrap().remove(0);
        assert_eq!(record.description_media_type.as_deref(), Some(expected));
        assert!(record.description.unwrap().contains("<b>"));
    }
    let rss = br#"<rss version="2.0"><channel><title>f</title><link>https://example.test</link><description>f</description><item><guid>a</guid><title>A</title><description><![CDATA[<p>A <b>B</b></p>]]></description></item></channel></rss>"#;
    let record = parse_xml(rss, &url).unwrap().remove(0);
    assert_eq!(record.description_media_type.as_deref(), Some("text/html"));
    assert_eq!(record.description.as_deref(), Some("<p>A <b>B</b></p>"));
    let record = parse_json(
        br#"{"items":[{"id":"a","summary":"A <b>literal</b>"}]}"#,
        &url,
    )
    .unwrap()
    .remove(0);
    assert_eq!(record.description_media_type.as_deref(), Some("text/plain"));
    assert_eq!(record.description.as_deref(), Some("A <b>literal</b>"));
}
