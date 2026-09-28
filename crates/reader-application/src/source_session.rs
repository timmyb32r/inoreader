use async_trait::async_trait;
use uuid::Uuid;
/// Account-owned source-session profile boundary. Errors must be stable,
/// credential-free messages; neither status nor errors return secret material.
/// Save validates remotely before atomically replacing the encrypted credential.
#[async_trait]
pub trait ZhihuProfilePort: Send + Sync {
    async fn configured(&self, owner: Uuid) -> Result<bool, String>;
    async fn save(&self, owner: Uuid, cookies: String) -> Result<(), String>;
    async fn check(&self, owner: Uuid) -> Result<(), String>;
    async fn remove(&self, owner: Uuid) -> Result<(), String>;
}
