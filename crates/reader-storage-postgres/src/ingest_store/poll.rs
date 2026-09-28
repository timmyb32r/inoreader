//! PostgreSQL poll operations. Transaction boundaries stay local and explicit.
use super::*;

pub(super) async fn source(
    store: &PostgresIngestStore,
    id: SourceId,
) -> Result<SourceDefinition, StoreError> {
    let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id = $1")
        .bind(id.as_uuid().to_string())
        .fetch_optional(&store.pool)
        .await
        .map_err(storage)?
        .ok_or(StoreError::NotFound)?;
    decode(&document)
}

pub(super) async fn has_committed_poll(
    store: &PostgresIngestStore,
    source: SourceId,
) -> Result<bool, StoreError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM content_refresh_state WHERE id = $1)",
    )
    .bind(format!("source/{}", source.as_uuid()))
    .fetch_one(&store.pool)
    .await
    .map_err(storage)
}

pub(super) async fn source_validators(
    store: &PostgresIngestStore,
    source: SourceId,
) -> Result<CacheValidators, StoreError> {
    let document =
        sqlx::query_scalar::<_, String>("SELECT document FROM content_refresh_state WHERE id = $1")
            .bind(format!("source/{}", source.as_uuid()))
            .fetch_optional(&store.pool)
            .await
            .map_err(storage)?;
    match document {
        Some(document) => decode::<SourcePollState>(&document)
            .map(|state| state.validators)
            .or_else(|_| decode(&document)),
        None => Ok(CacheValidators::default()),
    }
}

pub(super) async fn pending_poll(
    store: &PostgresIngestStore,
    source: SourceId,
    limit: usize,
) -> Result<Option<PollCommit>, StoreError> {
    store.pending_poll_batch(source, limit).await
}

pub(super) async fn active_delivery_count(
    store: &PostgresIngestStore,
    source: SourceId,
) -> Result<u64, StoreError> {
    let documents = sqlx::query_scalar::<_, String>(
        "SELECT s.document
             FROM subscription_sources AS link
             JOIN subscriptions AS s ON s.id = link.subscription_id
             WHERE link.source_id = $1",
    )
    .bind(source.as_uuid().to_string())
    .fetch_all(&store.pool)
    .await
    .map_err(storage)?;
    let mut count = 0;
    for document in documents {
        let subscription: Subscription = decode(&document)?;
        let workspace_document =
            sqlx::query_scalar::<_, String>("SELECT document FROM workspaces WHERE id = $1")
                .bind(subscription.workspace_id().as_uuid().to_string())
                .fetch_optional(&store.pool)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        let workspace: Workspace = decode(&workspace_document)?;
        if matches!(subscription.status(), SubscriptionStatus::Active)
            && workspace.accepts_delivery()
        {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) async fn commit_poll(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    commit: PollCommit,
) -> Result<(), StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
    assert_lease(&mut tx, lease).await?;
    backlog::persist(&mut tx, &commit).await?;
    let observed_at_ms = commit.fetched_at.timestamp_millis();
    let discovered_items = commit.records.len();
    let now_ms = Utc::now().timestamp_millis();
    for polled in &commit.records {
        let record = &polled.record;
        let revision = to_i64(record.revision(), "source record revision")?;
        let document = encode(record)?;
        sqlx::query(
            "INSERT INTO source_records (id, revision, document) VALUES ($1, $2, $3)
                 ON CONFLICT (id) DO UPDATE
                 SET revision = EXCLUDED.revision, document = EXCLUDED.document",
        )
        .bind(record.id().as_uuid().to_string())
        .bind(revision)
        .bind(&document)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        sqlx::query(
            "INSERT INTO source_record_identity
                 (source_id, upstream_id, record_id, observed_at_ms, revision, document)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (source_id, upstream_id) DO UPDATE
                 SET record_id = EXCLUDED.record_id, observed_at_ms = EXCLUDED.observed_at_ms,
                     revision = EXCLUDED.revision, document = EXCLUDED.document",
        )
        .bind(record.source_id().as_uuid().to_string())
        .bind(record.upstream_id())
        .bind(record.id().as_uuid().to_string())
        .bind(observed_at_ms)
        .bind(revision)
        .bind(&document)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;

        if matches!(polled.action, PollAction::Deliver | PollAction::Regroup) {
            let item = WorkItem::FanOut {
                source_id: record.source_id(),
                record_id: record.id(),
                after_subscription: None,
            };
            enqueue_work(
                &mut tx,
                record_job("fanout", record.id(), record.revision())
                    .as_uuid()
                    .to_string(),
                &item,
                now_ms,
            )
            .await?;
        }
        if let Some(url) = record.key().location.fetch_url() {
            let item = WorkItem::ExtractFullText {
                record_id: record.id(),
                source_revision: record.revision(),
                url: url.clone(),
                manual: false,
            };
            let id = record_job("fulltext", record.id(), record.revision())
                .as_uuid()
                .to_string();
            let document = encode(&item)?;
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT item FROM ingest_jobs WHERE id = $1 FOR UPDATE",
            )
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            {
                if existing != document {
                    return Err(StoreError::Conflict);
                }
            } else {
                sqlx::query(
                        "INSERT INTO ingest_jobs
                         (id, status, run_at_ms, first_attempt_ms, origin_key, attempt, item, revision)
                         VALUES ($1, 'ready', $2, $3, $4, 0, $5, 0)",
                    )
                    .bind(id)
                    .bind(initial_job_run_at(&item))
                    .bind(now_ms)
                    .bind(http_origin(url)?)
                    .bind(document)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            }
        }
    }

    let state = SourcePollState {
        validators: commit.validators,
        incomplete: commit.incomplete,
        last_success_ms: Some(observed_at_ms),
    };
    sqlx::query(
        "INSERT INTO content_refresh_state (id, revision, document) VALUES ($1, 0, $2)
             ON CONFLICT (id) DO UPDATE SET revision = 0, document = EXCLUDED.document",
    )
    .bind(format!("source/{}", commit.source_id.as_uuid()))
    .bind(encode(&state)?)
    .execute(&mut *tx)
    .await
    .map_err(storage)?;
    record_source_success_state(
        &mut tx,
        commit.source_id,
        commit.fetched_at,
        commit.incomplete,
    )
    .await?;
    record_subscription_activity(
        &mut tx,
        commit.source_id,
        true,
        Some(commit.duration_ms),
        Some(discovered_items),
        None,
        commit.fetched_at,
    )
    .await?;
    if commit.incomplete {
        let item = WorkItem::RefreshSource {
            source_id: commit.source_id,
        };
        enqueue_work(
            &mut tx,
            record_job(
                "backfill",
                commit
                    .records
                    .last()
                    .ok_or_else(|| {
                        StoreError::Unavailable("incomplete poll has no progress".into())
                    })?
                    .id(),
                commit.source_revision,
            )
            .as_uuid()
            .to_string(),
            &item,
            now_ms,
        )
        .await?;
    }
    tx.commit().await.map_err(storage)
}

