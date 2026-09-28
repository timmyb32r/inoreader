use super::*;
use reader_glossary::{
    GlossaryError, GlossaryPolicy, GlossaryService, GlossaryVault, TelegramClient,
};
use reader_storage_postgres::PostgresGlossaryStore;

mod archive;
pub(super) use archive::import;

struct Vault(reader_ai::CredentialCipher);
impl GlossaryVault for Vault {
    fn seal(&self, owner: uuid::Uuid, value: &str) -> Result<Vec<u8>, GlossaryError> {
        self.0
            .encrypt(owner, value)
            .map_err(|_| GlossaryError::Storage)
    }
    fn open(&self, owner: uuid::Uuid, value: &[u8]) -> Result<String, GlossaryError> {
        self.0
            .decrypt(owner, value)
            .map_err(|_| GlossaryError::Storage)
    }
}
pub(super) fn compose(
    config: &Config,
    pool: sqlx::PgPool,
    transport: ProxyTransport,
) -> Result<Option<Arc<GlossaryService>>, Box<dyn std::error::Error>> {
    let Some(raw) = &config.glossary else {
        return Ok(None);
    };
    let policy = GlossaryPolicy::new(raw.clone())?;
    let ai = config
        .ai
        .as_ref()
        .ok_or("glossary requires ai encryption settings")?;
    let path = std::env::var(&ai.encryption_key_file_env)
        .map_err(|_| "Encryption key file environment is unavailable")?;
    let master = std::fs::read(path).map_err(|_| "Encryption key file is unavailable")?;
    let vault = Vault(reader_ai::CredentialCipher::new(&master)?);
    let http = |transport: ProxyTransport| -> Result<_, Box<dyn std::error::Error>> {
        let limits = OutboundLimits::try_from(RawOutboundLimits {
            connect_timeout_ms: raw
                .connect_timeout_seconds
                .checked_mul(1000)
                .ok_or("Timeout overflow")?,
            request_deadline_ms: raw
                .request_timeout_seconds
                .checked_mul(1000)
                .ok_or("Timeout overflow")?,
            max_redirect_hops: config.http.redirect_hops as usize,
            max_response_body_bytes: raw.max_response_bytes,
        })?;
        Ok(OutboundHttpClient::new(
            OutboundPolicy::for_plain_http_hosts([], limits),
            TokioDnsResolver,
            transport,
            RequestObserver {
                format: config.observability.log_format,
            },
        ))
    };
    let gateway = TelegramClient::new(
        http(transport.clone().for_telegram_bot_api())?,
        http(transport)?,
        policy.clone(),
    )?;
    Ok(Some(Arc::new(GlossaryService::new(
        Arc::new(PostgresGlossaryStore::new(pool)),
        Arc::new(gateway),
        Arc::new(vault),
        policy,
        ai.enabled_accounts.clone(),
    ))))
}
