//! Search is a read-only projection. Queries are literal, case-insensitive text;
//! no source text or identifiers are rewritten. Caller identity never comes from input.
use crate::RepositoryError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchLimitsInput {
    /// Maximum exact UTF-8 query size in bytes; no normalization or truncation.
    pub query_bytes: usize,
    /// Number of hits per page; one extra row determines has_more.
    pub page_size: u32,
    /// Unicode scalar values in the derived excerpt; source text is untouched.
    pub excerpt_characters: u32,
}
#[derive(Clone, Debug)]
pub struct SearchLimits(SearchLimitsInput);
impl SearchLimits {
    pub fn new(raw: SearchLimitsInput) -> Result<Self, &'static str> {
        if raw.query_bytes == 0
            || raw.page_size == 0
            || raw.page_size >= i32::MAX as u32
            || raw.excerpt_characters == 0
            || raw.excerpt_characters > i32::MAX as u32
        {
            return Err("invalid search limits");
        }
        Ok(Self(raw))
    }
    pub fn input(&self) -> &SearchLimitsInput {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    All,
    News,
    Wiki,
}
#[derive(Clone, Debug)]
pub struct SearchRequest {
    query: String,
    kind: SearchKind,
    namespace: Option<Uuid>,
    subscription: Option<Uuid>,
    offset: u32,
}
impl SearchRequest {
    pub fn new(
        limits: &SearchLimits,
        query: String,
        kind: SearchKind,
        namespace: Option<Uuid>,
        subscription: Option<Uuid>,
        offset: u32,
    ) -> Result<Self, &'static str> {
        if query.len() > limits.0.query_bytes || query.contains('\0') {
            return Err("search query exceeds configured limit or contains NUL");
        }
        if namespace.is_some() && (kind != SearchKind::Wiki || subscription.is_some())
            || subscription.is_some() && kind != SearchKind::News
        {
            return Err("search scope contradicts selected kind");
        }
        Ok(Self {
            query,
            kind,
            namespace,
            subscription,
            offset,
        })
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn kind(&self) -> SearchKind {
        self.kind
    }
    pub fn namespace(&self) -> Option<Uuid> {
        self.namespace
    }
    pub fn subscription(&self) -> Option<Uuid> {
        self.subscription
    }
    pub fn offset(&self) -> u32 {
        self.offset
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SearchTarget {
    News { workspace: Uuid, article: Uuid },
    Wiki { namespace: Uuid, page: Uuid },
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchHit {
    pub target: SearchTarget,
    pub title: String,
    pub context: String,
    pub excerpt: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchPage {
    pub items: Vec<SearchHit>,
    pub has_more: bool,
}
#[async_trait]
pub trait SearchPort: Send + Sync {
    async fn search(
        &self,
        owner: Uuid,
        request: SearchRequest,
    ) -> Result<SearchPage, RepositoryError>;
}
#[cfg(test)]
#[path = "tests/search.rs"]
mod tests;
