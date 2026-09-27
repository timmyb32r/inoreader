use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// An explicitly declared publication date. ISO text retains supplied precision
/// and offset. A day or timezone-less clock is never promoted to a UTC instant.
/// Construction/deserialization rejects invalid dates; there are no setters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PublicationDate(String);

impl TryFrom<String> for PublicationDate {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if DateTime::parse_from_rfc3339(&value).is_ok()
            || NaiveDate::parse_from_str(&value, "%Y-%m-%d").is_ok()
            || NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        {
            Ok(Self(value))
        } else {
            Err("invalid publication date")
        }
    }
}
impl From<PublicationDate> for String {
    fn from(value: PublicationDate) -> Self {
        value.0
    }
}
impl From<DateTime<Utc>> for PublicationDate {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value.to_rfc3339())
    }
}
impl PublicationDate {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Explicit source-date extraction accepts ISO/RFC and unambiguous named
    /// month/year-first dates. Ambiguous numeric month/day order is unsupported.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if let Ok(date) = Self::try_from(value.to_owned()) {
            return Some(date);
        }
        if let Ok(date) = DateTime::parse_from_rfc2822(value) {
            return Some(Self(date.to_rfc3339()));
        }
        for format in [
            "%Y/%m/%d",
            "%B %d, %Y",
            "%b %d, %Y",
            "%d %B %Y",
            "%d %b %Y",
            "%Y年%m月%d日",
        ] {
            if let Ok(date) = NaiveDate::parse_from_str(value, format) {
                return Some(Self(date.to_string()));
            }
        }
        for format in [
            "%Y-%m-%d %H:%M:%S%.f",
            "%Y-%m-%d %H:%M",
            "%Y/%m/%d %H:%M:%S",
        ] {
            if let Ok(date) = NaiveDateTime::parse_from_str(value, format) {
                return Some(Self(date.format("%Y-%m-%dT%H:%M:%S%.f").to_string()));
            }
        }
        None
    }
    /// Calendar views retain the established UTC day for zoned instants; dates
    /// without a timezone retain their declared local calendar day.
    pub fn day(&self) -> NaiveDate {
        if let Ok(value) = DateTime::parse_from_rfc3339(&self.0) {
            value.with_timezone(&Utc).date_naive()
        } else if let Ok(value) = NaiveDateTime::parse_from_str(&self.0, "%Y-%m-%dT%H:%M:%S%.f") {
            value.date()
        } else {
            NaiveDate::parse_from_str(&self.0, "%Y-%m-%d").expect("validated publication date")
        }
    }
    pub fn equivalent(&self, other: &Self) -> bool {
        match (
            DateTime::parse_from_rfc3339(&self.0),
            DateTime::parse_from_rfc3339(&other.0),
        ) {
            (Ok(a), Ok(b)) => a == b,
            _ => self == other,
        }
    }
    fn day_only(&self) -> bool {
        NaiveDate::parse_from_str(&self.0, "%Y-%m-%d").is_ok()
    }
}

/// Evidence is retained verbatim alongside its selector/protocol label. Invalid
/// external dates remain evidence with no parsed value, never ingestion time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublicationEvidence {
    pub source: String,
    pub raw: String,
}
impl PublicationEvidence {
    pub fn date(&self) -> Option<PublicationDate> {
        PublicationDate::parse(&self.raw)
    }
}

/// Equal declarations agree. A day may corroborate an explicitly supplied time
/// on that day; the time is retained. Contradictions remain unresolved. Feed
/// declarations are selected first by the ingest/presentation boundary.
pub fn resolve_publication(values: &[PublicationEvidence]) -> Option<PublicationDate> {
    let dates: Vec<_> = values
        .iter()
        .filter_map(PublicationEvidence::date)
        .collect();
    let first = dates
        .iter()
        .find(|date| !date.day_only())
        .or_else(|| dates.first())?;
    dates
        .iter()
        .all(|value| first.equivalent(value) || (value.day_only() && value.day() == first.day()))
        .then(|| first.clone())
}

#[cfg(test)]
#[path = "tests/publication.rs"]
mod tests;
