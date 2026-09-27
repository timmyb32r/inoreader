use serde::{Deserialize, Serialize};

use crate::AiError;

/// Provider generation parameters are mutually exclusive. Standard mode uses a
/// validated temperature; thinking mode has only a supported reasoning effort.
/// This same representation is frozen in each conversation, so configuration
/// changes cannot alter the parameters of subsequent turns or explicit retries.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GenerationMode {
    Standard { temperature: Temperature },
    Thinking { effort: ReasoningEffort },
}

impl GenerationMode {
    pub fn standard(temperature: f64) -> Result<Self, AiError> {
        Ok(Self::Standard {
            temperature: Temperature::try_from(temperature)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    High,
    Max,
}

/// Finite provider temperature in the inclusive range 0..=2. Both construction
/// and deserialization enforce the range; no public mutation can bypass it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Temperature(f64);

impl Temperature {
    pub fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Temperature {
    type Error = AiError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() || !(0.0..=2.0).contains(&value) {
            return Err(AiError::Configuration);
        }
        Ok(Self(value))
    }
}

impl From<Temperature> for f64 {
    fn from(value: Temperature) -> Self {
        value.0
    }
}
