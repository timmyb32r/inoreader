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
    for model in ["flash", "pro"] {
        for (field, bad) in [
            ("input_usd_per_million_tokens", "-1"),
            ("cached_input_usd_per_million_tokens", "NaN"),
            (
                "output_usd_per_million_tokens",
                "340282366920938463463374607431768211455",
            ),
        ] {
            let mut value: serde_yaml::Value =
                serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
            value["ai"]["models"][model][field] = bad.into();
            assert!(serde_yaml::from_value::<Config>(value).is_err());
        }
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
            0 => invalid.review.prompt_version.clear(),
            1 => invalid.review.max_output_tokens = 0,
            2 => invalid.review.prompt_path.clear(),
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
fn bulk_batch_capacity_rejects_invalid_configuration_before_connecting() {
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

fn proxy_config() -> Config {
    let mut config = example();
    config.http.proxy_pool = Some(ProxyPool {
        endpoints: vec!["8.8.8.8:8080".parse().unwrap()],
        max_connect_header_bytes: 8192,
        endpoint_attempt_timeout_ms: 8000,
        quarantine_ms: 60000,
    });
    config.http.proxy_routes = vec![ProxyRoute {
        target_hosts: vec!["example.com".into()],
        direct_first: false,
    }];
    config
}

#[test]
fn shared_proxy_pool_validates_before_io() {
    example().http.proxy_transport().unwrap();
    proxy_config().validate().unwrap();
    for endpoints in [
        vec![],
        vec!["127.0.0.1:8080".parse().unwrap()],
        vec!["8.8.8.8:0".parse().unwrap()],
        vec!["8.8.8.8:80".parse().unwrap(); 2],
    ] {
        let mut c = proxy_config();
        c.http.proxy_pool.as_mut().unwrap().endpoints = endpoints;
        assert!(c.validate().is_err());
    }
    for hosts in [
        vec![],
        vec!["*.example.com".into()],
        vec!["example.com/path".into()],
        vec!["example.com".into(); 2],
    ] {
        let mut c = proxy_config();
        c.http.proxy_routes[0].target_hosts = hosts;
        assert!(c.validate().is_err());
    }
    for (header, timeout, quarantine) in [
        (0, 8000, 60000),
        (8192, 0, 60000),
        (8192, 8000, 0),
        (8192, 999999999, 60000),
    ] {
        let mut c = proxy_config();
        let p = c.http.proxy_pool.as_mut().unwrap();
        p.max_connect_header_bytes = header;
        p.endpoint_attempt_timeout_ms = timeout;
        p.quarantine_ms = quarantine;
        assert!(c.validate().is_err());
    }
    let mut c = proxy_config();
    c.http.proxy_pool = None;
    assert!(c.validate().is_err());
    let mut c = proxy_config();
    c.http.proxy_routes.clear();
    assert!(c.validate().is_err());
}

#[test]
fn shared_proxy_routes_are_disjoint_and_cannot_override_pool() {
    let mut c = proxy_config();
    c.http.proxy_routes.push(ProxyRoute {
        target_hosts: vec!["api.telegram.org".into()],
        direct_first: false,
    });
    c.http
        .proxy_pool
        .as_mut()
        .unwrap()
        .endpoint_attempt_timeout_ms = 35000;
    c.validate().unwrap();
    c.http.proxy_routes[1]
        .target_hosts
        .push("example.com".into());
    assert!(c.validate().is_err());
    let raw: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    for key in ["endpoints", "forward_credentials"] {
        let mut v = raw.clone();
        v["http"]["proxy_routes"] = serde_yaml::from_str(&format!(
            "- target_hosts: [example.com]\n  direct_first: false\n  {key}: true"
        ))
        .unwrap();
        assert!(serde_yaml::from_value::<Config>(v).is_err());
    }
    for field in [
        "endpoint_attempt_timeout_ms",
        "quarantine_ms",
        "endpoints",
        "max_connect_header_bytes",
    ] {
        let mut v = raw.clone();
        let mut pool:serde_yaml::Value=serde_yaml::from_str("endpoints: ['8.8.8.8:8080']\nmax_connect_header_bytes: 8192\nendpoint_attempt_timeout_ms: 8000\nquarantine_ms: 60000").unwrap();
        pool.as_mapping_mut()
            .unwrap()
            .remove(serde_yaml::Value::String(field.into()));
        v["http"]["proxy_pool"] = pool;
        assert!(serde_yaml::from_value::<Config>(v).is_err());
    }
    let mut v = raw.clone();
    v["http"]["public_proxy_routes"] = serde_yaml::Value::Sequence(vec![]);
    assert!(serde_yaml::from_value::<Config>(v).is_err());
}

#[test]
fn absent_proxy_routes_are_direct_but_malformed_routes_are_rejected() {
    let mut raw: serde_yaml::Value =
        serde_yaml::from_str(include_str!("../../../config.example.yaml")).unwrap();
    raw["http"]
        .as_mapping_mut()
        .unwrap()
        .remove(serde_yaml::Value::String("proxy_routes".into()));
    serde_yaml::from_value::<Config>(raw.clone())
        .unwrap()
        .validate()
        .unwrap();
    for value in ["null", "{}", "false", "0", "''"] {
        let mut v = raw.clone();
        v["http"]["proxy_routes"] = serde_yaml::from_str(value).unwrap();
        assert!(serde_yaml::from_value::<Config>(v).is_err());
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

#[test]
fn attention_duration_is_explicit_positive_and_lossless() {
    for seconds in [0, u64::MAX] {
        let mut config = example();
        config.subscriptions.attention_after_seconds = seconds;
        assert!(config.validate().is_err());
    }
    let missing = include_str!("../../../config.example.yaml")
        .replace("  attention_after_seconds: 86400\n", "");
    assert!(serde_yaml::from_str::<Config>(&missing).is_err());
}

#[test]
fn telegram_long_poll_must_fit_shared_attempt_and_request_deadline() {
    let mut c = proxy_config();
    c.http.proxy_routes[0].target_hosts = vec!["api.telegram.org".into()];
    for timeout in [1, 25000, 46000] {
        c.http
            .proxy_pool
            .as_mut()
            .unwrap()
            .endpoint_attempt_timeout_ms = timeout;
        assert!(c.validate().is_err());
    }
    c.http
        .proxy_pool
        .as_mut()
        .unwrap()
        .endpoint_attempt_timeout_ms = 35000;
    c.validate().unwrap();
}
