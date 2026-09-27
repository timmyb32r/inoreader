use super::*;
use reader_ai::{AiPolicy, AiService, CredentialCipher, DeepSeekProvider};
use reader_storage_postgres::PostgresAiStore;

pub(super) fn compose(
    config: &Config,
    pool: sqlx::PgPool,
    repository: Arc<PostgresRepository>,
) -> Result<Option<Arc<AiService>>, Box<dyn std::error::Error>> {
    let Some(ai) = &config.ai else {
        return Ok(None);
    };
    ai.validate()?;
    if ai.enabled_accounts.is_empty() {
        return Ok(None);
    }
    let path = std::env::var(&ai.encryption_key_file_env)
        .map_err(|_| "AI encryption key file environment variable is missing")?;
    let master = std::fs::read(path).map_err(|_| "AI encryption key file is unavailable")?;
    let cipher = CredentialCipher::new(&master)?;
    // Prompt evaluation is an explicit gate. Missing candidate artifacts do not
    // prevent starting an installation with generation disabled.
    let prompt = if ai.prompt_approved {
        std::fs::read_to_string(&ai.prompt_path)
            .map_err(|_| "Approved AI prompt file is unavailable")?
    } else {
        String::new()
    };
    let review_prompt = if ai.prompt_approved {
        std::fs::read_to_string(&ai.review.prompt_path)
            .map_err(|_| "Approved AI factual-review prompt file is unavailable")?
    } else {
        String::new()
    };
    let policy = AiPolicy::new(ai.clone(), prompt, review_prompt)?;
    let limits = OutboundLimits::try_from(RawOutboundLimits {
        connect_timeout_ms: ai
            .connect_timeout_seconds
            .checked_mul(1000)
            .ok_or("AI timeout overflow")?,
        request_deadline_ms: ai
            .request_timeout_seconds
            .checked_mul(1000)
            .ok_or("AI timeout overflow")?,
        max_redirect_hops: config.http.redirect_hops as usize,
        max_response_body_bytes: ai.max_response_bytes,
    })?;
    let http = OutboundHttpClient::new(
        OutboundPolicy::for_plain_http_hosts([], limits),
        TokioDnsResolver,
        ReqwestPinnedTransport,
        RequestObserver {
            format: config.observability.log_format,
        },
    );
    let provider = DeepSeekProvider::new(http, Duration::from_millis(ai.poll_milliseconds))?;
    let store = PostgresAiStore::new(pool, repository);
    Ok(Some(Arc::new(AiService::new(
        Arc::new(store),
        Arc::new(provider),
        cipher,
        policy,
    ))))
}
