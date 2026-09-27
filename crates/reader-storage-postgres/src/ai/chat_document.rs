use super::*;
use serde_json::{Map, Value};

const INPUT_FIELDS: &[&str] = &[
    "snapshot",
    "system_prompt",
    "generation_mode",
    "cost_rates",
    "max_output_tokens",
    "limits",
    "review",
];

/// Mutable progress and pinned provider inputs have separate TOAST values.
/// Ordinary progress never rewrites the large article/prompt payload. Snapshot
/// resolution is the only permitted input mutation: absent -> pinned exactly once.
pub(super) fn encode(record: &ChatRecord) -> Result<(String, String), AiError> {
    let Value::Object(mut progress) = serde_json::to_value(record).map_err(storage)? else {
        return Err(AiError::Storage);
    };
    let mut inputs = Map::new();
    for &field in INPUT_FIELDS {
        inputs.insert(
            field.into(),
            progress.remove(field).ok_or(AiError::Storage)?,
        );
    }
    Ok((super::encode(&progress)?, super::encode(&inputs)?))
}

/// SQL returns a pair of opaque JSON strings, so malformed stored JSON reaches
/// the quarantine boundary instead of aborting the entire queue query in SQL.
pub(super) fn decode(pair: &str) -> Result<ChatRecord, AiError> {
    let (progress, inputs): (String, String) = super::decode(pair)?;
    let mut progress: Map<String, Value> = super::decode(&progress)?;
    let inputs: Map<String, Value> = super::decode(&inputs)?;
    if inputs.len() != INPUT_FIELDS.len()
        || INPUT_FIELDS
            .iter()
            .any(|field| !inputs.contains_key(*field) || progress.contains_key(*field))
    {
        return Err(AiError::Storage);
    }
    progress.extend(inputs);
    serde_json::from_value(Value::Object(progress)).map_err(storage)
}
