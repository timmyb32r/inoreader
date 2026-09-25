use super::*;
fn example() -> Config {
    serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap()
}

#[test]
fn example_configuration_is_valid() {
    Config::validate(&example()).unwrap()
}
#[test]
fn external_request_metrics_cannot_be_disabled() {
    let mut value = example();
    value.observability.external_request_metrics = false;
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid(
            "observability.external_request_metrics (mandatory)"
        ))
    ))
}
#[test]
fn invalid_argon2id_cost_is_rejected_at_configuration_boundary() {
    let mut value = example();
    value.auth.argon2id_memory_kib = 0;
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("auth Argon2id policy"))
    ))
}
#[test]
fn malformed_listener_and_non_origin_public_url_fail_before_startup() {
    let mut value = example();
    value.server.bind = "localhost:not-a-port".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("server.bind"))
    ));
    let mut value = example();
    value.server.external_origin = "https://reader.example.test/path?token=secret".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("server.external_origin"))
    ))
}
#[test]
fn credentials_reference_must_be_a_portable_environment_name() {
    let mut value = example();
    value.database.ydb.credentials_env = "bad-name".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("database.ydb.credentials_env"))
    ))
}
#[test]
fn operator_endpoints_and_paths_are_validated_before_io() {
    let mut value = example();
    value.database.ydb.endpoint = "https://ydb.example.test".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("database.ydb.endpoint"))
    ));
    let mut value = example();
    value.database.ydb.database_path = "relative".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("database.ydb.database_path"))
    ));
    let mut value = example();
    value.browser.cdp_endpoint = "http://user@chromium:9222/path".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("browser.cdp_endpoint"))
    ))
}
#[test]
fn related_resource_limits_are_consistent() {
    let mut value = example();
    value.http.connect_timeout_seconds = value.http.request_timeout_seconds + 1;
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("http.connect_timeout_seconds"))
    ));
    let mut value = example();
    value.http.max_decompressed_bytes = value.http.max_body_bytes - 1;
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("http.max_decompressed_bytes"))
    ));
    let mut value = example();
    value.browser.navigation_timeout_seconds = value.browser.preview_timeout_seconds + 1;
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("browser.navigation_timeout_seconds"))
    ))
}
#[test]
fn external_request_format_follows_configured_mode() {
    use reader_web_runtime::{ExternalRequestCompletion, ExternalRequestOutcome};
    use std::time::Duration;
    let completion = ExternalRequestCompletion {
        system: "http",
        operation: "fetch",
        outcome: ExternalRequestOutcome::Success,
        elapsed: Duration::from_millis(12),
    };
    assert_eq!(
        format_external_request_completion(LogFormat::Text, &completion),
        "external_request system=http operation=fetch outcome=Success elapsed_ms=12"
    );
    let json = format_external_request_completion(LogFormat::Json, &completion);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["system"], "http");
    assert_eq!(parsed["elapsed_ms"], 12)
}
#[test]
fn unknown_fields_fail() {
    let value = r#"[server]
listen="x"
surprise=true
[ydb]
endpoint="e"
database="d"
credentials_env="TOKEN"
[subscriptions]
pause_reason_max_bytes=1
[http]
request_timeout_ms=1
redirect_hops=1
max_response_bytes=1
[jobs]
lease_ms=1
max_attempts=1
concurrency=1
"#;
    assert!(toml::from_str::<Config>(value).is_err());
}
