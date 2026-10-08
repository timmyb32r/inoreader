use super::publication_clock;
use reader_core::{SourceId, SourceRecordId};
use reader_ingest::SourceRecord;
fn record(date: Option<&str>) -> SourceRecord {
    SourceRecord::from_parsed(
        SourceRecordId::new(),
        SourceId::new(),
        reader_collectors::ParsedRecord {
            upstream_id: "clock".into(),
            title: "Clock".into(),
            description: None,
            original_url: "https://example.com/clock".into(),
            absolute_url: Some(url::Url::parse("https://example.com/clock").unwrap()),
            categories: None,
            description_media_type: None,
            content_html: None,
            published_at: date.map(|value| value.to_owned().try_into().unwrap()),
        },
    )
    .unwrap()
}
#[test]
fn publication_sort_projection_preserves_source_dates_and_nanoseconds() {
    for (raw, expected) in [
        (
            "2026-10-05T02:00:00.123456789+03:00",
            "2026-10-04T23:00:00.123456789Z",
        ),
        (
            "2026-10-05T02:00:00.123456789",
            "2026-10-05T02:00:00.123456789Z",
        ),
        ("2026-10-05", "2026-10-05T00:00:00.000000000Z"),
    ] {
        let record = record(Some(raw));
        assert_eq!(publication_clock(&record).as_deref(), Some(expected));
        assert_eq!(record.published_at().unwrap().as_str(), raw);
    }
    assert_eq!(publication_clock(&record(None)), None);
}
