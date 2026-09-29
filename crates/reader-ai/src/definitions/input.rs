use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "InputWire")]
pub struct DefinitionsInput {
    snapshot: ArticleSnapshot,
    model: String,
    system: String,
    version: String,
    limits: InputLimits,
    max_output_tokens: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputWire {
    snapshot: ArticleSnapshot,
    model: String,
    system: String,
    version: String,
    limits: InputLimits,
    max_output_tokens: usize,
}
impl TryFrom<InputWire> for DefinitionsInput {
    type Error = AiError;
    fn try_from(v: InputWire) -> Result<Self, Self::Error> {
        let input = Self {
            snapshot: v.snapshot,
            model: v.model,
            system: v.system,
            version: v.version,
            limits: v.limits,
            max_output_tokens: v.max_output_tokens,
        };
        input.checked_body()?;
        Ok(input)
    }
}
impl DefinitionsInput {
    pub fn budget_cost(&self, rates: &crate::CostRates) -> Result<String, AiError> {
        let body = self.checked_body()?;
        crate::budget::bound_cost(
            body["messages"].as_array().ok_or(AiError::Protocol)?,
            self.limits.framing_tokens_per_message,
            self.limits.framing_tokens_base,
            self.max_output_tokens,
            rates,
        )
    }
    pub fn new(config: &AiConfig, snapshot: ArticleSnapshot) -> Result<Self, AiError> {
        config.validate()?;
        InputWire {
            snapshot,
            model: crate::DeepSeekModel::Flash.id().into(),
            system: SYSTEM.into(),
            version: DEFINITIONS_VERSION.into(),
            limits: InputLimits::from(config),
            max_output_tokens: config.max_output_tokens,
        }
        .try_into()
    }
    pub fn snapshot(&self) -> &ArticleSnapshot {
        &self.snapshot
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn identity(&self) -> Result<String, AiError> {
        serde_json::to_string(self).map_err(|_| AiError::Protocol)
    }
    fn checked_body(&self) -> Result<Value, AiError> {
        let l = &self.limits;
        if self.snapshot.text.trim().is_empty()
            || self.snapshot.source_revision.is_empty()
            || self.model.is_empty()
            || self.system.is_empty()
            || self.version.is_empty()
            || l.context_tokens == 0
            || l.framing_tokens_base == 0
            || l.framing_tokens_per_message == 0
            || l.max_input_bytes == 0
            || l.max_response_bytes == 0
            || self.max_output_tokens == 0
        {
            return Err(AiError::Configuration);
        }
        let content = serde_json::to_string(
            &json!({"title":self.snapshot.title,"article":self.snapshot.text}),
        )
        .map_err(|_| AiError::Protocol)?;
        let bytes = self
            .system
            .len()
            .checked_add(content.len())
            .ok_or(AiError::Context)?;
        let bound = l
            .framing_tokens_per_message
            .checked_mul(2)
            .and_then(|n| n.checked_add(l.framing_tokens_base))
            .and_then(|n| n.checked_add(bytes))
            .and_then(|n| n.checked_add(self.max_output_tokens))
            .ok_or(AiError::Context)?;
        if bytes > l.max_input_bytes || bound > l.context_tokens {
            return Err(AiError::Context);
        }
        Ok(
            json!({"model":self.model,"messages":[{"role":"system","content":self.system},{"role":"user","content":content}],"stream":false,"thinking":{"type":"disabled"},"response_format":{"type":"json_object"},"max_tokens":self.max_output_tokens,"temperature":0}),
        )
    }
    pub(super) fn body(&self) -> Result<Value, AiError> {
        self.checked_body()
    }
}
