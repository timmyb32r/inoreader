use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_core::*;
use reader_ingest::*;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgConnectOptions, PgPool, Postgres, Row, Transaction};

/// PostgreSQL-backed durable work queue and ingest state.
///
/// Every multi-row mutation runs in one PostgreSQL transaction. Lease tokens
/// are checked while the job row is locked, so a worker that lost its lease
/// cannot commit any dependent state.
pub struct PostgresIngestStore {
    pool: PgPool,
    poll_interval: chrono::Duration,
    per_origin_concurrency: usize,
    max_retry_age: chrono::Duration,
}

impl PostgresIngestStore {
    pub async fn connect(
        options: PgConnectOptions,
        poll_interval: chrono::Duration,
        per_origin_concurrency: usize,
        max_retry_age: chrono::Duration,
    ) -> Result<Self, StoreError> {
        let pool = PgPool::connect_with(options).await.map_err(storage)?;
        crate::prepare_schema(&pool).await.map_err(storage)?;
        Self::new(pool, poll_interval, per_origin_concurrency, max_retry_age)
    }

    pub fn new(
        pool: PgPool,
        poll_interval: chrono::Duration,
        per_origin_concurrency: usize,
        max_retry_age: chrono::Duration,
    ) -> Result<Self, StoreError> {
        if per_origin_concurrency == 0 || max_retry_age <= chrono::Duration::zero() {
            return Err(StoreError::Unavailable(
                "scheduler limits must be positive".into(),
            ));
        }
        Ok(Self {
            pool,
            poll_interval,
            per_origin_concurrency,
            max_retry_age,
        })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Reclassifies ready jobs created before priority bands were introduced.
    /// The update preserves payload, attempt, and diagnostic fields.
    pub async fn reprioritize_ready_jobs(&self) -> Result<u64, StoreError> {
        let rows = sqlx::query(
            "SELECT id, item, run_at_ms FROM ingest_jobs WHERE status = 'ready' FOR UPDATE",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut changed = 0;
        for row in rows {
            let current: i64 = row.try_get("run_at_ms").map_err(storage)?;
            let item: WorkItem = decode(row.try_get("item").map_err(storage)?)?;
            let priority = initial_job_run_at(&item);
            if current == 0 && priority != 0 {
                changed += sqlx::query("UPDATE ingest_jobs SET run_at_ms = $1 WHERE id = $2")
                    .bind(priority)
                    .bind(row.try_get::<String, _>("id").map_err(storage)?)
                    .execute(&self.pool)
                    .await
                    .map_err(storage)?
                    .rows_affected();
            }
        }
        Ok(changed)
    }

    async fn lease_update(
        &self,
        job: JobId,
        token: LeaseToken,
        status: &str,
        deadline: Option<i64>,
        run_at: Option<i64>,
        diagnostic: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query(
            "UPDATE ingest_jobs
             SET status = $1,
                 lease_deadline_ms = $2,
                 run_at_ms = COALESCE($3, run_at_ms),
                 diagnostic = $4,
                 attempt = CASE WHEN $1 = 'ready' THEN attempt + 1 ELSE attempt END,
                 revision = revision + 1
             WHERE id = $5 AND status = 'leased' AND lease_token = $6
             RETURNING item",
        )
        .bind(status)
        .bind(deadline)
        .bind(run_at)
        .bind(diagnostic)
        .bind(job.as_uuid().to_string())
        .bind(token.as_uuid().to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        let Some(row) = row else {
            return Err(StoreError::StaleLease);
        };
        if let Some(diagnostic) = diagnostic {
            let item: WorkItem = decode(row.try_get("item").map_err(storage)?)?;
            if let Some(source) = work_source(&item) {
                record_source_failure(&mut tx, source, diagnostic).await?;
            }
        }
        tx.commit().await.map_err(storage)
    }

    async fn finish(&self, job: JobId, token: LeaseToken) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query(
            "SELECT item FROM ingest_jobs
             WHERE id = $1 AND status = 'leased' AND lease_token = $2
             FOR UPDATE",
        )
        .bind(job.as_uuid().to_string())
        .bind(token.as_uuid().to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        let Some(row) = row else {
            return Err(StoreError::StaleLease);
        };
        let item: WorkItem = decode(row.try_get("item").map_err(storage)?)?;
        let recurring = is_recurring(&item);
        let next_run = millis(Utc::now() + self.poll_interval);
        sqlx::query(
            "UPDATE ingest_jobs
             SET status = $1,
                 run_at_ms = $2,
                 first_attempt_ms = CASE WHEN $3 THEN $2 ELSE first_attempt_ms END,
                 attempt = CASE WHEN $3 THEN 0 ELSE attempt END,
                 lease_token = NULL,
                 lease_deadline_ms = NULL,
                 revision = revision + 1
             WHERE id = $4",
        )
        .bind(if recurring { "ready" } else { "completed" })
        .bind(if recurring { next_run } else { 0 })
        .bind(recurring)
        .bind(job.as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
}

#[derive(Serialize, Deserialize)]
struct SourcePollState {
    validators: CacheValidators,
    incomplete: bool,
    #[serde(default)]
    last_success_ms: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct SourceHealth {
    last_success_ms: Option<i64>,
    incomplete: bool,
    error: Option<String>,
    #[serde(default)]
    last_error_ms: Option<i64>,
    #[serde(default)]
    consecutive_failures: u32,
}

pub(crate) fn retry_age_exceeded(first_attempt_ms: i64, oldest_allowed_ms: i64) -> bool {
    first_attempt_ms < oldest_allowed_ms
}

pub(crate) fn origin_has_capacity(active: usize, limit: usize) -> bool {
    active < limit
}

pub(crate) fn is_recurring(item: &WorkItem) -> bool {
    matches!(
        item,
        WorkItem::PollSource { .. } | WorkItem::CollectWebFeed { .. }
    )
}

pub(crate) fn manifest_is_current_or_newer(
    current: &ContentManifestPointer,
    candidate: &ContentRevision,
) -> bool {
    current.source_revision > candidate.source_revision
        || (current.source_revision == candidate.source_revision
            && current.fetched_at >= candidate.fetched_at)
}

fn work_source(item: &WorkItem) -> Option<SourceId> {
    match item {
        WorkItem::PollSource { source_id }
        | WorkItem::RefreshSource { source_id }
        | WorkItem::CollectWebFeed { source_id }
        | WorkItem::FanOut { source_id, .. } => Some(*source_id),
        _ => None,
    }
}

async fn assert_lease(
    tx: &mut Transaction<'_, Postgres>,
    lease: &LeasedWork,
) -> Result<(), StoreError> {
    let row = sqlx::query(
        "SELECT lease_deadline_ms FROM ingest_jobs
         WHERE id = $1 AND status = 'leased' AND lease_token = $2
         FOR UPDATE",
    )
    .bind(lease.job_id.as_uuid().to_string())
    .bind(lease.token.as_uuid().to_string())
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)?;
    let Some(row) = row else {
        return Err(StoreError::StaleLease);
    };
    let deadline: Option<i64> = row.try_get("lease_deadline_ms").map_err(storage)?;
    if deadline.is_none_or(|value| value < Utc::now().timestamp_millis()) {
        return Err(StoreError::StaleLease);
    }
    Ok(())
}

async fn source_origin(
    tx: &mut Transaction<'_, Postgres>,
    source: SourceId,
) -> Result<String, StoreError> {
    let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id = $1")
        .bind(source.as_uuid().to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage)?
        .ok_or(StoreError::NotFound)?;
    let source: SourceDefinition = decode(&document)?;
    http_origin(source.url())
}

fn http_origin(url: &url::Url) -> Result<String, StoreError> {
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(StoreError::Unavailable("job URL has no HTTP origin".into()));
    }
    Ok(url.origin().ascii_serialization())
}

async fn work_origin(
    tx: &mut Transaction<'_, Postgres>,
    item: &WorkItem,
) -> Result<String, StoreError> {
    match item {
        WorkItem::PollSource { source_id }
        | WorkItem::RefreshSource { source_id }
        | WorkItem::CollectWebFeed { source_id }
        | WorkItem::FanOut { source_id, .. } => source_origin(tx, *source_id).await,
        WorkItem::ExtractFullText { url, .. } => http_origin(url),
        WorkItem::CleanupContent { record_id, .. } => {
            let document: String =
                sqlx::query_scalar("SELECT document FROM source_records WHERE id = $1")
                    .bind(record_id.as_uuid().to_string())
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(storage)?
                    .ok_or(StoreError::NotFound)?;
            let record: SourceRecord = decode(&document)?;
            source_origin(tx, record.source_id()).await
        }
        WorkItem::EvaluateArticleRules { workspace_id, .. }
        | WorkItem::ApplyRule { workspace_id, .. } => {
            Ok(format!("internal:workspace:{}", workspace_id.as_uuid()))
        }
    }
}

async fn enqueue_work(
    tx: &mut Transaction<'_, Postgres>,
    id: String,
    item: &WorkItem,
    first_attempt_ms: i64,
) -> Result<(), StoreError> {
    let document = encode(item)?;
    if let Some(existing) =
        sqlx::query_scalar::<_, String>("SELECT item FROM ingest_jobs WHERE id = $1 FOR UPDATE")
            .bind(&id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?
    {
        return if existing == document {
            Ok(())
        } else {
            Err(StoreError::Conflict)
        };
    }
    let origin = work_origin(tx, item).await?;
    sqlx::query(
        "INSERT INTO ingest_jobs
         (id, status, run_at_ms, first_attempt_ms, origin_key, attempt, item, revision)
         VALUES ($1, 'ready', $2, $3, $4, 0, $5, 0)",
    )
    .bind(id)
    .bind(initial_job_run_at(item))
    .bind(first_attempt_ms)
    .bind(origin)
    .bind(document)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn initial_job_run_at(item: &WorkItem) -> i64 {
    match item {
        WorkItem::ExtractFullText { manual: true, .. } => -400,
        WorkItem::ExtractFullText { manual: false, .. } => -300,
        WorkItem::FanOut { .. } => -200,
        WorkItem::EvaluateArticleRules { .. } | WorkItem::ApplyRule { .. } => -100,
        WorkItem::CleanupContent { .. } => -50,
        WorkItem::PollSource { .. }
        | WorkItem::RefreshSource { .. }
        | WorkItem::CollectWebFeed { .. } => 0,
    }
}

async fn record_source_failure(
    tx: &mut Transaction<'_, Postgres>,
    source: SourceId,
    diagnostic: &str,
) -> Result<(), StoreError> {
    let key = source.as_uuid().to_string();
    let current = sqlx::query_scalar::<_, String>(
        "SELECT document FROM source_health WHERE source_id = $1 FOR UPDATE",
    )
    .bind(&key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)?;
    let mut health = current
        .as_deref()
        .map(decode)
        .transpose()?
        .unwrap_or(SourceHealth {
            last_success_ms: None,
            incomplete: false,
            error: None,
            last_error_ms: None,
            consecutive_failures: 0,
        });
    health.error = Some(diagnostic.to_owned());
    health.last_error_ms = Some(Utc::now().timestamp_millis());
    health.consecutive_failures = health.consecutive_failures.saturating_add(1);
    sqlx::query(
        "INSERT INTO source_health (source_id, document) VALUES ($1, $2)
         ON CONFLICT (source_id) DO UPDATE SET document = EXCLUDED.document",
    )
    .bind(key)
    .bind(encode(&health)?)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    record_subscription_activity(
        tx,
        source,
        false,
        None,
        None,
        Some(diagnostic.to_owned()),
        Utc::now(),
    )
    .await?;
    Ok(())
}

async fn record_subscription_activity(
    tx: &mut Transaction<'_, Postgres>,
    source: SourceId,
    successful: bool,
    duration_ms: Option<u64>,
    discovered_items: Option<usize>,
    diagnostic: Option<String>,
    occurred_at: DateTime<Utc>,
) -> Result<(), StoreError> {
    let cutoff = (occurred_at - chrono::Duration::days(30)).timestamp_millis();
    loop {
        let expired = sqlx::query(
            "SELECT subscription_id, occurred_at_ms, id FROM subscription_activity
             WHERE occurred_at_ms < $1 ORDER BY occurred_at_ms LIMIT 100 FOR UPDATE SKIP LOCKED",
        )
        .bind(cutoff)
        .fetch_all(&mut **tx)
        .await
        .map_err(storage)?;
        if expired.is_empty() {
            break;
        }
        for row in expired {
            sqlx::query(
                "DELETE FROM subscription_activity
                 WHERE subscription_id=$1 AND occurred_at_ms=$2 AND id=$3",
            )
            .bind(
                row.try_get::<String, _>("subscription_id")
                    .map_err(storage)?,
            )
            .bind(row.try_get::<i64, _>("occurred_at_ms").map_err(storage)?)
            .bind(row.try_get::<String, _>("id").map_err(storage)?)
            .execute(&mut **tx)
            .await
            .map_err(storage)?;
        }
    }
    let subscriptions = sqlx::query_scalar::<_, String>(
        "SELECT subscription_id FROM subscription_sources WHERE source_id=$1",
    )
    .bind(source.as_uuid().to_string())
    .fetch_all(&mut **tx)
    .await
    .map_err(storage)?;
    for subscription_id in subscriptions {
        let subscription_document =
            sqlx::query_scalar::<_, String>("SELECT document FROM subscriptions WHERE id=$1")
                .bind(&subscription_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        let subscription: Subscription = decode(&subscription_document)?;
        let workspace_document =
            sqlx::query_scalar::<_, String>("SELECT document FROM workspaces WHERE id=$1")
                .bind(subscription.workspace_id().as_uuid().to_string())
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        let workspace: Workspace = decode(&workspace_document)?;
        if !workspace.accepts_delivery() {
            continue;
        }
        let event = reader_application::SubscriptionActivity {
            id: uuid::Uuid::new_v4(),
            subscription_id: SubscriptionId::from_uuid(
                uuid::Uuid::parse_str(&subscription_id).map_err(storage)?,
            ),
            occurred_at,
            successful,
            duration_ms,
            discovered_items,
            diagnostic: diagnostic.clone(),
        };
        sqlx::query(
            "INSERT INTO subscription_activity(subscription_id,occurred_at_ms,id,document)
             VALUES($1,$2,$3,$4)",
        )
        .bind(&subscription_id)
        .bind(occurred_at.timestamp_millis())
        .bind(event.id.to_string())
        .bind(encode(&event)?)
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    }
    Ok(())
}

fn storage(error: impl ToString) -> StoreError {
    StoreError::Unavailable(error.to_string())
}

fn decode<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, StoreError> {
    serde_json::from_str(raw).map_err(storage)
}

fn encode<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(storage)
}

fn millis(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis()
}

fn to_i64(value: u64, field: &str) -> Result<i64, StoreError> {
    i64::try_from(value)
        .map_err(|_| StoreError::Unavailable(format!("{field} exceeds PostgreSQL BIGINT")))
}

fn to_u32(value: i64, field: &str) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| StoreError::Unavailable(format!("{field} is outside u32")))
}

fn durable_job_id(identity: &str) -> JobId {
    let hash = murmur3::murmur3_x64_128(&mut std::io::Cursor::new(identity.as_bytes()), 0)
        .expect("in-memory hashing cannot fail");
    JobId::from_uuid(uuid::Uuid::from_u128(hash))
}

pub(crate) fn record_job(kind: &str, record: SourceRecordId, revision: u64) -> JobId {
    durable_job_id(&format!("{kind}/{}/{revision}", record.as_uuid()))
}

pub(crate) fn cleanup_job(record: SourceRecordId, refresh: uuid::Uuid) -> JobId {
    durable_job_id(&format!("cleanup/{}/{refresh}", record.as_uuid()))
}

fn source_job(source: SourceId, tag: u8) -> JobId {
    durable_job_id(&format!("source/{}/{tag}", source.as_uuid()))
}

fn rule_job(rule: RuleId, version: u64, cursor: ArticleId) -> JobId {
    durable_job_id(&format!(
        "rule/{}/{version}/{}",
        rule.as_uuid(),
        cursor.as_uuid()
    ))
}

pub(crate) fn article_rule_job(
    article: ArticleId,
    subscription: SubscriptionId,
    record: SourceRecordId,
    revision: u64,
) -> JobId {
    durable_job_id(&format!(
        "article-rule/{}/{}/{}/{revision}",
        article.as_uuid(),
        subscription.as_uuid(),
        record.as_uuid()
    ))
}

#[async_trait]
impl IngestStore for PostgresIngestStore {
    async fn claim(
        &self,
        _worker: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> Result<Option<LeasedWork>, StoreError> {
        let token = LeaseToken::new();
        let now_ms = millis(now);
        let oldest_ms = millis(now - self.max_retry_age);
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let rows = sqlx::query(
            "SELECT id, item, attempt, first_attempt_ms, origin_key
             FROM ingest_jobs
             WHERE (status = 'ready' AND run_at_ms <= $1)
                OR (status = 'leased' AND lease_deadline_ms < $1)
             ORDER BY CASE WHEN status = 'ready' THEN 0 ELSE 1 END, run_at_ms, id
             LIMIT 128
             FOR UPDATE SKIP LOCKED",
        )
        .bind(now_ms)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;

        for row in rows {
            let id: String = row.try_get("id").map_err(storage)?;
            let first_attempt_ms: i64 = row.try_get("first_attempt_ms").map_err(storage)?;
            if retry_age_exceeded(first_attempt_ms, oldest_ms) {
                sqlx::query(
                    "UPDATE ingest_jobs
                     SET status = 'failed', lease_token = NULL, lease_deadline_ms = NULL,
                         diagnostic = 'maximum retry age exceeded', revision = revision + 1
                     WHERE id = $1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
                continue;
            }
            let origin: String = row.try_get("origin_key").map_err(storage)?;
            let active: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM ingest_jobs
                 WHERE status = 'leased' AND origin_key = $1 AND lease_deadline_ms >= $2",
            )
            .bind(origin)
            .bind(now_ms)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
            if !origin_has_capacity(active as usize, self.per_origin_concurrency) {
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

    async fn renew(
        &self,
        job: JobId,
        token: LeaseToken,
        lease_until: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        self.lease_update(job, token, "leased", Some(millis(lease_until)), None, None)
            .await
    }

    async fn complete(&self, job: JobId, token: LeaseToken) -> Result<(), StoreError> {
        self.finish(job, token).await
    }

    async fn retry(
        &self,
        job: JobId,
        token: LeaseToken,
        run_at: DateTime<Utc>,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        self.lease_update(
            job,
            token,
            "ready",
            None,
            Some(millis(run_at)),
            Some(diagnostic),
        )
        .await
    }

    async fn fail(
        &self,
        job: JobId,
        token: LeaseToken,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        self.lease_update(job, token, "failed", None, None, Some(diagnostic))
            .await
    }

    async fn source(&self, id: SourceId) -> Result<SourceDefinition, StoreError> {
        let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id = $1")
            .bind(id.as_uuid().to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .ok_or(StoreError::NotFound)?;
        decode(&document)
    }

    async fn has_committed_poll(&self, source: SourceId) -> Result<bool, StoreError> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM content_refresh_state WHERE id = $1)",
        )
        .bind(format!("source/{}", source.as_uuid()))
        .fetch_one(&self.pool)
        .await
        .map_err(storage)
    }

    async fn source_validators(&self, source: SourceId) -> Result<CacheValidators, StoreError> {
        let document = sqlx::query_scalar::<_, String>(
            "SELECT document FROM content_refresh_state WHERE id = $1",
        )
        .bind(format!("source/{}", source.as_uuid()))
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        match document {
            Some(document) => decode::<SourcePollState>(&document)
                .map(|state| state.validators)
                .or_else(|_| decode(&document)),
            None => Ok(CacheValidators::default()),
        }
    }

    async fn active_delivery_count(&self, source: SourceId) -> Result<u64, StoreError> {
        let documents = sqlx::query_scalar::<_, String>(
            "SELECT s.document
             FROM subscription_sources AS link
             JOIN subscriptions AS s ON s.id = link.subscription_id
             WHERE link.source_id = $1",
        )
        .bind(source.as_uuid().to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut count = 0;
        for document in documents {
            let subscription: Subscription = decode(&document)?;
            let workspace_document =
                sqlx::query_scalar::<_, String>("SELECT document FROM workspaces WHERE id = $1")
                    .bind(subscription.workspace_id().as_uuid().to_string())
                    .fetch_optional(&self.pool)
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

    async fn commit_poll(&self, lease: &LeasedWork, commit: PollCommit) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
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
                source_job(commit.source_id, 9).as_uuid().to_string(),
                &item,
                now_ms,
            )
            .await?;
        }
        tx.commit().await.map_err(storage)
    }

    async fn record_source_success(
        &self,
        lease: &LeasedWork,
        source: SourceId,
        at: DateTime<Utc>,
        incomplete: bool,
        duration_ms: u64,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        record_source_success_state(&mut tx, source, at, incomplete).await?;
        record_subscription_activity(&mut tx, source, true, Some(duration_ms), None, None, at)
            .await?;
        tx.commit().await.map_err(storage)
    }

    async fn record(&self, id: SourceRecordId) -> Result<SourceRecord, StoreError> {
        let document: String =
            sqlx::query_scalar("SELECT document FROM source_records WHERE id = $1")
                .bind(id.as_uuid().to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        decode(&document)
    }

    async fn record_by_upstream(
        &self,
        source: SourceId,
        upstream_id: &str,
    ) -> Result<Option<SourceRecord>, StoreError> {
        sqlx::query_scalar::<_, String>(
            "SELECT document FROM source_record_identity
             WHERE source_id = $1 AND upstream_id = $2",
        )
        .bind(source.as_uuid().to_string())
        .bind(upstream_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .as_deref()
        .map(decode)
        .transpose()
    }

    async fn delivery_targets(
        &self,
        source: SourceId,
        after: Option<SubscriptionId>,
        limit: usize,
    ) -> Result<Vec<DeliveryTarget>, StoreError> {
        let after = after.map(|id| id.as_uuid().to_string()).unwrap_or_default();
        let rows = sqlx::query(
            "SELECT s.document
             FROM subscription_sources AS link
             JOIN subscriptions AS s ON s.id = link.subscription_id
             WHERE link.source_id = $1 AND link.subscription_id > $2
             ORDER BY link.subscription_id
             LIMIT $3",
        )
        .bind(source.as_uuid().to_string())
        .bind(after)
        .bind(i64::try_from(limit).map_err(storage)?)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut targets = Vec::with_capacity(rows.len());
        for row in rows {
            let subscription: Subscription = decode(row.try_get("document").map_err(storage)?)?;
            if !matches!(subscription.status(), SubscriptionStatus::Active) {
                continue;
            }
            let workspace_document =
                sqlx::query_scalar::<_, String>("SELECT document FROM workspaces WHERE id = $1")
                    .bind(subscription.workspace_id().as_uuid().to_string())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(storage)?
                    .ok_or(StoreError::NotFound)?;
            let workspace: Workspace = decode(&workspace_document)?;
            if workspace.accepts_delivery() {
                targets.push(DeliveryTarget {
                    workspace_id: subscription.workspace_id(),
                    subscription_id: subscription.id(),
                });
            }
        }
        Ok(targets)
    }

    async fn deliver(
        &self,
        lease: &LeasedWork,
        commit: DeliveryCommit,
    ) -> Result<DeliveryResult, StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        let workspace_id = commit.target.workspace_id;
        let workspace = workspace_id.as_uuid().to_string();
        let subscription = commit.target.subscription_id.as_uuid().to_string();
        let sub_document: String =
            sqlx::query_scalar("SELECT document FROM subscriptions WHERE id = $1 FOR UPDATE")
                .bind(&subscription)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        let sub_value: Subscription = decode(&sub_document)?;
        let workspace_document: String =
            sqlx::query_scalar("SELECT document FROM workspaces WHERE id = $1 FOR UPDATE")
                .bind(&workspace)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        let workspace_value: Workspace = decode(&workspace_document)?;
        if sub_value.workspace_id() != workspace_id || workspace_value.id() != workspace_id {
            return Err(StoreError::NotFound);
        }
        if !matches!(sub_value.status(), SubscriptionStatus::Active)
            || !workspace_value.accepts_delivery()
        {
            tx.commit().await.map_err(storage)?;
            return Ok(DeliveryResult::SkippedInactive);
        }

        let record = commit.record;
        let origin_rows = sqlx::query(
            "SELECT article_id, subscription_id FROM library_origins
             WHERE workspace_id = $1 AND source_record_id = $2
             FOR UPDATE",
        )
        .bind(&workspace)
        .bind(record.id().as_uuid().to_string())
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let mut prior_by_article = std::collections::BTreeMap::<String, Vec<String>>::new();
        for row in origin_rows {
            prior_by_article
                .entry(row.try_get("article_id").map_err(storage)?)
                .or_default()
                .push(row.try_get("subscription_id").map_err(storage)?);
        }
        let mut inherited = Vec::new();
        let mut moved_articles = Vec::new();
        let mut moved_subscriptions = Vec::new();
        for (old_id, subscriptions) in prior_by_article {
            let old_key = format!("{workspace}/{old_id}");
            let old_document: String =
                sqlx::query_scalar("SELECT document FROM articles WHERE id = $1 FOR UPDATE")
                    .bind(&old_key)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
                    .ok_or_else(|| {
                        StoreError::Unavailable(
                            "library origin references a missing article".into(),
                        )
                    })?;
            let mut old: Article = decode(&old_document)?;
            if old.key == *record.key() {
                continue;
            }
            moved_articles.push(old.id);
            inherited.push((old.state, old.first_arrived_at));
            old.detach_origin(record.id());
            sqlx::query(
                "DELETE FROM library_origins
                 WHERE workspace_id = $1 AND article_id = $2 AND source_record_id = $3",
            )
            .bind(&workspace)
            .bind(&old_id)
            .bind(record.id().as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            for linked in subscriptions {
                if !moved_subscriptions.contains(&linked) {
                    moved_subscriptions.push(linked);
                }
            }
            let old_dedup = encode(&old.key)?;
            if old.origins.is_empty() {
                sqlx::query("DELETE FROM articles WHERE id = $1")
                    .bind(old_key)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                sqlx::query("DELETE FROM library_dedup WHERE workspace_id = $1 AND dedup_key = $2")
                    .bind(&workspace)
                    .bind(old_dedup)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            } else {
                upsert_article(&mut tx, &workspace, &old, &old_dedup).await?;
            }
        }

        let dedup_key = encode(record.key())?;
        let found = sqlx::query_scalar::<_, String>(
            "SELECT document FROM library_dedup
             WHERE workspace_id = $1 AND dedup_key = $2 FOR UPDATE",
        )
        .bind(&workspace)
        .bind(&dedup_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .as_deref()
        .map(decode::<Article>)
        .transpose()?;
        let inherited_time = inherited.iter().map(|(_, time)| *time).min();
        let inherited_state = ArticleState::merge(inherited.into_iter().map(|(state, _)| state));
        let found = match (found, inherited_state) {
            (Some(mut article), Some(state)) => {
                article.state = ArticleState::merge([article.state, state]).expect("two states");
                if let Some(time) = inherited_time {
                    article.first_arrived_at = article.first_arrived_at.min(time);
                }
                Some(article)
            }
            (Some(article), None) => Some(article),
            (None, Some(state)) => Some(Article {
                id: commit.proposed_article_id,
                key: record.key().clone(),
                state,
                first_arrived_at: inherited_time.unwrap_or(commit.delivered_at),
                origins: vec![],
                revision: 0,
            }),
            (None, None) => None,
        };
        let (mut article, result) = match found {
            Some(article) => {
                let id = article.id;
                (article, DeliveryResult::AlreadyDelivered(id))
            }
            None => (
                Article {
                    id: commit.proposed_article_id,
                    key: record.key().clone(),
                    state: ArticleState::default(),
                    first_arrived_at: commit.delivered_at,
                    origins: vec![],
                    revision: 0,
                },
                DeliveryResult::Delivered(commit.proposed_article_id),
            ),
        };
        article.attach_origin(record.id());
        upsert_article(&mut tx, &workspace, &article, &dedup_key).await?;

        if !moved_subscriptions.contains(&subscription) {
            moved_subscriptions.push(subscription);
        }
        for linked in &moved_subscriptions {
            sqlx::query(
                "INSERT INTO library_origins
                 (workspace_id, article_id, subscription_id, source_record_id)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT DO NOTHING",
            )
            .bind(&workspace)
            .bind(article.id.as_uuid().to_string())
            .bind(linked)
            .bind(record.id().as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }

        migrate_rule_provenance(&mut tx, &workspace, article.id, &moved_articles).await?;
        enqueue_article_rule_jobs(
            &mut tx,
            workspace_id,
            &sub_value,
            &article,
            &record,
            moved_subscriptions,
        )
        .await?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }

    async fn advance_fanout(
        &self,
        lease: &LeasedWork,
        after: SubscriptionId,
    ) -> Result<(), StoreError> {
        let WorkItem::FanOut {
            source_id,
            record_id,
            ..
        } = lease.item
        else {
            return Ok(());
        };
        let item = WorkItem::FanOut {
            source_id,
            record_id,
            after_subscription: Some(after),
        };
        let id = durable_job_id(&format!(
            "fanout-continuation/{}/{}",
            lease.job_id.as_uuid(),
            after.as_uuid()
        ));
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        enqueue_work(
            &mut tx,
            id.as_uuid().to_string(),
            &item,
            Utc::now().timestamp_millis(),
        )
        .await?;
        tx.commit().await.map_err(storage)
    }

    async fn publish_content(
        &self,
        lease: &LeasedWork,
        revision: ContentRevision,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        let record = revision.record_id.as_uuid().to_string();
        let current = sqlx::query_scalar::<_, String>(
            "SELECT document FROM content_manifests WHERE id = $1 FOR UPDATE",
        )
        .bind(&record)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .as_deref()
        .map(decode::<ContentManifestPointer>)
        .transpose()?;
        if current
            .as_ref()
            .is_some_and(|value| manifest_is_current_or_newer(value, &revision))
        {
            tx.commit().await.map_err(storage)?;
            return Ok(());
        }
        for (representation, chunks) in [
            ("raw", &revision.raw_chunks),
            ("safe", &revision.safe_html_chunks),
        ] {
            for chunk in chunks {
                sqlx::query(
                    "INSERT INTO staged_content_chunks
                     (record_id, refresh_id, representation, ordinal, bytes)
                     VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (record_id, refresh_id, representation, ordinal)
                     DO UPDATE SET bytes = EXCLUDED.bytes",
                )
                .bind(&record)
                .bind(revision.refresh_id.to_string())
                .bind(representation)
                .bind(i64::from(chunk.ordinal))
                .bind(encode(&chunk.bytes)?)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            }
        }
        let pointer = ContentManifestPointer::from(&revision);
        sqlx::query(
            "INSERT INTO content_manifests (id, revision, document) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE
             SET revision = EXCLUDED.revision, document = EXCLUDED.document",
        )
        .bind(&record)
        .bind(to_i64(revision.source_revision, "content source revision")?)
        .bind(encode(&pointer)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        sqlx::query("DELETE FROM content_refresh_state WHERE id = $1")
            .bind(format!("failure/{record}"))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let cleanup = WorkItem::CleanupContent {
            record_id: revision.record_id,
            keep_refresh_id: revision.refresh_id,
        };
        enqueue_work(
            &mut tx,
            cleanup_job(revision.record_id, revision.refresh_id)
                .as_uuid()
                .to_string(),
            &cleanup,
            Utc::now().timestamp_millis(),
        )
        .await?;
        tx.commit().await.map_err(storage)
    }

    async fn cleanup_content(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        _requested_keep: uuid::Uuid,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        let manifest: String =
            sqlx::query_scalar("SELECT document FROM content_manifests WHERE id = $1 FOR UPDATE")
                .bind(record.as_uuid().to_string())
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or_else(|| {
                    StoreError::Unavailable("content cleanup has no current manifest".into())
                })?;
        let current: ContentManifestPointer = decode(&manifest)?;
        sqlx::query("DELETE FROM staged_content_chunks WHERE record_id = $1 AND refresh_id <> $2")
            .bind(record.as_uuid().to_string())
            .bind(current.refresh_id.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }

    async fn record_refresh_failure(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        let key = format!("failure/{}", record.as_uuid());
        sqlx::query(
            "INSERT INTO content_refresh_state (id, revision, document)
             VALUES ($1, 1, $2)
             ON CONFLICT (id) DO UPDATE
             SET revision = content_refresh_state.revision + 1,
                 document = EXCLUDED.document",
        )
        .bind(key)
        .bind(encode(&serde_json::json!({
            "diagnostic": diagnostic,
            "job": lease.job_id.as_uuid().to_string(),
        }))?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }

    async fn evaluate_article_rules(
        &self,
        lease: &LeasedWork,
        workspace: WorkspaceId,
        article: ArticleId,
        evaluations: &[PendingRuleEvaluation],
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        let workspace_text = workspace.as_uuid().to_string();
        let article_text = article.as_uuid().to_string();
        let key = format!("{workspace_text}/{article_text}");
        let Some(document) = sqlx::query_scalar::<_, String>(
            "SELECT document FROM articles WHERE id = $1 FOR UPDATE",
        )
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        else {
            tx.commit().await.map_err(storage)?;
            return Ok(());
        };
        let mut article: Article = decode(&document)?;
        let original_state = article.state;
        let (full_text, generations) = article_full_text(&mut tx, &article).await?;
        for pending in evaluations {
            let rule_key = format!("{workspace_text}/{}", pending.rule_id.as_uuid());
            let Some(rule_document) =
                sqlx::query_scalar::<_, String>("SELECT document FROM rules WHERE id = $1")
                    .bind(rule_key)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
            else {
                continue;
            };
            let rule: Rule = decode(&rule_document)?;
            rule.validate().map_err(storage)?;
            if !pending.still_valid_for(&rule) {
                continue;
            }
            ensure_rule_content(&rule, &article, full_text.as_deref())?;
            let matched = rule.matches(&article.key.title, full_text.as_deref());
            let before = article.state;
            if matched {
                rule.apply(&mut article.state);
            }
            let reason = encode(&serde_json::json!({
                "matched": matched,
                "action": rule.action,
                "state_changed": before != article.state,
                "manual_override_preserved": matched && before == article.state,
                "content_generations": generations,
            }))?;
            upsert_rule_evaluation(
                &mut tx,
                &workspace_text,
                &article_text,
                rule.id,
                rule.version,
                &reason,
            )
            .await?;
        }
        if article.state != original_state {
            article.revision = article.revision.saturating_add(1);
            sqlx::query("UPDATE articles SET revision = $1, document = $2 WHERE id = $3")
                .bind(to_i64(article.revision, "article revision")?)
                .bind(encode(&article)?)
                .bind(key)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply_rule_batch(
        &self,
        lease: &LeasedWork,
        workspace_id: WorkspaceId,
        rule_id: RuleId,
        version: u64,
        after_id: Option<ArticleId>,
        through_id: Option<ArticleId>,
        limit: usize,
    ) -> Result<Option<ArticleId>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        assert_lease(&mut tx, lease).await?;
        let workspace = workspace_id.as_uuid().to_string();
        let rule_text = rule_id.as_uuid().to_string();
        let Some(rule_document) =
            sqlx::query_scalar::<_, String>("SELECT document FROM rules WHERE id = $1 FOR UPDATE")
                .bind(format!("{workspace}/{rule_text}"))
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
        else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let rule: Rule = decode(&rule_document)?;
        rule.validate().map_err(storage)?;
        if !rule.enabled || rule.version != version {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        }
        let prefix = format!("{workspace}/");
        let upper = format!("{workspace}0");
        let after = after_id
            .map(|value| format!("{workspace}/{}", value.as_uuid()))
            .unwrap_or_else(|| prefix.clone());
        let through = match through_id {
            Some(value) => value,
            None => {
                let boundary = sqlx::query_scalar::<_, String>(
                    "SELECT id FROM articles
                     WHERE id > $1 AND id < $2 ORDER BY id DESC LIMIT 1",
                )
                .bind(&prefix)
                .bind(&upper)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
                let Some(boundary) = boundary else {
                    tx.commit().await.map_err(storage)?;
                    return Ok(None);
                };
                ArticleId::from_uuid(
                    uuid::Uuid::parse_str(boundary.rsplit('/').next().unwrap_or_default())
                        .map_err(storage)?,
                )
            }
        };
        let rows = sqlx::query(
            "SELECT id, document FROM articles
             WHERE id > $1 AND id <= $2 ORDER BY id LIMIT $3 FOR UPDATE",
        )
        .bind(after)
        .bind(format!("{workspace}/{}", through.as_uuid()))
        .bind(i64::try_from(limit).map_err(storage)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(storage)?;
        let count = rows.len();
        let mut last = None;
        for row in rows {
            let key: String = row.try_get("id").map_err(storage)?;
            let mut article: Article = decode(row.try_get("document").map_err(storage)?)?;
            let before = article.state;
            let (full_text, _) = article_full_text(&mut tx, &article).await?;
            ensure_rule_content(&rule, &article, full_text.as_deref())?;
            let matched = rule.matches(&article.key.title, full_text.as_deref());
            if matched {
                rule.apply(&mut article.state);
            }
            if article.state != before {
                article.revision = article.revision.saturating_add(1);
                sqlx::query("UPDATE articles SET revision = $1, document = $2 WHERE id = $3")
                    .bind(to_i64(article.revision, "article revision")?)
                    .bind(encode(&article)?)
                    .bind(key)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            }
            let reason = encode(&serde_json::json!({
                "matched": matched,
                "action": rule.action,
                "state_changed": before != article.state,
                "manual_override_preserved": matched && before == article.state,
            }))?;
            upsert_rule_evaluation(
                &mut tx,
                &workspace,
                &article.id.as_uuid().to_string(),
                rule.id,
                version,
                &reason,
            )
            .await?;
            last = Some(article.id);
        }
        if count == limit {
            if let Some(cursor) = last {
                if cursor != through {
                    let next = WorkItem::ApplyRule {
                        workspace_id,
                        rule_id: rule.id,
                        rule_version: version,
                        after_article: Some(cursor),
                        through_article: Some(through),
                    };
                    enqueue_work(
                        &mut tx,
                        rule_job(rule.id, version, cursor).as_uuid().to_string(),
                        &next,
                        Utc::now().timestamp_millis(),
                    )
                    .await?;
                }
            }
        }
        tx.commit().await.map_err(storage)?;
        Ok(last)
    }
}

async fn record_source_success_state(
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
         ON CONFLICT (source_id) DO UPDATE SET document = EXCLUDED.document",
    )
    .bind(source_text)
    .bind(encode(&health)?)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}

async fn upsert_article(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    article: &Article,
    dedup_key: &str,
) -> Result<(), StoreError> {
    let document = encode(article)?;
    let revision = to_i64(article.revision, "article revision")?;
    sqlx::query(
        "INSERT INTO articles (id, revision, document) VALUES ($1, $2, $3)
         ON CONFLICT (id) DO UPDATE
         SET revision = EXCLUDED.revision, document = EXCLUDED.document",
    )
    .bind(format!("{workspace}/{}", article.id.as_uuid()))
    .bind(revision)
    .bind(&document)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    sqlx::query(
        "INSERT INTO library_dedup
         (workspace_id, dedup_key, article_id, revision, document)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (workspace_id, dedup_key) DO UPDATE
         SET article_id = EXCLUDED.article_id, revision = EXCLUDED.revision,
             document = EXCLUDED.document",
    )
    .bind(workspace)
    .bind(dedup_key)
    .bind(article.id.as_uuid().to_string())
    .bind(revision)
    .bind(document)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}

async fn migrate_rule_provenance(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    article: ArticleId,
    moved_articles: &[ArticleId],
) -> Result<(), StoreError> {
    for old_article in moved_articles {
        if *old_article == article {
            continue;
        }
        let rows = sqlx::query(
            "SELECT rule_id, rule_version, document FROM rule_evaluations
             WHERE workspace_id = $1 AND article_id = $2 FOR UPDATE",
        )
        .bind(workspace)
        .bind(old_article.as_uuid().to_string())
        .fetch_all(&mut **tx)
        .await
        .map_err(storage)?;
        for row in rows {
            let rule_id: String = row.try_get("rule_id").map_err(storage)?;
            let version: i64 = row.try_get("rule_version").map_err(storage)?;
            let old_document: String = row.try_get("document").map_err(storage)?;
            let existing = sqlx::query_scalar::<_, String>(
                "SELECT document FROM rule_evaluations
                 WHERE workspace_id = $1 AND article_id = $2
                   AND rule_id = $3 AND rule_version = $4 FOR UPDATE",
            )
            .bind(workspace)
            .bind(article.as_uuid().to_string())
            .bind(&rule_id)
            .bind(version)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?;
            let document = match existing {
                Some(current) => encode(&serde_json::json!({
                    "merged_provenance": [
                        serde_json::from_str::<serde_json::Value>(&current).map_err(storage)?,
                        serde_json::from_str::<serde_json::Value>(&old_document).map_err(storage)?,
                    ],
                }))?,
                None => old_document,
            };
            sqlx::query(
                "INSERT INTO rule_evaluations
                 (workspace_id, article_id, rule_id, rule_version, document)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (workspace_id, article_id, rule_id, rule_version)
                 DO UPDATE SET document = EXCLUDED.document",
            )
            .bind(workspace)
            .bind(article.as_uuid().to_string())
            .bind(&rule_id)
            .bind(version)
            .bind(document)
            .execute(&mut **tx)
            .await
            .map_err(storage)?;
            sqlx::query(
                "DELETE FROM rule_evaluations
                 WHERE workspace_id = $1 AND article_id = $2
                   AND rule_id = $3 AND rule_version = $4",
            )
            .bind(workspace)
            .bind(old_article.as_uuid().to_string())
            .bind(rule_id)
            .bind(version)
            .execute(&mut **tx)
            .await
            .map_err(storage)?;
        }
    }
    Ok(())
}

async fn enqueue_article_rule_jobs(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    subscription: &Subscription,
    article: &Article,
    record: &SourceRecord,
    subscriptions: Vec<String>,
) -> Result<(), StoreError> {
    let prefix = format!("{}/", workspace.as_uuid());
    let upper = format!("{}0", workspace.as_uuid());
    let documents =
        sqlx::query_scalar::<_, String>("SELECT document FROM rules WHERE id >= $1 AND id < $2")
            .bind(prefix)
            .bind(upper)
            .fetch_all(&mut **tx)
            .await
            .map_err(storage)?;
    let mut rules = Vec::new();
    for document in documents {
        let rule: Rule = decode(&document)?;
        rule.validate().map_err(storage)?;
        if rule.enabled {
            rules.push(rule);
        }
    }
    for linked in subscriptions {
        let linked = SubscriptionId::from_uuid(uuid::Uuid::parse_str(&linked).map_err(storage)?);
        let evaluations = rules
            .iter()
            .filter(|rule| rule.subscription_id == linked)
            .map(|rule| PendingRuleEvaluation {
                rule_id: rule.id,
                rule_version: rule.version,
                subscription_id: rule.subscription_id,
            })
            .collect::<Vec<_>>();
        if evaluations.is_empty() {
            continue;
        }
        let item = WorkItem::EvaluateArticleRules {
            workspace_id: subscription.workspace_id(),
            article_id: article.id,
            evaluations,
        };
        enqueue_work(
            tx,
            article_rule_job(article.id, linked, record.id(), record.revision())
                .as_uuid()
                .to_string(),
            &item,
            Utc::now().timestamp_millis(),
        )
        .await?;
    }
    Ok(())
}

async fn article_full_text(
    tx: &mut Transaction<'_, Postgres>,
    article: &Article,
) -> Result<(Option<String>, Vec<uuid::Uuid>), StoreError> {
    let mut text = String::new();
    let mut generations = Vec::new();
    for origin in &article.origins {
        let Some(document) =
            sqlx::query_scalar::<_, String>("SELECT document FROM content_manifests WHERE id = $1")
                .bind(origin.as_uuid().to_string())
                .fetch_optional(&mut **tx)
                .await
                .map_err(storage)?
        else {
            continue;
        };
        let manifest: ContentManifestPointer = decode(&document)?;
        let chunks = sqlx::query_scalar::<_, String>(
            "SELECT bytes FROM staged_content_chunks
             WHERE record_id = $1 AND refresh_id = $2 AND representation = 'safe'
             ORDER BY ordinal",
        )
        .bind(origin.as_uuid().to_string())
        .bind(manifest.refresh_id.to_string())
        .fetch_all(&mut **tx)
        .await
        .map_err(storage)?;
        for chunk in chunks {
            let bytes: Vec<u8> = decode(&chunk)?;
            text.push_str(std::str::from_utf8(&bytes).map_err(storage)?);
        }
        generations.push(manifest.refresh_id);
    }
    Ok(((!text.is_empty()).then_some(text), generations))
}

fn ensure_rule_content(
    rule: &Rule,
    article: &Article,
    full_text: Option<&str>,
) -> Result<(), StoreError> {
    let title_match = rule.matches(&article.key.title, None);
    if full_text.is_none()
        && (matches!(rule.field, RuleField::Text)
            || (matches!(rule.field, RuleField::Both) && !title_match))
    {
        return Err(StoreError::Unavailable(
            "rule evaluation awaits fulltext".into(),
        ));
    }
    Ok(())
}

async fn upsert_rule_evaluation(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    article: &str,
    rule: RuleId,
    version: u64,
    document: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO rule_evaluations
         (workspace_id, article_id, rule_id, rule_version, document)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (workspace_id, article_id, rule_id, rule_version)
         DO UPDATE SET document = EXCLUDED.document",
    )
    .bind(workspace)
    .bind(article)
    .bind(rule.as_uuid().to_string())
    .bind(to_i64(version, "rule version")?)
    .bind(document)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}
