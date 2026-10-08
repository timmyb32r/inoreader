use crate::ProviderReply;
use crate::{AiConfig, AiError, ArticleSnapshot, CostRates, InputLimits, Usage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

mod input;
mod provider;
mod record;
pub use input::*;
pub use record::*;
pub const DEFINITIONS_VERSION: &str = "reading-data-news-definitions-v3";
const SYSTEM: &str = include_str!("../../../../prompts/reading-data-news/definitions/system.md");

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Company,
    Product,
    Technology,
    Abbreviation,
    Protocol,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "EntityWire", rename_all = "camelCase")]
pub struct EntityDefinition {
    name: String,
    kind: EntityKind,
    explanation: String,
    insufficient_context: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EntityWire {
    name: String,
    kind: EntityKind,
    explanation: String,
    insufficient_context: bool,
}
impl TryFrom<EntityWire> for EntityDefinition {
    type Error = AiError;
    fn try_from(v: EntityWire) -> Result<Self, Self::Error> {
        if v.name.trim().is_empty()
            || v.explanation.trim().is_empty()
            || v.name.contains(['\n', '\r'])
        {
            return Err(AiError::Protocol);
        }
        Ok(Self {
            name: v.name,
            kind: v.kind,
            explanation: v.explanation,
            insufficient_context: v.insufficient_context,
        })
    }
}
impl EntityDefinition {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionResult {
    pub entities: Vec<EntityDefinition>,
}
impl DefinitionResult {
    pub fn from_response(snapshot: &ArticleSnapshot, response: &str) -> Result<Self, AiError> {
        let result: Self = serde_json::from_str(response).map_err(|_| AiError::Protocol)?;
        result.check_source(snapshot)?;
        Ok(result)
    }
    fn check_source(&self, snapshot: &ArticleSnapshot) -> Result<(), AiError> {
        let mut names = std::collections::HashSet::new();
        for entity in &self.entities {
            if !names.insert(entity.name())
                || !(snapshot.text.contains(entity.name())
                    || snapshot.title.contains(entity.name()))
            {
                return Err(AiError::Protocol);
            }
        }
        Ok(())
    }
}
