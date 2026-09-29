use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Editor,
    Reader,
}
impl Role {
    pub fn can_edit(self) -> bool {
        matches!(self, Self::Owner | Self::Editor)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Namespace {
    pub id: Uuid,
    pub name: String,
    pub role: Role,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Page {
    pub parent: Option<Uuid>,
    pub namespace: Uuid,
    pub id: Uuid,
    pub revision: Uuid,
    pub name: String,
    pub markdown: String,
    pub deleted: bool,
    pub author: Uuid,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PageSummary {
    pub excerpt: String,
    pub excerpt_truncated: bool,
    pub id: Uuid,
    pub name: String,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PageList {
    pub items: Vec<PageSummary>,
    pub has_more: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NamespaceList {
    pub items: Vec<Namespace>,
    pub has_more: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Revision {
    pub page: Page,
    pub action: String,
    pub author_name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RevisionList {
    pub items: Vec<RevisionSummary>,
    pub has_more: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Member {
    pub account: Uuid,
    pub username: String,
    pub role: Role,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MemberList {
    pub items: Vec<Member>,
    pub has_more: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Draft {
    pub revision: Option<Uuid>,
    pub id: Uuid,
    pub page: Option<Uuid>,
    pub base_revision: Option<Uuid>,
    pub name: String,
    pub markdown: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Binding {
    pub linked: bool,
    pub page: Option<PageSummary>,
    pub namespace: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Link {
    pub name: String,
    pub page: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RevisionSummary {
    pub revision: Uuid,
    pub name: String,
    pub author: Uuid,
    pub author_name: String,
    pub created_at: DateTime<Utc>,
    pub action: String,
}

/// Namespace-local, actor-owned organization. Deleted pages are excluded from
/// collections; hierarchy edges remain intact through trash/restore.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Collection {
    Favorites,
    Standalone,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Organization {
    pub parent: Option<PageSummary>,
    pub children: PageList,
    pub favorite: bool,
}
