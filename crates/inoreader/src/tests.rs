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

#[test]
fn bulk_selection_limit_rejects_invalid_configuration_before_connecting() {
    for size in [0, usize::MAX] {
        let mut config = example();
        config.ingest.batch_items = size;
        assert!(config.validate().is_err());
    }
}

#[test]
fn stop_budgets_cover_both_paid_phases_and_container_never_preempts_drain() {
    let mut value = example();
    value.server.graceful_shutdown_seconds = value.ai.as_ref().unwrap().request_timeout_seconds;
    assert!(value.validate().is_err());
    value.server.graceful_shutdown_seconds = value.ai.as_ref().unwrap().lease_seconds + 1;
    value.validate().unwrap();
    assert!(value
        .validate_container_stop_budget(value.server.graceful_shutdown_seconds)
        .is_err());
    value
        .validate_container_stop_budget(value.server.graceful_shutdown_seconds + 1)
        .unwrap();
}

#[test]
fn recovery_batch_is_required_in_raw_configuration() {
    let mut raw: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    raw["ai"]
        .as_mapping_mut()
        .unwrap()
        .remove(serde_yaml::Value::String("recovery_batch".into()));
    assert!(serde_yaml::from_value::<Config>(raw).is_err());
}

#[test]
fn public_proxy_routes_default_to_direct_and_validate_before_io() {
    let config = example();
    assert!(config.http.public_proxy_routes.is_empty());
    config.http.public_fetch_transport().unwrap();
    let route = PublicProxy {
        target_hosts: vec!["aws.amazon.com".into()],
        endpoints: vec!["35.207.254.58:8899".parse().unwrap()],
        max_connect_header_bytes: 8192,
        endpoint_attempt_timeout_ms: 8000,
        quarantine_ms: 60000,
    };
    let mut config = example();
    config.http.public_proxy_routes = vec![route.clone()];
    config.validate().unwrap();
    for endpoints in [
        vec![],
        vec!["127.0.0.1:8080".parse().unwrap()],
        vec!["35.207.254.58:0".parse().unwrap()],
        vec!["8.8.8.8:80".parse().unwrap(); 2],
    ] {
        config.http.public_proxy_routes = vec![PublicProxy {
            endpoints,
            ..route.clone()
        }];
        assert!(config.validate().is_err());
    }
    for target_hosts in [
        vec![],
        vec!["*.amazon.com".into()],
        vec!["aws.amazon.com/path".into()],
        vec!["aws.amazon.com".into(), "aws.amazon.com".into()],
    ] {
        config.http.public_proxy_routes = vec![PublicProxy {
            target_hosts,
            ..route.clone()
        }];
        assert!(config.validate().is_err());
    }
    config.http.public_proxy_routes = vec![PublicProxy {
        max_connect_header_bytes: 0,
        ..route.clone()
    }];
    assert!(config.validate().is_err());
    for (endpoint_attempt_timeout_ms, quarantine_ms) in [
        (0, 60000),
        (8000, 0),
        (config.http.request_timeout_seconds * 1000 + 1, 60000),
    ] {
        config.http.public_proxy_routes = vec![
            route.clone(),
            PublicProxy {
                target_hosts: vec!["medium.com".into()],
                endpoint_attempt_timeout_ms,
                quarantine_ms,
                ..route.clone()
            },
        ];
        assert!(config.http.public_fetch_transport().is_err());
        assert!(config.validate().is_err());
    }
}

