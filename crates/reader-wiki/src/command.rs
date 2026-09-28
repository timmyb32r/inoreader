use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw configuration only. Limits are bytes (UTF-8), counts, and milliseconds.
/// Construct Limits before serving requests; no deserialization bypass exists.
/// Indexed names have a 2000-byte capacity (below PostgreSQL 8KiB B-tree tuple
/// capacity including namespace UUID). Draft timers fit browser signed i32.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LimitsInput {
    pub name_bytes: usize,
    pub markdown_bytes: usize,
    pub search_bytes: usize,
    pub search_excerpt_characters: u32,
    pub page_size: usize,
    pub draft_save_delay_ms: u64,
}
#[derive(Clone, Debug)]
pub struct Limits(LimitsInput);
impl TryFrom<LimitsInput> for Limits {
    type Error = Error;
    fn try_from(v: LimitsInput) -> Result<Self, Error> {
        if v.name_bytes == 0
            || v.markdown_bytes == 0
            || v.search_bytes == 0
            || v.search_excerpt_characters == 0
            || v.search_excerpt_characters > i32::MAX as u32
            || v.page_size == 0
            || v.page_size >= i32::MAX as usize
            || v.draft_save_delay_ms == 0
            || v.draft_save_delay_ms > i32::MAX as u64
            || v.name_bytes > 2000
        {
            return Err(Error::Invalid(
                "wiki limits must be positive; name_bytes must not exceed the 2000-byte indexed-name capacity; page_size and draft_save_delay_ms must fit signed 32-bit integers".into(),
            ));
        }
        Ok(Self(v))
    }
}
impl Limits {
    pub fn input(&self) -> &LimitsInput {
        &self.0
    }
    pub fn name(&self, value: &str) -> Result<(), Error> {
        if value.is_empty()
            || value.len() > self.0.name_bytes
            || value.contains(['\n', '\r', '[', ']', '\0'])
        {
            return Err(Error::Invalid(
                "name is empty, too long or contains a line break, bracket or NUL".into(),
            ));
        }
        Ok(())
    }
    pub fn markdown(&self, value: &str) -> Result<(), Error> {
        if value.len() > self.0.markdown_bytes || value.contains('\0') {
            return Err(Error::Invalid(
                "Markdown exceeds configured byte limit or contains NUL".into(),
            ));
        }
        Ok(())
    }
    pub fn search(&self, value: &str) -> Result<(), Error> {
        if value.len() > self.0.search_bytes || value.contains('\0') {
            return Err(Error::Invalid(
                "search exceeds configured byte limit or contains NUL".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChangeInput {
    Save { name: String, markdown: String },
    Rename { name: String },
    Trash,
    Restore,
    RestoreRevision { revision: Uuid },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WriteInput {
    pub operation: Uuid,
    pub page: Uuid,
    pub expected_revision: Option<Uuid>,
    pub change: ChangeInput,
}
/// A validated write preserves exact text. None revision means create-only;
/// existing pages always require the exact revision, including trash/restore.
#[derive(Clone, Debug)]
pub struct Write(WriteInput);
impl Write {
    pub fn new(input: WriteInput, limits: &Limits) -> Result<Self, Error> {
        match &input.change {
            ChangeInput::Save { name, markdown } => {
                limits.name(name)?;
                limits.markdown(markdown)?;
            }
            ChangeInput::Rename { name } => limits.name(name)?,
            _ => (),
        }
        if input.expected_revision.is_none() && !matches!(input.change, ChangeInput::Save { .. }) {
            return Err(Error::Invalid("existing page revision required".into()));
        }
        Ok(Self(input))
    }
    pub fn input(&self) -> &WriteInput {
        &self.0
    }
}
/// Target names remain exact; this is a rebuildable link projection, never a
/// page-creation operation. Duplicate links do not create duplicate identities.
pub fn link_names(markdown: &str) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut rest = markdown;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break };
        let name = &rest[..end];
        if !name.is_empty() && !name.contains(['[', ']', '\n', '\r', '\0']) {
            names.insert(name.to_owned());
        }
        rest = &rest[end + 2..];
    }
    names.into_iter().collect()
}
