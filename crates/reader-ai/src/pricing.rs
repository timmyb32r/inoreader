use serde::{Deserialize, Serialize};

use crate::{AiError, DecimalRate, Usage};

/// Immutable USD-per-million-token tariff snapshot for a conversation's model.
/// Construction and deserialization preserve the authored decimal strings and
/// reject negative, malformed or unsupported precision/ranges. Checked decimal
/// arithmetic must cover every supported u64 usage count, without rounding.
/// This estimates usage at the saved tariffs; it does not assert the provider's
/// current prices or replace the provider account balance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawCostRates", into = "RawCostRates")]
pub struct CostRates(RawCostRates);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCostRates {
    input_usd_per_million_tokens: String,

    cached_input_usd_per_million_tokens: String,

    output_usd_per_million_tokens: String,
}

impl CostRates {
    pub fn new(input: String, cached_input: String, output: String) -> Result<Self, AiError> {
        Self::try_from(RawCostRates {
            input_usd_per_million_tokens: input,
            cached_input_usd_per_million_tokens: cached_input,
            output_usd_per_million_tokens: output,
        })
    }

    pub fn cost(&self, usage: &Usage) -> Result<String, AiError> {
        DecimalRate::cost(&[
            (
                &self.0.input_usd_per_million_tokens,
                usage.prompt_cache_miss_tokens,
            ),
            (
                &self.0.cached_input_usd_per_million_tokens,
                usage.prompt_cache_hit_tokens,
            ),
            (
                &self.0.output_usd_per_million_tokens,
                usage.completion_tokens,
            ),
        ])
    }
}

impl TryFrom<RawCostRates> for CostRates {
    type Error = AiError;

    fn try_from(raw: RawCostRates) -> Result<Self, Self::Error> {
        DecimalRate::cost(&[
            (&raw.input_usd_per_million_tokens, u64::MAX),
            (&raw.cached_input_usd_per_million_tokens, u64::MAX),
            (&raw.output_usd_per_million_tokens, u64::MAX),
        ])?;
        Ok(Self(raw))
    }
}

impl From<CostRates> for RawCostRates {
    fn from(value: CostRates) -> Self {
        value.0
    }
}
