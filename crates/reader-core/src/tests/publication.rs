use super::*;

#[test]
fn dates_keep_precision_and_timezone_without_inventing_time() {
    for value in [
        "2026-09-27",
        "2026-09-27T10:03:02.123456789+03:00",
        "2026-09-27T10:03:02",
    ] {
        let date = PublicationDate::try_from(value.to_owned()).unwrap();
        assert_eq!(date.as_str(), value);
        assert_eq!(
            serde_json::from_str::<PublicationDate>(&serde_json::to_string(&date).unwrap())
                .unwrap(),
            date
        );
    }
    assert_eq!(
        PublicationDate::parse("September 27, 2026")
            .unwrap()
            .as_str(),
        "2026-09-27"
    );
    for invalid in ["2026-02-30", "today", "09/10/2026", "2026-09-27T99:00:00"] {
        assert!(PublicationDate::parse(invalid).is_none());
        assert!(serde_json::from_str::<PublicationDate>(&format!("\"{invalid}\"")).is_err());
    }
}

#[test]
fn conflicts_are_not_silently_resolved() {
    let evidence = |raw: &str| PublicationEvidence {
        source: "test".into(),
        raw: raw.into(),
    };
    assert!(resolve_publication(&[evidence("2026-01-01"), evidence("2026-01-02")]).is_none());
    assert!(resolve_publication(&[
        evidence("2026-01-01T03:00:00+03:00"),
        evidence("2026-01-01T00:00:00Z")
    ])
    .is_some());
    assert!(resolve_publication(&[evidence("not a date")]).is_none());
    assert_eq!(
        resolve_publication(&[evidence("2026-01-01"), evidence("2026-01-01T12:00:00Z")])
            .unwrap()
            .as_str(),
        "2026-01-01T12:00:00Z"
    );
}
