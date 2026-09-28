//! Wiki wire contracts. Domain values are shared through schema generation.
pub use reader_wiki::{
    Binding as WikiBinding, Draft as WikiDraft, LimitsInput as WikiLimits,
    MemberList as WikiMembers, Namespace as WikiNamespace, NamespaceList as WikiNamespaces,
    Page as WikiPage, PageList as WikiPages, RevisionList as WikiHistory, Role as WikiRole,
    WriteInput as WikiWrite,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiCreateNamespace {
    pub id: Uuid,
    pub name: String,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiMemberCommand {
    pub username: String,
    pub role: Option<WikiRole>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiBindCommand {
    pub target: Option<WikiTarget>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiTarget {
    pub namespace: Uuid,
    pub page: Uuid,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct WikiDraftResponse {
    pub draft: Option<WikiDraft>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WikiDiscardDraft {
    pub revision: Option<Uuid>,
}

pub use reader_wiki::Link as WikiLink;

pub use reader_wiki::Revision as WikiRevision;
