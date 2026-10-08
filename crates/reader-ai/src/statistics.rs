use crate::{AiError, SpendMode};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatisticsBucket {
    Day,
    Month,
    Year,
}
impl StatisticsBucket {
    pub fn name(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}
/// Inclusive Moscow billing dates; reversed ranges are rejected before querying.
/// Aggregation never truncates the requested interval. This is read-only and
/// owner-scoped; API access additionally requires the authenticated timmyb32r account.
pub struct StatisticsRange {
    from: NaiveDate,
    until: NaiveDate,
    bucket: StatisticsBucket,
}
impl StatisticsRange {
    pub fn new(
        from: NaiveDate,
        until: NaiveDate,
        bucket: StatisticsBucket,
    ) -> Result<Self, AiError> {
        if from > until {
            return Err(AiError::StatisticsRange);
        }
        Ok(Self {
            from,
            until,
            bucket,
        })
    }
    pub fn from(&self) -> NaiveDate {
        self.from
    }
    pub fn until(&self) -> NaiveDate {
        self.until
    }
    pub fn bucket(&self) -> StatisticsBucket {
        self.bucket
    }
}
/// One durable paid admission is one attempt, including retries and unknown
/// billing. It is not a count of articles or proof of receipt by the provider.
/// USD remains exact decimal text. Rows with unknown billing retain reservations.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequestStatistics {
    pub from: String,
    pub until: String,
    pub bucket: StatisticsBucket,
    pub timezone: String,
    pub rows: Vec<AiRequestStatisticsRow>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequestStatisticsRow {
    pub period: String,
    pub mode: SpendMode,
    pub requests: i64,
    pub unconfirmed: i64,
    pub spent_usd: String,
    pub reserved_usd: String,
}
