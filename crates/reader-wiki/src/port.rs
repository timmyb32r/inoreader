use crate::*;
use async_trait::async_trait;
use uuid::Uuid;

/// Every operation authorizes actor within its database transaction. Namespace
/// write locks serialize page mutations and membership revocation. No method
/// accepts an administrative bypass or an unscoped page identity.
#[async_trait]
pub trait Store: Send + Sync {
    async fn collection(
        &self,
        actor: Uuid,
        namespace: Uuid,
        kind: Collection,
        offset: u32,
    ) -> Result<PageList, Error>;
    async fn organization(
        &self,
        actor: Uuid,
        namespace: Uuid,
        page: Uuid,
        offset: u32,
    ) -> Result<Organization, Error>;
    async fn favorite(
        &self,
        actor: Uuid,
        namespace: Uuid,
        page: Uuid,
        favorite: bool,
    ) -> Result<(), Error>;
    async fn subscription_root(&self, actor: Uuid, namespace: Uuid) -> Result<Page, Error>;

    async fn revision(
        &self,
        actor: Uuid,
        namespace: Uuid,
        page: Uuid,
        revision: Uuid,
    ) -> Result<Revision, Error>;
    async fn links(&self, actor: Uuid, namespace: Uuid, page: Uuid) -> Result<Vec<Link>, Error>;
    async fn namespaces(&self, actor: Uuid, offset: u32) -> Result<NamespaceList, Error>;
    async fn create_namespace(&self, actor: Uuid, id: Uuid, name: &str)
        -> Result<Namespace, Error>;
    async fn namespace(&self, actor: Uuid, namespace: Uuid) -> Result<Namespace, Error>;
    async fn pages(
        &self,
        actor: Uuid,
        namespace: Uuid,
        search: &str,
        trash: bool,
        offset: u32,
    ) -> Result<PageList, Error>;
    async fn page(&self, actor: Uuid, namespace: Uuid, page: Uuid) -> Result<Page, Error>;
    async fn resolve(&self, actor: Uuid, namespace: Uuid, name: &str) -> Result<Page, Error>;
    async fn write(&self, actor: Uuid, namespace: Uuid, command: Write) -> Result<Page, Error>;
    async fn history(
        &self,
        actor: Uuid,
        namespace: Uuid,
        page: Uuid,
        offset: u32,
    ) -> Result<RevisionList, Error>;
    async fn draft(&self, actor: Uuid, namespace: Uuid, id: Uuid) -> Result<Option<Draft>, Error>;
    async fn save_draft(&self, actor: Uuid, namespace: Uuid, draft: Draft) -> Result<Draft, Error>;
    async fn discard_draft(
        &self,
        actor: Uuid,
        namespace: Uuid,
        id: Uuid,
        revision: Option<Uuid>,
    ) -> Result<(), Error>;
    async fn members(&self, actor: Uuid, namespace: Uuid, offset: u32)
        -> Result<MemberList, Error>;
    async fn set_member(
        &self,
        actor: Uuid,
        namespace: Uuid,
        username: &str,
        role: Option<Role>,
    ) -> Result<(), Error>;
    async fn binding(&self, actor: Uuid, subscription: Uuid) -> Result<Binding, Error>;
    async fn bind(
        &self,
        actor: Uuid,
        subscription: Uuid,
        target: Option<(Uuid, Uuid)>,
    ) -> Result<(), Error>;
}
