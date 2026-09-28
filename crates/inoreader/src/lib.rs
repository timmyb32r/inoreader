use reader_web_runtime::ExternalRequestCompletion;
use serde::Deserialize;
use std::{fs, net::SocketAddr, path::Path};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub ai: Option<reader_ai::AiConfig>,
    pub glossary: Option<reader_glossary::GlossaryConfig>,

    pub server: Server,
    pub database: Database,
    pub auth: Auth,
    pub http: Http,
    pub scheduler: Scheduler,
    pub subscriptions: Subscriptions,
    pub browser: Browser,
    pub ingest: Ingest,
    pub content: Content,
    pub observability: Observability,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub bind: String,
    pub external_origin: String,
    pub request_timeout_seconds: u64,
    pub graceful_shutdown_seconds: u64,
    pub max_request_body_bytes: usize,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Database {
    pub postgres: Postgres,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Postgres {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password_file_env: String,
    pub max_connections: u32,
    pub acquire_timeout_seconds: u64,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    pub session_lifetime_seconds: u64,
    pub invite_lifetime_seconds: u64,
    pub reset_lifetime_seconds: u64,
    pub login_attempts_per_minute: u32,
    pub argon2id_memory_kib: u32,
    pub argon2id_time_cost: u32,
    pub argon2id_parallelism: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Http {
    pub user_agent: String,

    pub connect_timeout_seconds: u64,

    pub request_timeout_seconds: u64,

    pub redirect_hops: u32,

    pub max_body_bytes: usize,

    pub max_decompressed_bytes: usize,

    pub allowed_plain_http_hosts: Vec<String>,

    /// Explicit trust decisions for public, credential-free fetches only.
    /// Empty/absent means direct. Each exact host belongs to at most one route;
    /// different host groups may have different ordered endpoint pools.
    #[serde(default, deserialize_with = "deserialize_public_proxy_routes")]
    pub public_proxy_routes: Vec<PublicProxy>,
}

// YAML's generic Vec deserializer accepts explicit null as an empty sequence.
// Only an omitted field has the authored direct-fetch default; a supplied route
// configuration must be a sequence rather than silently disabling routing.
fn deserialize_public_proxy_routes<'de, D>(deserializer: D) -> Result<Vec<PublicProxy>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct RoutesVisitor;

    impl<'de> serde::de::Visitor<'de> for RoutesVisitor {
        type Value = Vec<PublicProxy>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a sequence of public proxy routes")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut routes = Vec::new();
            while let Some(route) = sequence.next_element()? {
                routes.push(route);
            }
            Ok(routes)
        }
    }

    deserializer.deserialize_any(RoutesVisitor)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicProxy {
    /// Exact destination hostnames; subdomains are not implicitly included.
    pub target_hosts: Vec<String>,

    /// Finite HTTP CONNECT proxy pool. Only public literal IP addresses allowed.
    pub endpoints: Vec<SocketAddr>,

    /// Maximum bytes read while receiving a CONNECT response header.
    pub max_connect_header_bytes: usize,

    /// One endpoint's full attempt, including its streamed response body.
    /// Must be positive and no greater than the overall HTTP request deadline.
    pub endpoint_attempt_timeout_ms: u64,

    /// Failed host/endpoint pairs become eligible for one real-request recovery
    /// probe after this positive interval. No background requests are generated.
    pub quarantine_ms: u64,
}
impl Http {
    /// Validate the explicit proxy trust boundary before any connection is made.
    /// The returned transport is used only by public feeds, pages and icons.
    pub fn public_fetch_transport(
        &self,
    ) -> Result<reader_web_runtime::PublicFetchTransport, ConfigError> {
        let mut routes = Vec::with_capacity(self.public_proxy_routes.len());
        for proxy in &self.public_proxy_routes {
            let attempt_timeout =
                std::time::Duration::from_millis(proxy.endpoint_attempt_timeout_ms);
            if attempt_timeout > std::time::Duration::from_secs(self.request_timeout_seconds) {
                return Err(ConfigError::Invalid(
                    "http.public_proxy_routes.endpoint_attempt_timeout_ms",
                ));
            }
            routes.push(
                reader_web_runtime::PublicProxyRoute::new(
                    proxy.target_hosts.clone(),
                    proxy.endpoints.clone(),
                    proxy.max_connect_header_bytes,
                    attempt_timeout,
                    std::time::Duration::from_millis(proxy.quarantine_ms),
                )
                .map_err(|_| ConfigError::Invalid("http.public_proxy_routes"))?,
            );
        }
        reader_web_runtime::PublicFetchTransport::new(routes)
            .map_err(|_| ConfigError::Invalid("http.public_proxy_routes"))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheduler {
    pub polling_interval_seconds: u64,
    pub queue_poll_interval_seconds: u64,
    pub lease_seconds: u64,
    pub renew_seconds: u64,
    pub workers: usize,
    pub per_origin_concurrency: usize,
    pub retry_attempts: u32,
    /// Minimum pause after HTTP 429, including responses without Retry-After.
    pub rate_limit_retry_seconds: u64,
    /// Stable per-job offset added to retry delays; zero disables staggering.
    pub retry_jitter_seconds: u64,

    pub max_retry_age_seconds: u64,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscriptions {
    pub pause_reason_max_bytes: usize,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Browser {
    pub cdp_endpoint: String,
    pub max_contexts: usize,
    pub max_pages_per_context: usize,
    pub navigation_timeout_seconds: u64,
    pub preview_timeout_seconds: u64,
    pub max_actions: usize,
    pub max_downloads: usize,
    pub egress_profile: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ingest {
    pub initial_feed_items: usize,
    pub max_web_feed_pages: usize,
    pub batch_items: usize,
    pub max_input_bytes: usize,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub version_policy: VersionPolicy,
    pub chunk_bytes: usize,
    pub max_extracted_bytes: usize,
    pub media_proxy_enabled: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VersionPolicy {
    LatestSuccessful,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Text,
    Json,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observability {
    pub log_format: LogFormat,
    pub external_request_metrics: bool,
    pub archive_budget_bytes: u64,
}

pub fn format_external_request_completion(
    format: LogFormat,
    value: &ExternalRequestCompletion,
) -> String {
    match format{LogFormat::Text=>format!("external_request system={} operation={} outcome={:?} elapsed_ms={}",value.system,value.operation,value.outcome,value.elapsed.as_millis()),LogFormat::Json=>serde_json::json!({"event":"external_request","system":value.system,"operation":value.operation,"outcome":format!("{:?}",value.outcome),"elapsed_ms":value.elapsed.as_millis()}).to_string()}
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid TOML config: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid YAML config: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid {0}")]
    Invalid(&'static str),
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path)?;
        let value = if matches!(
            path.extension().and_then(|v| v.to_str()),
            Some("yaml" | "yml")
        ) {
            serde_yaml::from_str(&raw)?
        } else {
            toml::from_str(&raw)?
        };
        Self::validate(&value)?;
        Ok(value)
    }
    pub fn validate_container_stop_budget(&self, seconds: u64) -> Result<(), ConfigError> {
        if seconds <= self.server.graceful_shutdown_seconds {
            return Err(ConfigError::Invalid(
                "CONTAINER_STOP_GRACE_SECONDS must exceed server.graceful_shutdown_seconds",
            ));
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(glossary) = &self.glossary {
            reader_glossary::GlossaryPolicy::new(glossary.clone())
                .map_err(|_| ConfigError::Invalid("glossary"))?;
            if self.ai.is_none() {
                return Err(ConfigError::Invalid(
                    "glossary requires ai credential encryption",
                ));
            }
        }
        if let Some(ai) = &self.ai {
            ai.validate().map_err(|_| ConfigError::Invalid("ai"))?;
        }
        // No accepted paid call is abandoned by the normal process stop budget.
        // AI leases cover generation + verification and their persistence margin.
        let admitted = [
            self.server.request_timeout_seconds,
            self.http.request_timeout_seconds,
            self.browser.preview_timeout_seconds,
            self.ai.as_ref().map_or(0, |ai| ai.lease_seconds),
            self.glossary
                .as_ref()
                .map_or(0, |glossary| glossary.lease_seconds),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        if self.server.graceful_shutdown_seconds <= admitted {
            return Err(ConfigError::Invalid(
                "server.graceful_shutdown_seconds must exceed admitted request/AI lease budgets",
            ));
        }
        let strings = [
            ("server.bind", self.server.bind.as_str()),
            (
                "server.external_origin",
                self.server.external_origin.as_str(),
            ),
            (
                "database.postgres.host",
                self.database.postgres.host.as_str(),
            ),
            (
                "database.postgres.database",
                self.database.postgres.database.as_str(),
            ),
            (
                "database.postgres.username",
                self.database.postgres.username.as_str(),
            ),
            (
                "database.postgres.password_file_env",
                self.database.postgres.password_file_env.as_str(),
            ),
            ("http.user_agent", self.http.user_agent.as_str()),
            ("browser.cdp_endpoint", self.browser.cdp_endpoint.as_str()),
            (
                "browser.egress_profile",
                self.browser.egress_profile.as_str(),
            ),
        ];
        for (n, v) in strings {
            if v.trim().is_empty() {
                return Err(ConfigError::Invalid(n));
            }
        }
        self.server
            .bind
            .parse::<std::net::SocketAddr>()
            .map_err(|_| ConfigError::Invalid("server.bind"))?;
        let external = url::Url::parse(&self.server.external_origin)
            .map_err(|_| ConfigError::Invalid("server.external_origin"))?;
        if !matches!(external.scheme(), "http" | "https")
            || external.host_str().is_none()
            || external.username() != ""
            || external.password().is_some()
            || external.query().is_some()
            || external.fragment().is_some()
            || external.path() != "/"
        {
            return Err(ConfigError::Invalid("server.external_origin"));
        }
        let cdp = url::Url::parse(&self.browser.cdp_endpoint)
            .map_err(|_| ConfigError::Invalid("browser.cdp_endpoint"))?;
        if !matches!(cdp.scheme(), "http" | "https" | "ws" | "wss")
            || cdp.host_str().is_none()
            || cdp.username() != ""
            || cdp.password().is_some()
        {
            return Err(ConfigError::Invalid("browser.cdp_endpoint"));
        }
        let values = [
            (
                "server.request_timeout_seconds",
                self.server.request_timeout_seconds,
            ),
            (
                "server.graceful_shutdown_seconds",
                self.server.graceful_shutdown_seconds,
            ),
            (
                "database.postgres.acquire_timeout_seconds",
                self.database.postgres.acquire_timeout_seconds,
            ),
            (
                "auth.session_lifetime_seconds",
                self.auth.session_lifetime_seconds,
            ),
            (
                "auth.invite_lifetime_seconds",
                self.auth.invite_lifetime_seconds,
            ),
            (
                "auth.reset_lifetime_seconds",
                self.auth.reset_lifetime_seconds,
            ),
            (
                "http.connect_timeout_seconds",
                self.http.connect_timeout_seconds,
            ),
            (
                "http.request_timeout_seconds",
                self.http.request_timeout_seconds,
            ),
            (
                "scheduler.polling_interval_seconds",
                self.scheduler.polling_interval_seconds,
            ),
            (
                "scheduler.queue_poll_interval_seconds",
                self.scheduler.queue_poll_interval_seconds,
            ),
            ("scheduler.lease_seconds", self.scheduler.lease_seconds),
            ("scheduler.renew_seconds", self.scheduler.renew_seconds),
            (
                "scheduler.rate_limit_retry_seconds",
                self.scheduler.rate_limit_retry_seconds,
            ),
            (
                "scheduler.max_retry_age_seconds",
                self.scheduler.max_retry_age_seconds,
            ),
            (
                "observability.archive_budget_bytes",
                self.observability.archive_budget_bytes,
            ),
        ];
        for (n, v) in values {
            if v == 0 {
                return Err(ConfigError::Invalid(n));
            }
        }
        // Validate scheduler seconds before startup converts them to chrono
        // durations or signed timestamps. Jitter alone may deliberately be zero.
        for (name, seconds) in [
            (
                "scheduler.polling_interval_seconds",
                self.scheduler.polling_interval_seconds,
            ),
            (
                "scheduler.queue_poll_interval_seconds",
                self.scheduler.queue_poll_interval_seconds,
            ),
            ("scheduler.lease_seconds", self.scheduler.lease_seconds),
            ("scheduler.renew_seconds", self.scheduler.renew_seconds),
            (
                "scheduler.max_retry_age_seconds",
                self.scheduler.max_retry_age_seconds,
            ),
            (
                "scheduler.rate_limit_retry_seconds",
                self.scheduler.rate_limit_retry_seconds,
            ),
            (
                "scheduler.retry_jitter_seconds",
                self.scheduler.retry_jitter_seconds,
            ),
        ] {
            let duration = i64::try_from(seconds)
                .ok()
                .and_then(chrono::Duration::try_seconds)
                .ok_or(ConfigError::Invalid(name))?;
            if chrono::Utc::now().checked_add_signed(duration).is_none() {
                return Err(ConfigError::Invalid(name));
            }
        }
        let retry_total = self
            .scheduler
            .rate_limit_retry_seconds
            .max(1024)
            .checked_add(self.scheduler.retry_jitter_seconds)
            .and_then(|value| i64::try_from(value).ok())
            .and_then(chrono::Duration::try_seconds)
            .and_then(|duration| chrono::Utc::now().checked_add_signed(duration));
        if retry_total.is_none() {
            return Err(ConfigError::Invalid("scheduler retry delay plus jitter"));
        }
        let sizes = [
            (
                "server.max_request_body_bytes",
                self.server.max_request_body_bytes,
            ),
            (
                "subscriptions.pause_reason_max_bytes",
                self.subscriptions.pause_reason_max_bytes,
            ),
            ("http.max_body_bytes", self.http.max_body_bytes),
            (
                "http.max_decompressed_bytes",
                self.http.max_decompressed_bytes,
            ),
            (
                "scheduler.per_origin_concurrency",
                self.scheduler.per_origin_concurrency,
            ),
            ("browser.max_contexts", self.browser.max_contexts),
            (
                "browser.max_pages_per_context",
                self.browser.max_pages_per_context,
            ),
            ("browser.max_actions", self.browser.max_actions),
            ("ingest.initial_feed_items", self.ingest.initial_feed_items),
            ("ingest.max_web_feed_pages", self.ingest.max_web_feed_pages),
            ("ingest.batch_items", self.ingest.batch_items),
            ("ingest.max_input_bytes", self.ingest.max_input_bytes),
            ("content.chunk_bytes", self.content.chunk_bytes),
            (
                "content.max_extracted_bytes",
                self.content.max_extracted_bytes,
            ),
        ];
        for (n, v) in sizes {
            if v == 0 {
                return Err(ConfigError::Invalid(n));
            }
        }
        reader_application::SelectionLimit::new(self.ingest.batch_items)
            .map_err(|_| ConfigError::Invalid("ingest.batch_items"))?;
        if self.http.redirect_hops == 0
            || self.scheduler.retry_attempts == 0
            || self.database.postgres.max_connections == 0
            || self.database.postgres.port == 0
            || self.auth.login_attempts_per_minute == 0
        {
            return Err(ConfigError::Invalid("attempt and redirect limits"));
        }
        if self.server.bind.parse::<SocketAddr>().is_err() {
            return Err(ConfigError::Invalid("server.bind"));
        }
        let origin = url::Url::parse(&self.server.external_origin)
            .map_err(|_| ConfigError::Invalid("server.external_origin"))?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(ConfigError::Invalid("server.external_origin"));
        }
        let mut postgres_env = self.database.postgres.password_file_env.bytes();
        if !postgres_env
            .next()
            .is_some_and(|v| v == b'_' || v.is_ascii_alphabetic())
            || !postgres_env.all(|v| v == b'_' || v.is_ascii_alphanumeric())
        {
            return Err(ConfigError::Invalid("database.postgres.password_file_env"));
        }
        for (name, value) in [
            ("database.postgres.host", &self.database.postgres.host),
            (
                "database.postgres.database",
                &self.database.postgres.database,
            ),
            (
                "database.postgres.username",
                &self.database.postgres.username,
            ),
        ] {
            if value.contains(['\0', '\n', '\r']) {
                return Err(ConfigError::Invalid(name));
            }
        }
        if self.http.user_agent.bytes().any(|v| v < 0x20 || v == 0x7f) {
            return Err(ConfigError::Invalid("http.user_agent"));
        }
        if self.http.connect_timeout_seconds > self.http.request_timeout_seconds {
            return Err(ConfigError::Invalid("http.connect_timeout_seconds"));
        }
        if self.http.max_decompressed_bytes < self.http.max_body_bytes {
            return Err(ConfigError::Invalid("http.max_decompressed_bytes"));
        }
        self.http.public_fetch_transport()?;
        let mut plain_http_hosts = std::collections::HashSet::new();
        for host in &self.http.allowed_plain_http_hosts {
            let parsed = url::Url::parse(&format!("http://{host}/"))
                .map_err(|_| ConfigError::Invalid("http.allowed_plain_http_hosts"))?;
            if parsed.host_str() != Some(host.as_str())
                || parsed.port().is_some()
                || !plain_http_hosts.insert(host)
            {
                return Err(ConfigError::Invalid("http.allowed_plain_http_hosts"));
            }
        }
        let cdp = url::Url::parse(&self.browser.cdp_endpoint)
            .map_err(|_| ConfigError::Invalid("browser.cdp_endpoint"))?;
        if cdp.scheme() != "http"
            || cdp.host_str().is_none()
            || !cdp.username().is_empty()
            || cdp.password().is_some()
            || cdp.path() != "/"
            || cdp.query().is_some()
            || cdp.fragment().is_some()
        {
            return Err(ConfigError::Invalid("browser.cdp_endpoint"));
        }
        if self.browser.egress_profile != "public-web-only" {
            return Err(ConfigError::Invalid("browser.egress_profile"));
        }
        if self.browser.navigation_timeout_seconds > self.browser.preview_timeout_seconds {
            return Err(ConfigError::Invalid("browser.navigation_timeout_seconds"));
        }
        if self.content.chunk_bytes > self.content.max_extracted_bytes {
            return Err(ConfigError::Invalid("content.chunk_bytes"));
        }
        if reader_application::Argon2idPolicy::new(
            self.auth.argon2id_memory_kib,
            self.auth.argon2id_time_cost,
            self.auth.argon2id_parallelism,
        )
        .is_err()
        {
            return Err(ConfigError::Invalid("auth Argon2id policy"));
        }
        if self.scheduler.renew_seconds >= self.scheduler.lease_seconds {
            return Err(ConfigError::Invalid("scheduler.renew_seconds"));
        }
        if self.browser.max_pages_per_context != 1 {
            return Err(ConfigError::Invalid(
                "browser.max_pages_per_context (v1 requires 1)",
            ));
        }
        if self.browser.max_downloads != 0 {
            return Err(ConfigError::Invalid(
                "browser.max_downloads (v1 requires 0)",
            ));
        }
        if self.content.media_proxy_enabled {
            return Err(ConfigError::Invalid(
                "content.media_proxy_enabled (unsupported in v1)",
            ));
        }
        if !self.observability.external_request_metrics {
            return Err(ConfigError::Invalid(
                "observability.external_request_metrics (mandatory)",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