pub(super) async fn record_source_success(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    source: SourceId,
    at: DateTime<Utc>,
    incomplete: bool,
    duration_ms: u64,
) -> Result<(), StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
    assert_lease(&mut tx, lease).await?;
    record_source_success_state(&mut tx, source, at, incomplete).await?;
    record_subscription_activity(&mut tx, source, true, Some(duration_ms), None, None, at).await?;
    tx.commit().await.map_err(storage)
}

pub(super) async fn record(
    store: &PostgresIngestStore,
    id: SourceRecordId,
) -> Result<SourceRecord, StoreError> {
    let document: String = sqlx::query_scalar("SELECT document FROM source_records WHERE id = $1")
        .bind(id.as_uuid().to_string())
        .fetch_optional(&store.pool)
        .await
        .map_err(storage)?
        .ok_or(StoreError::NotFound)?;
    decode(&document)
}

pub(super) async fn record_by_upstream(
    store: &PostgresIngestStore,
    source: SourceId,
    upstream_id: &str,
) -> Result<Option<SourceRecord>, StoreError> {
    sqlx::query_scalar::<_, String>(
        "SELECT document FROM source_record_identity
             WHERE source_id = $1 AND upstream_id = $2",
    )
    .bind(source.as_uuid().to_string())
    .bind(upstream_id)
    .fetch_optional(&store.pool)
    .await
    .map_err(storage)?
    .as_deref()
    .map(decode)
    .transpose()
}

pub(super) async fn record_source_success_state(
    tx: &mut Transaction<'_, Postgres>,
    source: SourceId,
    at: DateTime<Utc>,
    incomplete: bool,
) -> Result<(), StoreError> {
    let source_text = source.as_uuid().to_string();
    let prior = sqlx::query_scalar::<_, String>(
        "SELECT document FROM source_health WHERE source_id = $1 FOR UPDATE",
    )
    .bind(&source_text)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)?
    .as_deref()
    .map(decode::<SourceHealth>)
    .transpose()?;
    let health = SourceHealth {
        last_success_ms: Some(at.timestamp_millis()),
        incomplete,
        error: None,
        last_error_ms: prior.and_then(|value| value.last_error_ms),
        consecutive_failures: 0,
    };
    sqlx::query(
        "INSERT INTO source_health (source_id, document) VALUES ($1, $2)
         ON CONFLICT (source_id) DO UPDATE SET document = EXCLUDED.document, failure_since_ms = NULL",
    )
    .bind(source_text)
    .bind(encode(&health)?)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}
