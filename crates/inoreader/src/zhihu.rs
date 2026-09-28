use super::*;
use reader_ingest::zhihu::{Error, Service, Vault};
struct Cipher(reader_ai::CredentialCipher);
impl Vault for Cipher {
    fn seal(&self, owner: uuid::Uuid, raw: &str) -> Result<Vec<u8>, Error> {
        self.0.encrypt(owner, raw).map_err(|_| Error::Storage)
    }
    fn open(&self, owner: uuid::Uuid, raw: &[u8]) -> Result<String, Error> {
        self.0.decrypt(owner, raw).map_err(|_| Error::Storage)
    }
}
pub(super) fn compose(
    config: &Config,
    pool: sqlx::PgPool,
) -> Result<Option<Arc<Service>>, Box<dyn std::error::Error>> {
    let Some(ai) = &config.ai else {
        return Ok(None);
    };
    let path = std::env::var(&ai.encryption_key_file_env)
        .map_err(|_| "Encryption key file environment is unavailable")?;
    let master = std::fs::read(path).map_err(|_| "Encryption key file is unavailable")?;
    let limits = OutboundLimits::try_from(RawOutboundLimits {
        connect_timeout_ms: config.http.connect_timeout_seconds * 1000,
        request_deadline_ms: config.http.request_timeout_seconds * 1000,
        // Credential-bearing requests intentionally reject even same-host redirects.
        max_redirect_hops: 0,
        max_response_body_bytes: config
            .http
            .max_body_bytes
            .min(config.http.max_decompressed_bytes),
    })?;
    let http = OutboundHttpClient::new(
        OutboundPolicy::for_plain_http_hosts([], limits),
        TokioDnsResolver,
        ReqwestPinnedTransport,
        RequestObserver {
            format: config.observability.log_format,
        },
    )
    .with_user_agent(&config.http.user_agent)?;
    Ok(Some(Arc::new(Service::new(
        Arc::new(reader_storage_postgres::PostgresZhihuStore::new(pool)),
        Arc::new(Cipher(reader_ai::CredentialCipher::new(&master)?)),
        Arc::new(http),
        std::num::NonZeroUsize::new(config.ingest.max_web_feed_pages)
            .ok_or("Zhihu page limit must be positive")?,
    ))))
}

pub(super) struct Profile(pub Arc<Service>);
#[async_trait::async_trait]
impl reader_application::ZhihuProfilePort for Profile {
    async fn configured(&self, owner: uuid::Uuid) -> Result<bool, String> {
        self.0.configured(owner).await.map_err(|e| e.to_string())
    }
    async fn save(&self, owner: uuid::Uuid, cookies: String) -> Result<(), String> {
        self.0.save(owner, cookies).await.map_err(|e| e.to_string())
    }
    async fn check(&self, owner: uuid::Uuid) -> Result<(), String> {
        self.0.check(owner).await.map_err(|e| e.to_string())
    }
    async fn remove(&self, owner: uuid::Uuid) -> Result<(), String> {
        self.0.remove(owner).await.map_err(|e| e.to_string())
    }
}
