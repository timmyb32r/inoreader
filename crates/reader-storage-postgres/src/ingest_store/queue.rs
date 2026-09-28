//! PostgreSQL queue operations. Transaction boundaries stay local and explicit.
use super::*;

pub(super) async fn claim(
    store: &PostgresIngestStore,
    _worker: &str,
    now: DateTime<Utc>,
    lease_until: DateTime<Utc>,
) -> Result<Option<LeasedWork>, StoreError> {
    let token = LeaseToken::new();
    let now_ms = millis(now);
    let oldest_ms = millis(now - store.max_retry_age);
    let mut tx = store.pool.begin().await.map_err(storage)?;
    // Origin reservations are transaction-scoped. A hash collision only
    // serializes admission; hashes never stand in for source identity.
    let mut excluded = Vec::<String>::new();
    loop {
        let row = sqlx::query(
                "SELECT j.id,j.item,j.attempt,j.first_attempt_ms,j.origin_key
                 FROM ingest_jobs j
                 WHERE ((j.status='ready' AND j.run_at_ms <= $1)
                    OR (j.status='leased' AND j.lease_deadline_ms < $1))
                 AND NOT (j.origin_key = ANY($2))
                 AND (SELECT count(*) FROM ingest_jobs a WHERE a.status='leased'
                      AND a.origin_key=j.origin_key AND a.lease_deadline_ms >= $1) < $3
                 AND NOT EXISTS (
                    SELECT 1 FROM ingest_jobs a WHERE a.status='leased' AND a.lease_deadline_ms >= $1
                    AND COALESCE(a.item::jsonb#>>'{PollSource,source_id}',a.item::jsonb#>>'{RefreshSource,source_id}',a.item::jsonb#>>'{CollectWebFeed,source_id}')
                      = COALESCE(j.item::jsonb#>>'{PollSource,source_id}',j.item::jsonb#>>'{RefreshSource,source_id}',j.item::jsonb#>>'{CollectWebFeed,source_id}'))
                 ORDER BY CASE WHEN j.status='ready' THEN 0 ELSE 1 END,j.run_at_ms,j.id
                 LIMIT 1 FOR UPDATE OF j SKIP LOCKED",
            ).bind(now_ms).bind(&excluded)
             .bind(i64::try_from(store.per_origin_concurrency).map_err(storage)?)
             .fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            break;
        };
        let origin: String = row.try_get("origin_key").map_err(storage)?;
        let reserved: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(1919246692,hashtext($1))")
                .bind(&origin)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if !reserved {
            excluded.push(origin);
            continue;
        }
        let id: String = row.try_get("id").map_err(storage)?;
        let first_attempt_ms: i64 = row.try_get("first_attempt_ms").map_err(storage)?;
        if retry_age_exceeded(first_attempt_ms, oldest_ms) {
            let item: WorkItem = decode(row.try_get("item").map_err(storage)?)?;
            let recurring = is_recurring(&item);
            let next_run = millis(now + store.poll_interval);
            sqlx::query(
                "UPDATE ingest_jobs
                 SET status=$2,lease_token=NULL,lease_deadline_ms=NULL,
                     diagnostic='maximum retry age exceeded',revision=revision+1,
                     run_at_ms=CASE WHEN $3 THEN $4 ELSE run_at_ms END,
                     first_attempt_ms=CASE WHEN $3 THEN $4 ELSE first_attempt_ms END,
                     attempt=CASE WHEN $3 THEN 0 ELSE attempt END
                 WHERE id=$1",
            )
            .bind(&id)
            .bind(if recurring { "ready" } else { "failed" })
            .bind(recurring)
            .bind(next_run)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            if let Some(source) = work_source(&item) {
                record_source_failure(&mut tx, source, "maximum retry age exceeded").await?;
            }
            continue;
        }
        let origin: String = row.try_get("origin_key").map_err(storage)?;
        let active: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ingest_jobs
                 WHERE status = 'leased' AND origin_key = $1 AND lease_deadline_ms >= $2",
        )
        .bind(&origin)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if !origin_has_capacity(active as usize, store.per_origin_concurrency) {
            excluded.push(origin);
            continue;
        }
        let source_busy: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM ingest_jobs a,ingest_jobs j WHERE j.id=$1
                 AND a.status='leased' AND a.lease_deadline_ms >= $2
                 AND COALESCE(a.item::jsonb#>>'{PollSource,source_id}',a.item::jsonb#>>'{RefreshSource,source_id}',a.item::jsonb#>>'{CollectWebFeed,source_id}')
                   = COALESCE(j.item::jsonb#>>'{PollSource,source_id}',j.item::jsonb#>>'{RefreshSource,source_id}',j.item::jsonb#>>'{CollectWebFeed,source_id}'))")
                .bind(&id).bind(now_ms).fetch_one(&mut *tx).await.map_err(storage)?;
        if source_busy {
            excluded.push(origin);
            continue;
        }
        sqlx::query(
            "UPDATE ingest_jobs
                 SET status = 'leased', lease_token = $1, lease_deadline_ms = $2,
                     revision = revision + 1
                 WHERE id = $3",
        )
        .bind(token.as_uuid().to_string())
        .bind(millis(lease_until))
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        let item: WorkItem = decode(row.try_get("item").map_err(storage)?)?;
        let attempt = to_u32(row.try_get("attempt").map_err(storage)?, "job attempt")?;
        let job_id = JobId::from_uuid(uuid::Uuid::parse_str(&id).map_err(storage)?);
        tx.commit().await.map_err(storage)?;
        return Ok(Some(LeasedWork {
            job_id,
            item,
            token,
            deadline: lease_until,
            attempt,
        }));
    }
    tx.commit().await.map_err(storage)?;
    Ok(None)
}

pub(super) async fn renew(
    store: &PostgresIngestStore,
    job: JobId,
    token: LeaseToken,
    lease_until: DateTime<Utc>,
) -> Result<(), StoreError> {
    store
        .lease_update(job, token, "leased", Some(millis(lease_until)), None, None)
        .await
}

pub(super) async fn complete(
    store: &PostgresIngestStore,
    job: JobId,
    token: LeaseToken,
) -> Result<(), StoreError> {
    store.finish(job, token).await
}

pub(super) async fn retry(
    store: &PostgresIngestStore,
    job: JobId,
    token: LeaseToken,
    run_at: DateTime<Utc>,
    diagnostic: &str,
) -> Result<(), StoreError> {
    store
        .lease_update(
            job,
            token,
            "ready",
            None,
            Some(millis(run_at)),
            Some(diagnostic),
        )
        .await
}

pub(super) async fn fail(
    store: &PostgresIngestStore,
    job: JobId,
    token: LeaseToken,
    diagnostic: &str,
    not_before: Option<DateTime<Utc>>,
) -> Result<(), StoreError> {
    store
        .lease_update(
            job,
            token,
            "failed",
            None,
            not_before.map(millis),
            Some(diagnostic),
        )
        .await
}
