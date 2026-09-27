mod content;
mod delivery;
mod poll;
mod queue;
mod rules;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_core::*;
use reader_ingest::*;
use rules::enqueue_article_rule_jobs;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgConnectOptions, PgPool, Postgres, Row, Transaction};
mod backlog;

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
        validate_scheduler_limits(poll_interval, per_origin_concurrency, max_retry_age)?;
        let pool = PgPool::connect_with(crate::instrument_postgres(options))
            .await
            .map_err(storage)?;
        crate::verify_schema(&pool).await.map_err(storage)?;
        Self::new(pool, poll_interval, per_origin_concurrency, max_retry_age)
    }

    pub fn new(
        pool: PgPool,
        poll_interval: chrono::Duration,
        per_origin_concurrency: usize,
        max_retry_age: chrono::Duration,
    ) -> Result<Self, StoreError> {
        validate_scheduler_limits(poll_interval, per_origin_concurrency, max_retry_age)?;
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
               AND lease_deadline_ms >= (extract(epoch FROM clock_timestamp()) * 1000)::bigint
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

pub(crate) fn dedup_fingerprint(dedup_key: &str) -> String {
    let hash = murmur3::murmur3_x64_128(&mut std::io::Cursor::new(dedup_key.as_bytes()), 0)
        .expect("in-memory hashing cannot fail");
    format!("{hash:032x}")
}

pub(crate) fn record_job(kind: &str, record: SourceRecordId, revision: u64) -> JobId {
    durable_job_id(&format!("{kind}/{}/{revision}", record.as_uuid()))
}

pub(crate) fn cleanup_job(record: SourceRecordId, refresh: uuid::Uuid) -> JobId {
    durable_job_id(&format!("cleanup/{}/{refresh}", record.as_uuid()))
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
        queue::claim(self, _worker, now, lease_until).await
    }

    async fn renew(
        &self,
        job: JobId,
        token: LeaseToken,
        lease_until: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        queue::renew(self, job, token, lease_until).await
    }

    async fn complete(&self, job: JobId, token: LeaseToken) -> Result<(), StoreError> {
        queue::complete(self, job, token).await
    }

    async fn retry(
        &self,
        job: JobId,
        token: LeaseToken,
        run_at: DateTime<Utc>,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        queue::retry(self, job, token, run_at, diagnostic).await
    }

    async fn fail(
        &self,
        job: JobId,
        token: LeaseToken,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        queue::fail(self, job, token, diagnostic).await
    }

    async fn source(&self, id: SourceId) -> Result<SourceDefinition, StoreError> {
        poll::source(self, id).await
    }

    async fn has_committed_poll(&self, source: SourceId) -> Result<bool, StoreError> {
        poll::has_committed_poll(self, source).await
    }

    async fn source_validators(&self, source: SourceId) -> Result<CacheValidators, StoreError> {
        poll::source_validators(self, source).await
    }
    async fn pending_poll(
        &self,
        source: SourceId,
        limit: usize,
    ) -> Result<Option<PollCommit>, StoreError> {
        poll::pending_poll(self, source, limit).await
    }

    async fn active_delivery_count(&self, source: SourceId) -> Result<u64, StoreError> {
        poll::active_delivery_count(self, source).await
    }

    async fn commit_poll(&self, lease: &LeasedWork, commit: PollCommit) -> Result<(), StoreError> {
        poll::commit_poll(self, lease, commit).await
    }

    async fn record_source_success(
        &self,
        lease: &LeasedWork,
        source: SourceId,
        at: DateTime<Utc>,
        incomplete: bool,
        duration_ms: u64,
    ) -> Result<(), StoreError> {
        poll::record_source_success(self, lease, source, at, incomplete, duration_ms).await
    }

    async fn record(&self, id: SourceRecordId) -> Result<SourceRecord, StoreError> {
        poll::record(self, id).await
    }

    async fn record_by_upstream(
        &self,
        source: SourceId,
        upstream_id: &str,
    ) -> Result<Option<SourceRecord>, StoreError> {
        poll::record_by_upstream(self, source, upstream_id).await
    }

    async fn delivery_targets(
        &self,
        source: SourceId,
        after: Option<SubscriptionId>,
        limit: usize,
    ) -> Result<Vec<DeliveryTarget>, StoreError> {
        delivery::delivery_targets(self, source, after, limit).await
    }

    async fn deliver(
        &self,
        lease: &LeasedWork,
        commit: DeliveryCommit,
    ) -> Result<DeliveryResult, StoreError> {
        delivery::deliver(self, lease, commit).await
    }

    async fn advance_fanout(
        &self,
        lease: &LeasedWork,
        after: SubscriptionId,
    ) -> Result<(), StoreError> {
        delivery::advance_fanout(self, lease, after).await
    }

    async fn publish_content(
        &self,
        lease: &LeasedWork,
        revision: ContentRevision,
    ) -> Result<(), StoreError> {
        content::publish_content(self, lease, revision).await
    }

    async fn cleanup_content(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        _requested_keep: uuid::Uuid,
    ) -> Result<(), StoreError> {
        content::cleanup_content(self, lease, record, _requested_keep).await
    }

    async fn record_refresh_failure(
        &self,
        lease: &LeasedWork,
        record: SourceRecordId,
        diagnostic: &str,
    ) -> Result<(), StoreError> {
        content::record_refresh_failure(self, lease, record, diagnostic).await
    }

    async fn evaluate_article_rules(
        &self,
        lease: &LeasedWork,
        workspace: WorkspaceId,
        article: ArticleId,
        evaluations: &[PendingRuleEvaluation],
    ) -> Result<(), StoreError> {
        rules::evaluate_article_rules(self, lease, workspace, article, evaluations).await
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
        rules::apply_rule_batch(
            self,
            lease,
            workspace_id,
            rule_id,
            version,
            after_id,
            through_id,
            limit,
        )
        .await
    }
}

/// Scheduler durations and concurrency must be positive before opening a pool.
fn validate_scheduler_limits(
    poll: chrono::Duration,
    concurrency: usize,
    retry: chrono::Duration,
) -> Result<(), StoreError> {
    if poll <= chrono::Duration::zero() || concurrency == 0 || retry <= chrono::Duration::zero() {
        return Err(StoreError::Unavailable(
            "scheduler limits must be positive".into(),
        ));
    }
    Ok(())
}
