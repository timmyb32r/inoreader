use super::*;
fn example() -> Config {
    serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap()
}

#[test]
fn example_configuration_is_valid() {
    Config::validate(&example()).unwrap()
}

#[test]
fn ai_policy_rejects_invalid_alternate_configuration_before_runtime() {
    let value = example();
    let raw = value.ai.unwrap();
    assert!(
        reader_ai::AiPolicy::new(raw.clone(), String::new(), String::new()).is_ok(),
        "unapproved prompt may be absent"
    );
    let mut invalid = raw.clone();
    invalid.prompt_approved = true;
    assert!(reader_ai::AiPolicy::new(invalid, String::new(), String::new()).is_err());
    let mut invalid = raw.clone();
    invalid.lease_seconds = invalid.request_timeout_seconds;
    assert!(reader_ai::AiPolicy::new(invalid, "prompt".into(), "review".into()).is_err());
    for field in 0..3 {
        let mut invalid = raw.clone();
        match field {
            0 => invalid.input_usd_per_million_tokens = "-1".into(),
            1 => invalid.cached_input_usd_per_million_tokens = "NaN".into(),
            _ => {
                invalid.output_usd_per_million_tokens =
                    "340282366920938463463374607431768211455".into()
            }
        }
        assert!(reader_ai::AiPolicy::new(invalid, "prompt".into(), "review".into()).is_err());
    }
    let mut invalid = raw;
    invalid.max_message_bytes = 0;
    assert!(reader_ai::AiPolicy::new(invalid, "prompt".into(), "review".into()).is_err());
}

#[test]
fn ai_generation_mode_rejects_ignored_parameters_and_non_finite_yaml() {
    let with_mode = |mode: &str| {
        let mut value: serde_yaml::Value =
            serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
        value["ai"]["generation_mode"] = serde_yaml::from_str(mode).unwrap();
        serde_yaml::from_value::<Config>(value)
    };
    for temperature in [".nan", ".inf", "-0.1"] {
        assert!(with_mode(&format!("kind: standard\ntemperature: {temperature}")).is_err());
    }
    assert!(with_mode("kind: thinking\neffort: high\ntemperature: 0.3").is_err());
    with_mode("kind: standard\ntemperature: 0.3")
        .unwrap()
        .validate()
        .unwrap();
    let config = with_mode("kind: thinking\neffort: high").unwrap();
    config.validate().unwrap();
    assert_eq!(
        config.ai.unwrap().generation_mode,
        reader_ai::GenerationMode::Thinking {
            effort: reader_ai::ReasoningEffort::High
        }
    );
}

#[test]
fn ai_review_configuration_is_explicit_and_both_prompts_must_be_approved_artifacts() {
    let mut config = example().ai.unwrap();
    config.prompt_approved = true;
    assert!(reader_ai::AiPolicy::new(config.clone(), "style".into(), String::new()).is_err());
    assert!(reader_ai::AiPolicy::new(config.clone(), String::new(), "review".into()).is_err());
    reader_ai::AiPolicy::new(config.clone(), "style".into(), "review".into()).unwrap();
    config.lease_seconds = config.request_timeout_seconds * 2;
    assert!(config.validate().is_err());
    config.lease_seconds += 1;
    config.validate().unwrap();
    for field in 0..4 {
        let mut invalid = config.clone();
        match field {
            0 => invalid.review.model.clear(),
            1 => invalid.review.max_output_tokens = 0,
            2 => invalid.review.input_usd_per_million_tokens = "-1".into(),
            _ => {
                invalid.review.max_output_tokens =
                    invalid.context_tokens - invalid.framing_tokens_base
            }
        }
        assert!(invalid.validate().is_err());
    }
    let mut wire: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    wire["ai"]["review"]["generation_mode"] =
        serde_yaml::from_str("kind: thinking\neffort: high\ntemperature: 0.3").unwrap();
    assert!(serde_yaml::from_value::<Config>(wire).is_err());
}

#[test]
fn zero_workers_explicitly_pauses_ingest_without_invalidating_the_server() {
    let mut value = example();
    value.scheduler.workers = 0;
    value.validate().unwrap();
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
    value.database.postgres.password_file_env = "bad-name".into();
    assert!(matches!(
        Config::validate(&value),
        Err(ConfigError::Invalid("database.postgres.password_file_env"))
    ))
}
#[test]
fn operator_endpoints_and_paths_are_validated_before_io() {
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
    let mut value: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    value["database"]["migration_source_ydb"] = serde_yaml::Value::Mapping(Default::default());
    assert!(
        serde_yaml::from_value::<Config>(value).is_err(),
        "removed database settings must fail explicitly"
    );
}
