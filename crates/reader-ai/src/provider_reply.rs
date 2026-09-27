use crate::{AiError, ArticleSnapshot, DefinitionResult, ParagraphTranslation, Usage};
use serde_json::Value;

/// Raw bounded provider response retained even when its JSON/finish reason fails.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProviderReply {
    pub status: u16,
    pub body: Vec<u8>,
    pub interrupted: bool,
}
impl ProviderReply {
    pub fn definition_result(
        &self,
        snapshot: &ArticleSnapshot,
    ) -> Result<DefinitionResult, AiError> {
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
    /// Offline validation/replay consumes retained bytes, never another API request.
    pub fn translation_result(&self, source: &str) -> Result<ParagraphTranslation, AiError> {
        if self.interrupted {
            return Err(AiError::Provider);
        }
        crate::provider::check_status(
            http::StatusCode::from_u16(self.status).map_err(|_| AiError::Protocol)?,
        )?;
        let value: Value = serde_json::from_slice(&self.body).map_err(|_| AiError::Protocol)?;
        let choices = value["choices"]
            .as_array()
            .filter(|items| items.len() == 1)
            .ok_or(AiError::Protocol)?;
        if choices[0]["finish_reason"] != "stop" {
            return Err(AiError::Context);
        }
        ParagraphTranslation::from_response(
            source,
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
