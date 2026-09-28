use super::*;
use chrono::{Duration, TimeZone, Utc};

#[test]
fn supports_delta_seconds_and_http_date_without_shortening() {
    let now = Utc.with_ymd_and_hms(2026, 9, 28, 12, 0, 0).unwrap();
    assert_eq!(
        retry_after("120", now).unwrap(),
        now + Duration::seconds(120)
    );
    assert_eq!(
        retry_after("Mon, 28 Sep 2026 12:02:00 GMT", now).unwrap(),
        now + Duration::seconds(120)
    );
    assert_eq!(retry_after("0", now).unwrap(), now);
}

#[test]
fn invalid_or_overflowing_cooldowns_fail_explicitly() {
    for value in [
        "",
        "-1",
        "NaN",
        "999999999999999999999999999",
        "9223372036854775807",
    ] {
        assert!(retry_after(value, Utc::now()).is_err(), "{value}");
    }
}

#[test]
fn accepts_legacy_http_date_wire_formats() {
    let now = Utc.with_ymd_and_hms(2026, 9, 28, 12, 0, 0).unwrap();
    for value in ["Monday, 28-Sep-26 12:02:00 GMT", "Mon Sep 28 12:02:00 2026"] {
        assert_eq!(
            retry_after(value, now).unwrap(),
            now + Duration::seconds(120)
        );
    }
}
