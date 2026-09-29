use serde::Serialize;

/// Inclusive calendar dates in the separately validated IANA timezone.
/// Construction rejects reversed ranges and dates whose exclusive end overflows.
#[derive(Clone, Copy, Debug)]
pub struct SourceActivityPeriod {
    start: chrono::NaiveDate,
    end: chrono::NaiveDate,
}
impl SourceActivityPeriod {
    pub fn new(start: chrono::NaiveDate, end: chrono::NaiveDate) -> Result<Self, &'static str> {
        if start > end || end.succ_opt().is_none() {
            return Err("invalid source activity date range");
        }
        Ok(Self { start, end })
    }
    pub fn start(self) -> chrono::NaiveDate {
        self.start
    }
    pub fn end(self) -> chrono::NaiveDate {
        self.end
    }
}
/// First arrivals, attributed once to each distinct source, including removed
/// subscriptions. An article with no provenance appears in the explicit null bucket.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceActivityCount {
    pub subscription_id: Option<String>,
    pub name: String,
    pub present: bool,
    pub count: u32,
}
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct SourceActivityDay {
    pub day: String,
    /// Unique articles, before multi-source attribution.
    pub total: u32,
    pub sources: Vec<SourceActivityCount>,
}

#[cfg(test)]
#[path = "tests/source_activity.rs"]
mod tests;
