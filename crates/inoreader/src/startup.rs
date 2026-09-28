//! Startup composition responsibilities.
use super::*;

pub(super) async fn serve(
    config: &Config,
    pool: sqlx::PgPool,
    reason_policy: ReasonPolicy,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "archive_budget_bytes={} policy=telemetry_only",
        config.observability.archive_budget_bytes
    );
    let repository = Arc::new(PostgresRepository::new(
        pool.clone(),
        reason_policy,
        config.ingest.initial_feed_items,
        config.subscriptions.attention_after_seconds,
    )?);
    repository.readiness().await?;
    let ai_service = ai::compose(config, pool.clone(), repository.clone())?;
    let zhihu_service = zhihu::compose(config, pool.clone())?;
    // The transport exposes decoded response bytes, so both the wire/body
    // budget and decompressed budget constrain the same pre-parser boundary.
    let limits = OutboundLimits::try_from(RawOutboundLimits {
        connect_timeout_ms: config.http.connect_timeout_seconds * 1000,
        request_deadline_ms: config.http.request_timeout_seconds * 1000,
        max_redirect_hops: config.http.redirect_hops as usize,
        max_response_body_bytes: config
            .http
            .max_body_bytes
            .min(config.http.max_decompressed_bytes),
    })?;
    let outbound_policy = OutboundPolicy::for_plain_http_hosts(
        config.http.allowed_plain_http_hosts.iter().cloned(),
        limits,
    );
    let observer = RequestObserver {
        format: config.observability.log_format,
    };
    let public_transport = config.http.proxy_transport()?.with_observer(observer);
    let glossary_service = glossary::compose(config, pool.clone(), public_transport.clone())?;
    let fetcher = Arc::new(SecureWebFetcher::new(
        OutboundHttpClient::new(
            outbound_policy.clone(),
            TokioDnsResolver,
            public_transport.clone(),
            observer,
        )
        .with_user_agent(&config.http.user_agent)?,
    ));
    let icon_pool = pool.clone();
    let icon_fetcher = fetcher.clone();
    let icon_refresh_interval = Duration::from_secs(config.scheduler.polling_interval_seconds);
    let icon_workers = config.scheduler.workers;
    let icon_batch = config.ingest.batch_items;
    let browser_http: Arc<dyn BrowserHttpClient> = Arc::new(
        OutboundHttpClient::new(
            outbound_policy,
            TokioDnsResolver,
            public_transport,
            observer,
        )
        .with_user_agent(&config.http.user_agent)?,
    );
    let cdp = CdpBrowserCollector::configured(
        config.browser.cdp_endpoint.clone(),
        browser_http.clone(),
        config.browser.max_contexts,
        Duration::from_secs(config.browser.navigation_timeout_seconds),
        config.ingest.max_web_feed_pages,
    )?;
    if let Err(error) = cdp.health_check().await {
        eprintln!("browser capability degraded: {error}")
    }
    let web_feeds = Arc::new(ProductionWebCollector {
        static_feeds: StaticWebFeedCollector::new(fetcher.clone()),
        adapters: BuiltInAdapterCollector::new(browser_http),
        cdp,
        zhihu: zhihu_service.clone(),
        max_pages: config.ingest.max_web_feed_pages,
        max_actions: config.browser.max_actions,
    });
    let discovery = Arc::new(ProductionDiscovery {
        fetcher: fetcher.clone(),
        web_feeds: web_feeds.clone(),
        preview_timeout: Duration::from_secs(config.browser.preview_timeout_seconds),
        initial_items: config.ingest.initial_feed_items,
        visual_snapshots: tokio::sync::Mutex::new(HashMap::new()),
    });
    let mut server_state = reader_server::AppState::new(
        repository,
        discovery,
        reason_policy,
        auth_policy(config)?,
        config.server.external_origin.clone(),
        config.auth.login_attempts_per_minute,
        reader_application::SelectionLimit::new(config.ingest.batch_items)?,
    );
    if let Some(ai) = &ai_service {
        server_state = server_state.with_ai(ai.clone());
    }
    if let Some(glossary) = &glossary_service {
        server_state = server_state.with_glossary(glossary.clone());
    }
    if let Some(zhihu) = zhihu_service {
        server_state = server_state.with_zhihu(Arc::new(zhihu::Profile(zhihu)));
    }
    let wiki_limits = reader_wiki::Limits::try_from(config.wiki.clone())?;
    server_state = server_state.with_wiki(
        Arc::new(reader_storage_postgres::PostgresWikiStore::new(
            pool.clone(),
            wiki_limits.clone(),
        )),
        wiki_limits,
    );
    let search_limits = reader_application::SearchLimits::new(config.search.clone())?;
    server_state = server_state.with_search(
        Arc::new(reader_storage_postgres::PostgresSearchStore::new(
            pool.clone(),
            search_limits.clone(),
        )),
        search_limits,
    );
    let app = reader_server::router(server_state)
        .fallback(serve_ui)
        .layer(DefaultBodyLimit::max(config.server.max_request_body_bytes))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(config.server.request_timeout_seconds),
        ));
    let ingest_store = Arc::new(PostgresIngestStore::new(
        pool,
        chrono::Duration::seconds(config.scheduler.polling_interval_seconds as i64),
        config.scheduler.per_origin_concurrency,
        chrono::Duration::seconds(config.scheduler.max_retry_age_seconds as i64),
    )?);
    let ingest_limits = IngestLimits::new(
        config.ingest.initial_feed_items,
        config.ingest.batch_items,
        config.content.chunk_bytes,
        config.ingest.max_input_bytes,
        config.content.max_extracted_bytes,
    )?;
    let worker = Arc::new(
        IngestWorker::new(
            ingest_store,
            fetcher.clone(),
            fetcher,
            web_feeds,
            ingest_limits,
            chrono::Duration::seconds(config.scheduler.lease_seconds as i64),
        )?
        .with_runtime_policy(
            chrono::Duration::seconds(config.scheduler.renew_seconds as i64),
            config.scheduler.retry_attempts,
            chrono::Duration::seconds(config.scheduler.rate_limit_retry_seconds as i64),
            chrono::Duration::seconds(config.scheduler.retry_jitter_seconds as i64),
        )?,
    );
    let listener = tokio::net::TcpListener::bind(&config.server.bind).await?;
    let mut supervisor = reader_runtime::TaskSupervisor::new();
    supervisor.spawn("subscription_icons", move |mut stop| async move {
        while !stop.requested() {
            if let Err(error) = icons::refresh(
                icon_pool.clone(),
                icon_fetcher.clone(),
                icon_workers,
                icon_batch,
                &mut stop,
            )
            .await
            {
                log::warn!("subscription icon refresh failed: {error}");
            }
            stop.sleep(icon_refresh_interval).await;
        }
        Ok(())
    });

    if let Some(ai) = ai_service {
        ai.spawn_workers(&mut supervisor);
    }
    if let Some(glossary) = glossary_service {
        glossary.spawn_workers(&mut supervisor);
    }

    let worker_count = config.scheduler.workers;
    let poll = Duration::from_secs(config.scheduler.queue_poll_interval_seconds);
    supervisor.spawn("ingest", move |mut stop| async move {
        run_until_shutdown(worker, worker_count, poll, stop.wait()).await
    });
    supervisor.spawn("http", move |mut stop| async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { stop.wait().await })
            .await
            .map_err(|error| error.to_string())
    });
    let failure = tokio::select! {
        signal = reader_runtime::termination_signal() => { signal?; None },
        error = supervisor.unexpected_exit() => Some(error),
    };
    supervisor.request_shutdown();
    tokio::time::timeout(
        Duration::from_secs(config.server.graceful_shutdown_seconds),
        supervisor.drain(),
    )
    .await
    .map_err(|_| {
        format!(
            "graceful shutdown exceeded {} seconds",
            config.server.graceful_shutdown_seconds
        )
    })??;
    if let Some(error) = failure {
        return Err(error.into());
    }
    Ok(())
}

