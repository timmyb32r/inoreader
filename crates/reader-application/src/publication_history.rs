use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Publication frequency among the articles delivered to one owned subscription.
/// Buckets are UTC calendar days, sorted ascending; each logical article counts
/// once. Missing dates and contradictory publication days are reported separately
/// instead of substituting an ingestion date or arbitrarily choosing a date.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationHistory {
    pub days: Vec<PublicationDay>,
    pub undated: u64,
    pub conflicting: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicationDay {
    pub date: NaiveDate,
    pub count: u64,
}
