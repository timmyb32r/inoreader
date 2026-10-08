//! Source-declared video metadata. Unknown is not false; duration does not prove
//! that a YouTube video is a Short (orientation is also required).
use serde::{Deserialize, Serialize};
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoMetadata {
    pub duration_seconds: Option<u64>,
    pub shorts: Option<bool>,
}
