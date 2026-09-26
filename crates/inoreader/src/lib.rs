use reader_web_runtime::ExternalRequestCompletion;
use serde::Deserialize;
use std::{fs, net::SocketAddr, path::Path};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
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
    pub migration_source_ydb: Ydb,
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
pub struct Ydb {
    pub endpoint: String,
    pub database_path: String,
    pub credentials_env: String,
    pub request_timeout_seconds: u64,
    pub max_concurrency: usize,
    pub retry_attempts: u32,
    pub retry_initial_backoff_milliseconds: u64,
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
    pub fn validate(&self) -> Result<(), ConfigError> {
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
            (
                "database.migration_source_ydb.endpoint",
                self.database.migration_source_ydb.endpoint.as_str(),
            ),
            (
                "database.migration_source_ydb.database_path",
                self.database.migration_source_ydb.database_path.as_str(),
            ),
            (
                "database.migration_source_ydb.credentials_env",
                self.database.migration_source_ydb.credentials_env.as_str(),
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
        if !self
            .database
            .migration_source_ydb
            .credentials_env
            .bytes()
            .enumerate()
            .all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_uppercase() || index > 0 && byte.is_ascii_digit()
            })
        {
            return Err(ConfigError::Invalid(
                "database.migration_source_ydb.credentials_env",
            ));
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
                "database.migration_source_ydb.request_timeout_seconds",
                self.database.migration_source_ydb.request_timeout_seconds,
            ),
            (
                "database.migration_source_ydb.retry_initial_backoff_milliseconds",
                self.database
                    .migration_source_ydb
                    .retry_initial_backoff_milliseconds,
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
        let sizes = [
            (
                "server.max_request_body_bytes",
                self.server.max_request_body_bytes,
            ),
            (
                "database.migration_source_ydb.max_concurrency",
                self.database.migration_source_ydb.max_concurrency,
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
        if self.http.redirect_hops == 0
            || self.scheduler.retry_attempts == 0
            || self.database.migration_source_ydb.retry_attempts == 0
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
        let endpoint = url::Url::parse(&self.database.migration_source_ydb.endpoint)
            .map_err(|_| ConfigError::Invalid("database.migration_source_ydb.endpoint"))?;
        if !matches!(endpoint.scheme(), "grpc" | "grpcs")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || !matches!(endpoint.path(), "" | "/")
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(ConfigError::Invalid(
                "database.migration_source_ydb.endpoint",
            ));
        }
        if !self
            .database
            .migration_source_ydb
            .database_path
            .starts_with('/')
            || self.database.migration_source_ydb.database_path == "/"
            || self
                .database
                .migration_source_ydb
                .database_path
                .contains(['\n', '\r'])
        {
            return Err(ConfigError::Invalid(
                "database.migration_source_ydb.database_path",
            ));
        }
        let mut env = self.database.migration_source_ydb.credentials_env.bytes();
        if !env
            .next()
            .is_some_and(|v| v == b'_' || v.is_ascii_alphabetic())
            || !env.all(|v| v == b'_' || v.is_ascii_alphanumeric())
        {
            return Err(ConfigError::Invalid(
                "database.migration_source_ydb.credentials_env",
            ));
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
