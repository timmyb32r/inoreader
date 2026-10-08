use crate::{
    built_in_adapters::dropbox::parse, BuiltInAdapter, RecordRevisionEffect, SourceDefinition,
    SourceKind, SourceRecord,
};
use reader_collectors::ParsedRecord;
use reader_core::{SourceId, SourceRecordId};
use url::Url;

fn source() -> SourceDefinition {
    SourceDefinition::new(
        SourceId::new(),
        Url::parse("https://dropbox.tech/feed").unwrap(),
        SourceKind::BuiltIn(BuiltInAdapter::Dropbox),
    )
    .unwrap()
}

#[test]
fn dropbox_archive_covers_all_existing_rss_identities_without_reducing_metadata() {
    let source = source();
    let records = parse(&source, include_bytes!("fixtures/dropbox-archive.html")).unwrap();
    assert_eq!(records.len(), 409);
    let old: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/dropbox-existing.json")).unwrap();
    assert_eq!(old.len(), 10);
    for row in old {
        let identity = row["upstream_id"].as_str().unwrap();
        let next = records
            .iter()
            .find(|r| r.upstream_id() == identity)
            .unwrap()
            .clone();
        let previous = SourceRecord::from_parsed(
            SourceRecordId::new(),
            source.id(),
            ParsedRecord {
                categories: None,
                upstream_id: identity.into(),
                original_url: identity.into(),
                absolute_url: Some(Url::parse(identity).unwrap()),
                title: row["title"].as_str().unwrap().into(),
                description: Some("<p>Original RSS summary &amp; details</p>".into()),
                description_media_type: Some("text/html".into()),
                content_html: Some("<article>Full RSS content — 原文</article>".into()),
                published_at: Some(
                    reader_core::PublicationDate::parse(row["published_at"].as_str().unwrap())
                        .unwrap(),
                ),
            },
        )
        .unwrap();
        let revision = previous.revise_from_dropbox_listing(next).unwrap();
        assert_eq!(revision.record.id(), previous.id());
        assert_eq!(revision.record.source_id(), previous.source_id());
        assert_eq!(revision.record.upstream_id(), previous.upstream_id());
        assert_eq!(revision.record.published_at(), previous.published_at());
        assert_eq!(
            revision.record.feed_content_html(),
            previous.feed_content_html()
        );
        assert_eq!(
            revision.record.description_media_type(),
            previous.description_media_type()
        );
        assert_eq!(revision.record.key(), previous.key());
        assert_eq!(revision.effect, RecordRevisionEffect::Unchanged);
    }
}

#[test]
fn dropbox_rejects_missing_fields_duplicate_identities_and_conflicting_dates() {
    let source = source();
    let card = r#"<li class="dr-article-section__list-item"><a data-element-id="article-link" href="https://dropbox.tech/test/article"><span data-element-id="article-title">Title</span></a><span data-element-id="article-date">Sep 23, 2026</span></li>"#;
    assert!(parse(&source, format!("{card}{card}").as_bytes()).is_err());
    assert!(parse(
        &source,
        card.replace("article-date", "missing-date").as_bytes()
    )
    .is_err());
    assert!(parse(&source, card.replace("Sep 23, 2026", "invalid").as_bytes()).is_err());
    assert!(parse(
        &source,
        card.replace("https://dropbox.tech", "https://other.example")
            .as_bytes()
    )
    .is_err());
    assert!(parse(&source, b"<html>Challenge</html>").is_err());
    let current = parse(&source, card.as_bytes()).unwrap().remove(0);
    let next = parse(&source, card.replace("Sep 23", "Sep 24").as_bytes())
        .unwrap()
        .remove(0);
    assert!(current.revise_from_dropbox_listing(next).is_err());
    let foreign = parse(&self::source(), card.as_bytes()).unwrap().remove(0);
    assert!(current.revise_from_dropbox_listing(foreign).is_err());
    let roundtrip: SourceRecord =
        serde_json::from_value(serde_json::to_value(&current).unwrap()).unwrap();
    assert_eq!(
        roundtrip
            .revise_from_dropbox_listing(current)
            .unwrap()
            .effect,
        RecordRevisionEffect::Unchanged
    );
}