pub(super) fn auth_policy(config: &Config) -> Result<AuthPolicy, reader_application::AuthError> {
    Ok(AuthPolicy {
        session_lifetime_seconds: config.auth.session_lifetime_seconds,
        invite_lifetime_seconds: config.auth.invite_lifetime_seconds,
        reset_lifetime_seconds: config.auth.reset_lifetime_seconds,
        argon2id: Argon2idPolicy::new(
            config.auth.argon2id_memory_kib,
            config.auth.argon2id_time_cost,
            config.auth.argon2id_parallelism,
        )?,
    })
}

async fn serve_ui(uri: Uri) -> Response<Body> {
    match ui_asset(uri.path()) {
        Some(asset) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, asset.content_type)
            .header(header::CACHE_CONTROL, reader_server_ui::cache_control(asset))
            .header("content-security-policy", "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'self'; connect-src 'self'; img-src 'self' https: data:; style-src 'self'; script-src 'self'")
            .header("x-content-type-options", "nosniff")
            .header("referrer-policy", "strict-origin-when-cross-origin")
            .body(Body::from(asset.bytes))
            .expect("static response headers are valid"),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"code":"not_found","message":"resource not found"}"#))
            .expect("static not-found response is valid"),
    }
}

pub(super) fn ui_asset(path: &str) -> Option<reader_server_ui::Asset> {
    reader_server_ui::asset(path).or_else(|| {
        (!path.starts_with("/api/") && !path.rsplit('/').next().unwrap_or_default().contains('.'))
            .then(|| reader_server_ui::asset("/index.html"))
            .flatten()
    })
}
