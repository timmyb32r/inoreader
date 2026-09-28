use super::*;

fn feed(items: &str) -> String {
    format!("<rss version=\"2.0\"><channel><title>Feed</title><link>https://example.test</link><description>Feed</description>{items}</channel></rss>")
}
fn item(id: &str, extra: &str) -> String {
    format!("<item><guid isPermaLink=\"false\">{id}</guid><title>{id}</title><link>https://example.test/{id}</link>{extra}</item>")
}
fn url() -> Url {
    Url::parse("https://example.test/feed").unwrap()
}

#[test]
fn explicit_policy_preserves_first_order_and_default_parser_retains_every_item() {
    let a = item("a", "<description>Original text</description>");
    let b = item("b", "");
    let input = feed(&format!("{a}\n  {b}\n{a}{b}"));
    assert_eq!(parse_xml(input.as_bytes(), &url()).unwrap().len(), 4);
    let (records, repeated) = parse_rss_coalescing_identical(input.as_bytes(), &url()).unwrap();
    assert_eq!(repeated, 2);
    assert_eq!(
        records
            .iter()
            .map(|r| r.upstream_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(records[0].description.as_deref(), Some("Original text"));
}

#[test]
fn differences_in_unprojected_metadata_and_whitespace_fail_closed() {
    for (first, second) in [
        ("<category>A</category>", "<category>B</category>"),
        (
            "<description>text</description>",
            "<description>text </description>",
        ),
        ("", "<!--extra metadata-->"),
    ] {
        let input = feed(&format!("{}{}", item("a", first), item("a", second)));
        assert!(parse_rss_coalescing_identical(input.as_bytes(), &url()).is_err());
    }
}

#[test]
fn distinct_ids_keep_identical_text_and_unsupported_formats_are_rejected() {
    let input = feed(&format!("{}{}", item("a", ""), item("b", "")));
    assert_eq!(
        parse_rss_coalescing_identical(input.as_bytes(), &url())
            .unwrap()
            .0
            .len(),
        2
    );
    for invalid in [
        b"<feed xmlns=\"http://www.w3.org/2005/Atom\"/>".as_slice(),
        b"<rss><channel/><channel/></rss>",
        &[0xff],
    ] {
        assert!(parse_rss_coalescing_identical(invalid, &url()).is_err());
    }
}
