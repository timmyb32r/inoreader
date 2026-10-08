use crate::{AiError, CostRates, DecimalRate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// USD amounts are exact decimal strings; days and reset boundaries use Moscow.
/// Unknown provider billing retains the entire reservation, never an assumed zero.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSpending {
    pub daily_limit_usd: String,
    pub today: String,
    pub resets_at: String,
    pub spent_usd: String,
    pub reserved_usd: String,
    pub remaining_usd: String,
    pub days: Vec<SpendingDay>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpendingDay {
    pub day: String,
    pub mode: SpendMode,
    pub spent_usd: String,
    pub reserved_usd: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpendMode {
    Summary,
    Verification,
    Chat,
    Translation,
    Terms,
    Ranking,
}
impl SpendMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Verification => "verification",
            Self::Chat => "chat",
            Self::Translation => "translation",
            Self::Terms => "terms",
            Self::Ranking => "ranking",
        }
    }
}
/// Validated conservative upper bound for one provider request. UTF-8 bytes plus
/// configured framing bound input tokens; all permitted output is reserved.
pub struct SpendReservation {
    amount: String,
    limit: String,
    mode: SpendMode,
}
impl SpendReservation {
    pub fn new(amount: String, limit: String, mode: SpendMode) -> Result<Self, AiError> {
        DecimalRate::parse(&amount)?;
        DecimalRate::parse(&limit)?;
        if !amount.bytes().any(|b| matches!(b, b'1'..=b'9'))
            || !limit.bytes().any(|b| matches!(b, b'1'..=b'9'))
        {
            return Err(AiError::Configuration);
        }
        Ok(Self {
            amount,
            limit,
            mode,
        })
    }
    pub fn amount(&self) -> &str {
        &self.amount
    }
    pub fn limit(&self) -> &str {
        &self.limit
    }
    pub fn mode(&self) -> SpendMode {
        self.mode
    }
}
#[derive(Clone)]
pub struct AutoSummary {
    pub owner: Uuid,
    pub workspace: Uuid,
    pub article: Uuid,
}

pub(crate) fn bound_cost(
    messages: &[serde_json::Value],
    framing: usize,
    base: usize,
    output: usize,
    rates: &CostRates,
) -> Result<String, AiError> {
    let input = messages
        .iter()
        .try_fold(base, |n, m| {
            n.checked_add(framing)?
                .checked_add(m["content"].as_str()?.len())
        })
        .ok_or(AiError::Context)?;
    rates.maximum(
        u64::try_from(input).map_err(|_| AiError::Context)?,
        u64::try_from(output).map_err(|_| AiError::Context)?,
    )
}
