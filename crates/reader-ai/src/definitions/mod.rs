use crate::{AiConfig, AiError, ArticleSnapshot, CostRates, InputLimits, Usage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

mod input;
mod provider;
mod record;
pub use input::*;
pub use record::*;
pub const DEFINITIONS_VERSION: &str = "reading-data-news-definitions-v2";
const SYSTEM: &str = include_str!("../../../../prompts/reading-data-news/definitions/system.md");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Company,
    Product,
    Technology,
    Abbreviation,
    Protocol,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "EntityWire", rename_all = "camelCase")]
pub struct EntityDefinition {
    name: String,
    kind: EntityKind,
    explanation: String,
    insufficient_context: bool,
}
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

/// Raw bounded provider response retained even when its JSON/finish reason fails.
pub struct DefinitionReply {
    pub status: u16,
    pub body: Vec<u8>,
    pub interrupted: bool,
}
impl DefinitionReply {
    pub fn result(&self, snapshot: &ArticleSnapshot) -> Result<DefinitionResult, AiError> {
        if self.interrupted {
            return Err(AiError::Provider);
        }
        crate::provider::check_status(
            http::StatusCode::from_u16(self.status).map_err(|_| AiError::Protocol)?,
        )?;
        let value: Value = serde_json::from_slice(&self.body).map_err(|_| AiError::Protocol)?;
        let choices = value["choices"]
            .as_array()
            .filter(|v| v.len() == 1)
            .ok_or(AiError::Protocol)?;
        if choices[0]["finish_reason"] != "stop" {
            return Err(AiError::Context);
        }
        DefinitionResult::from_response(
            snapshot,
            choices[0]["message"]["content"]
                .as_str()
                .ok_or(AiError::Protocol)?,
        )
    }
    pub fn usage(&self) -> Option<Usage> {
        let v: Value = serde_json::from_slice(&self.body).ok()?;
        let v = &v["usage"];
        let prompt_tokens = v["prompt_tokens"].as_u64()?;
        let completion_tokens = v["completion_tokens"].as_u64()?;
        let hit = v["prompt_cache_hit_tokens"].as_u64()?;
        let miss = v["prompt_cache_miss_tokens"].as_u64()?;
        if hit.checked_add(miss)? != prompt_tokens {
            return None;
        }
        Some(Usage {
            prompt_tokens,
            completion_tokens,
            prompt_cache_hit_tokens: hit,
            prompt_cache_miss_tokens: miss,
            estimated_cost_usd: None,
        })
    }
}
