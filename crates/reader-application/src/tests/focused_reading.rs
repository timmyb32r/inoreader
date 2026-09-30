use super::*;
#[test]
fn ratings_are_valid_at_every_construction_boundary() {
    for n in 0..=255u8 {
        assert_eq!(ArticleRating::try_from(n).is_ok(), (1..=10).contains(&n));
        assert_eq!(
            serde_json::from_str::<ArticleRating>(&n.to_string()).is_ok(),
            (1..=10).contains(&n)
        );
    }
    for invalid in ["null", "1.5", "\"8\"", "-1", "{}"] {
        assert!(serde_json::from_str::<ArticleRating>(invalid).is_err());
    }
    let rating = ArticleRating::try_from(8).unwrap();
    assert_eq!(serde_json::to_string(&rating).unwrap(), "8");
    assert_eq!(u8::from(rating), 8);
}
#[test]
fn revisions_are_lossless_and_reject_coercion() {
    for value in [0, 9007199254740993, i64::MAX as u64] {
        let revision = ReadingRevision::new(value).unwrap();
        let json = serde_json::to_string(&revision).unwrap();
        assert_eq!(
            serde_json::from_str::<ReadingRevision>(&json).unwrap(),
            revision
        );
    }
    assert!(ReadingRevision::new(u64::MAX).is_err());
    for invalid in [
        "1",
        "null",
        "\"01\"",
        "\"+1\"",
        "\"1.0\"",
        "\"9223372036854775808\"",
    ] {
        assert!(serde_json::from_str::<ReadingRevision>(invalid).is_err());
    }
}
#[test]
fn completion_requires_rating_and_rejects_extra_fields() {
    let mut raw =
        serde_json::json!({"operationId":Uuid::new_v4(),"expectedRevision":"0","rating":7});
    assert!(serde_json::from_value::<CompleteReading>(raw.clone()).is_ok());
    raw.as_object_mut().unwrap().remove("rating");
    assert!(serde_json::from_value::<CompleteReading>(raw.clone()).is_err());
    raw["rating"] = serde_json::json!(7);
    raw["read"] = serde_json::json!(true);
    assert!(serde_json::from_value::<CompleteReading>(raw).is_err());
}

#[test]
fn rating_reason_preserves_authored_text_and_rejects_unstorable_nul() {
    for raw in ["", "  ", "  CDC 中文 🦆\r\nбез замеров  "] {
        let value = RatingReason::try_from(raw.to_owned()).unwrap();
        assert_eq!(value.as_str(), raw);
        let roundtrip: RatingReason =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(roundtrip, value);
        assert_eq!(String::from(value), raw);
    }
    assert!(RatingReason::try_from("a\0b".to_owned()).is_err());
    assert!(serde_json::from_str::<RatingReason>(r#""a\u0000b""#).is_err());
    let mut raw = serde_json::json!({"operationId":Uuid::new_v4(),"expectedRevision":"0","rating":7,"reason":null});
    assert!(serde_json::from_value::<CompleteReading>(raw.clone())
        .unwrap()
        .reason
        .is_none());
    raw["reason"] = serde_json::json!("  exactly as typed\n");
    assert_eq!(
        serde_json::from_value::<CompleteReading>(raw.clone())
            .unwrap()
            .reason
            .unwrap()
            .as_str(),
        "  exactly as typed\n"
    );
    raw["reason"] = serde_json::json!("bad\0text");
    assert!(serde_json::from_value::<CompleteReading>(raw).is_err());
}