#[test]
fn public_proxy_routes_have_disjoint_hosts_and_independent_pools() {
    let mut config = example();
    let route = PublicProxy {
        target_hosts: vec!["aws.amazon.com".into()],
        endpoints: vec!["8.8.8.8:8080".parse().unwrap()],
        max_connect_header_bytes: 8192,
        endpoint_attempt_timeout_ms: 8000,
        quarantine_ms: 60000,
    };
    config.http.public_proxy_routes = vec![
        route.clone(),
        PublicProxy {
            target_hosts: vec!["medium.com".into()],
            endpoints: vec!["1.1.1.1:3128".parse().unwrap()],
            ..route.clone()
        },
    ];
    config.validate().unwrap();
    config.http.public_proxy_routes[1].endpoints = route.endpoints.clone();
    config.validate().unwrap(); // Shared endpoints are legal; host ownership is not.
    config.http.public_proxy_routes[1]
        .target_hosts
        .push("aws.amazon.com".into());
    assert!(config.http.public_fetch_transport().is_err());
    assert!(config.validate().is_err());
}

#[test]
fn public_proxy_route_wire_configuration_rejects_unknown_or_malformed_fields() {
    let mut raw: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    raw["http"]["public_proxy_routes"] = serde_yaml::from_str("- target_hosts: [aws.amazon.com]\n  endpoints: ['user:password@proxy.example:8080']\n  max_connect_header_bytes: 8192\n  endpoint_attempt_timeout_ms: 8000\n  quarantine_ms: 60000").unwrap();
    assert!(serde_yaml::from_value::<Config>(raw.clone()).is_err());
    raw["http"]["public_proxy_routes"] = serde_yaml::from_str("- target_hosts: [aws.amazon.com]\n  endpoints: ['35.207.254.58:8899']\n  max_connect_header_bytes: 8192\n  endpoint_attempt_timeout_ms: 8000\n  quarantine_ms: 60000\n  forward_credentials: true").unwrap();
    assert!(serde_yaml::from_value::<Config>(raw).is_err());
}

#[test]
fn omitted_public_proxy_routes_are_direct_but_non_sequences_are_rejected() {
    let mut raw: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    raw["http"]
        .as_mapping_mut()
        .unwrap()
        .remove(serde_yaml::Value::String("public_proxy_routes".into()));
    let config: Config = serde_yaml::from_value(raw.clone()).unwrap();
    assert!(config.http.public_proxy_routes.is_empty());
    config.validate().unwrap();
    for value in ["null", "{}", "false", "0", "''"] {
        raw["http"]["public_proxy_routes"] = serde_yaml::from_str(value).unwrap();
        assert!(serde_yaml::from_value::<Config>(raw.clone()).is_err());
    }
}

#[test]
fn enabled_public_proxy_routes_require_explicit_health_intervals() {
    let raw: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    let proxy: serde_yaml::Value = serde_yaml::from_str("target_hosts: [example.com]\nendpoints: ['8.8.8.8:80']\nmax_connect_header_bytes: 8192\nendpoint_attempt_timeout_ms: 8000\nquarantine_ms: 60000").unwrap();
    for missing in ["endpoint_attempt_timeout_ms", "quarantine_ms"] {
        let mut document = raw.clone();
        let mut incomplete = proxy.clone();
        incomplete
            .as_mapping_mut()
            .unwrap()
            .remove(serde_yaml::Value::String(missing.into()));
        document["http"]["public_proxy_routes"] = serde_yaml::Value::Sequence(vec![incomplete]);
        assert!(serde_yaml::from_value::<Config>(document).is_err());
    }
}

#[test]
fn retry_pacing_seconds_reject_overflow_before_startup_conversions() {
    for value in [u64::MAX, i64::MAX as u64, 9_000_000_000_000] {
        let mut config = example();
        config.scheduler.rate_limit_retry_seconds = value;
        assert!(config.validate().is_err());
        let mut config = example();
        config.scheduler.retry_jitter_seconds = value;
        assert!(config.validate().is_err());
        let mut config = example();
        config.scheduler.lease_seconds = value;
        assert!(config.validate().is_err());
    }
    let mut config = example();
    config.scheduler.rate_limit_retry_seconds = 0;
    assert!(config.validate().is_err());
    let mut config = example();
    config.scheduler.retry_jitter_seconds = 0;
    config.validate().unwrap();
}
