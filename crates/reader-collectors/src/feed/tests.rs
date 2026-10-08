use super::*;

#[test]
fn publication_is_not_updated_or_ingestion_time() {
    let url = Url::parse("https://example.test/feed").unwrap();
    let xml = br#"<feed xmlns="http://www.w3.org/2005/Atom"><id>f</id><title>f</title><updated>2026-01-01T00:00:00Z</updated><entry><id>a</id><title>A</title><published>2020-01-02T03:04:05Z</published><updated>2026-01-01T00:00:00Z</updated></entry><entry><id>b</id><title>B</title><updated>2026-01-01T00:00:00Z</updated></entry></feed>"#;
    let values = parse_xml(xml, &url).unwrap();
    assert_eq!(
        values[0].published_at.as_ref().unwrap().as_str(),
        "2020-01-02T03:04:05+00:00"
    );
    assert!(values[1].published_at.is_none());
    let json = br#"{"items":[{"id":"a","date_published":"2020-01-02T03:04:05Z","date_modified":"2026-01-01T00:00:00Z"}]}"#;
    assert_eq!(
        parse_json(json, &url).unwrap()[0]
            .published_at
            .as_ref()
            .unwrap()
            .as_str(),
        "2020-01-02T03:04:05+00:00"
    );
}

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

#[test]
fn atom_alternate_html_link_wins_over_comments_and_self() {
    let xml = br#"<feed xmlns="http://www.w3.org/2005/Atom"><id>blog</id><title>Blog</title><updated>2026-10-04T00:00:00Z</updated><entry><id>post</id><title>Post</title><updated>2026-10-04T00:00:00Z</updated><link rel="replies" type="application/atom+xml" href="https://example.com/comments"/><link rel="self" href="https://example.com/feed/post"/><link rel="alternate" type="text/html" href="https://example.com/2026/post.html"/></entry></feed>"#;
    let values = parse_xml(xml, &Url::parse("https://example.com/feed").unwrap()).unwrap();
    assert_eq!(values[0].original_url, "https://example.com/2026/post.html");
}
