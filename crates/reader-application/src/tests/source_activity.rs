use super::*;
#[test]
fn arbitrary_calendar_ranges_are_validated_at_construction() {
    let start = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let end = chrono::NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
    let period = SourceActivityPeriod::new(start, end).unwrap();
    assert_eq!(period.start(), start);
    assert_eq!(period.end(), end);
    assert!(SourceActivityPeriod::new(start, start).is_ok());
    assert!(SourceActivityPeriod::new(end, start).is_err());
    assert!(SourceActivityPeriod::new(start, chrono::NaiveDate::MAX).is_err());
}
