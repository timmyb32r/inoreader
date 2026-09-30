//! Account model choices are read immediately before each paid chat request.
//! An admitted call pins its model and tariff; later preference edits cannot
//! change its billing. Missing preferences mean Flash for both stages.
use crate::CostRates;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum DeepSeekModel {
    #[default]
    #[serde(rename = "deepseek-flash")]
    Flash,
    #[serde(rename = "deepseek-v4-pro")]
    Pro,
}
impl DeepSeekModel {
    pub fn id(self) -> &'static str {
        match self {
            Self::Flash => "deepseek-flash",
            Self::Pro => "deepseek-v4-pro",
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelPreferences {
    pub summary: DeepSeekModel,
    /// Explicit null disables the second request. Omission is invalid. Changes
    /// apply at phase admission; an already admitted checker may finish.
    #[cfg_attr(
        feature = "schema",
        schemars(required, schema_with = "verification_schema")
    )]
    #[serde(deserialize_with = "Option::deserialize")]
    pub verification: Option<DeepSeekModel>,
}

impl Default for ModelPreferences {
    fn default() -> Self {
        Self {
            summary: DeepSeekModel::Flash,
            verification: Some(DeepSeekModel::Flash),
        }
    }
}

/// Both tariffs are mandatory, valid-by-construction CostRates. No unknown
/// model or missing tariff can fall back to another model's price.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRates {
    pub flash: CostRates,
    pub pro: CostRates,
}
impl ModelRates {
    pub fn get(&self, model: DeepSeekModel) -> &CostRates {
        match model {
            DeepSeekModel::Flash => &self.flash,
            DeepSeekModel::Pro => &self.pro,
        }
    }
}

/// Immutable per-request selection, persisted atomically with the budget hold.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallModel {
    model: DeepSeekModel,
    rates: CostRates,
}
impl CallModel {
    pub fn new(model: DeepSeekModel, rates: CostRates) -> Self {
        Self { model, rates }
    }
    pub fn model(&self) -> DeepSeekModel {
        self.model
    }
    pub fn rates(&self) -> &CostRates {
        &self.rates
    }
}

#[cfg(feature = "schema")]
fn verification_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<DeepSeekModel>>()
}
