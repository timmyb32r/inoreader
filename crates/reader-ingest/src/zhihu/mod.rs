//! Account-owned Zhihu session and public author article collection. Secrets are
//! never source configuration. The store rejects sources shared between owners.
mod collect;
mod cookies;
use crate::{BrowserHttpClient, FetchError, SourceDefinition, SourceRecord};
use async_trait::async_trait;
pub use cookies::Cookies;
use http::{HeaderMap, HeaderValue, Method};
use reader_web_runtime::PreparedRequest;
use std::{num::NonZeroUsize, sync::Arc};
use url::Url;
use uuid::Uuid;

pub fn author(url: &Url) -> Option<&str> {
    if url.scheme() != "https"
        || url.host_str() != Some("www.zhihu.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let parts: Vec<_> = url.path().split('/').collect();
    if !(parts.len() == 3 || (parts.len() == 4 && parts[3] == "posts"))
        || !matches!(parts[1], "people" | "org")
        || parts[2].is_empty()
        || !parts[2]
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || matches!(v, b'-' | b'_'))
    {
        return None;
    }
    Some(parts[2])
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Paste a Cookie header or the complete Zhihu browser cookie table (including z_c0)")]
    InvalidCookies,
    #[error("Zhihu session expired or rejected. Replace cookies in Profile.")]
    Session,
    #[error("Zhihu is temporarily unavailable; the saved session has not been replaced")]
    Transport,
    #[error("Zhihu returned an unsupported response")]
    Protocol,
    #[error("Zhihu returned gated or truncated content; collection stopped before saving incomplete articles")]
    RestrictedContent,
    #[error("Zhihu session storage is unavailable")]
    Storage,
    #[error("Add Zhihu cookies in Profile to collect this source")]
    Missing,
    #[error("Authenticated Zhihu collection requires a source belonging to one account")]
    SharedSource,
}
#[async_trait]
pub trait Store: Send + Sync {
    async fn load(&self, owner: Uuid) -> Result<Option<Vec<u8>>, Error>;
    /// Atomically save encrypted cookies and activate the owner's existing Zhihu
    /// sources. Shared sources reject the entire transaction without changing data.
    async fn save(&self, owner: Uuid, encrypted: Vec<u8>, pages: NonZeroUsize)
        -> Result<(), Error>;
    async fn remove(&self, owner: Uuid) -> Result<(), Error>;
    async fn source_owner(&self, source: &SourceDefinition) -> Result<Uuid, Error>;
}
pub trait Vault: Send + Sync {
    fn seal(&self, owner: Uuid, raw: &str) -> Result<Vec<u8>, Error>;
    fn open(&self, owner: Uuid, encrypted: &[u8]) -> Result<String, Error>;
}
/// `http` must be the shared outbound client with a direct transport and zero
/// redirects. This service only constructs exact www.zhihu.com API requests.
pub struct Service {
    store: Arc<dyn Store>,
    vault: Arc<dyn Vault>,
    http: Arc<dyn BrowserHttpClient>,
    pages: NonZeroUsize,
}
impl Service {
    pub fn new(
        store: Arc<dyn Store>,
        vault: Arc<dyn Vault>,
        http: Arc<dyn BrowserHttpClient>,
        pages: NonZeroUsize,
    ) -> Self {
        Self {
            store,
            vault,
            http,
            pages,
        }
    }
    pub async fn configured(&self, owner: Uuid) -> Result<bool, Error> {
        Ok(self.store.load(owner).await?.is_some())
    }
    pub async fn save(&self, owner: Uuid, raw: String) -> Result<(), Error> {
        let cookies = Cookies::parse(&raw)?;
        self.validate(&cookies).await?;
        self.store
            .save(owner, self.vault.seal(owner, &raw)?, self.pages)
            .await
    }
    pub async fn remove(&self, owner: Uuid) -> Result<(), Error> {
        self.store.remove(owner).await
    }
    pub async fn check(&self, owner: Uuid) -> Result<(), Error> {
        self.validate(&self.cookies(owner).await?).await
    }
    async fn cookies(&self, owner: Uuid) -> Result<Cookies, Error> {
        let encrypted = self.store.load(owner).await?.ok_or(Error::Missing)?;
        Cookies::parse(&self.vault.open(owner, &encrypted)?)
    }
    async fn validate(&self, cookies: &Cookies) -> Result<(), Error> {
        let value = self.request(cookies, "/api/v4/me").await?;
        if value
            .get("id")
            .and_then(|v| v.as_str())
            .is_none_or(str::is_empty)
        {
            return Err(Error::Session);
        }
        Ok(())
    }
    async fn request(&self, cookies: &Cookies, path: &str) -> Result<serde_json::Value, Error> {
        let url =
            Url::parse(&format!("https://www.zhihu.com{path}")).map_err(|_| Error::Protocol)?;
        let mut headers = HeaderMap::new();
        headers.insert(http::header::COOKIE, cookies.header(url.path())?);
        headers.insert(
            http::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            http::header::REFERER,
            HeaderValue::from_static("https://www.zhihu.com/"),
        );
        let response = self
            .http
            .execute(PreparedRequest {
                method: Method::GET,
                url: url.clone(),
                headers,
                body: None,
            })
            .await
            .map_err(|_| Error::Transport)?;
        if matches!(response.status, 401 | 403) {
            return Err(Error::Session);
        }
        if response.status != 200 || response.final_url != url {
            return Err(Error::Transport);
        }
        serde_json::from_slice(&response.body).map_err(|_| Error::Protocol)
    }
    pub async fn collect(
        &self,
        source: &SourceDefinition,
        pages: NonZeroUsize,
    ) -> Result<Vec<SourceRecord>, FetchError> {
        self.collect_inner(source, pages)
            .await
            .map_err(|e| FetchError::Rejected(e.to_string()))
    }
}
#[cfg(test)]
mod tests;
