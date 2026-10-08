//! Initial AI admission is frozen after source collection AND owned delivery.
//! Public discovery metadata contains no owner data. Subscription selection is
//! private and is checked again at scheduler and budget-reservation boundaries.
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use reader_ingest::{PollCommit, SourceRecord};
use sqlx::{Postgres, Transaction};

pub(crate) async fn lock_source(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("ai-discovery/{source}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Calendar ordering projection only: zoned dates use UTC, unzoned dates retain
/// their declared clock, day-only dates sort at their day's start. Exact source
/// dates are never rewritten. Nanosecond precision is retained by the SQL key.
fn publication_clock(record: &SourceRecord) -> Option<String> {
    let value = record.published_at()?.as_str();
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        Some(
            value
                .with_timezone(&Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        )
    } else if let Ok(value) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f") {
        Some(value.format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string())
    } else {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .map(|day| format!("{day}T00:00:00.000000000Z"))
    }
}

pub(crate) async fn discover(
    tx: &mut Transaction<'_, Postgres>,
    record: &SourceRecord,
    ordinal: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO ai_record_discovery(record,source,publication_order,source_order) VALUES($1,$2,reader_arrival_order($3),$4) ON CONFLICT(record) DO NOTHING")
        .bind(record.id().as_uuid().to_string()).bind(record.source_id().as_uuid().to_string())
        .bind(publication_clock(record)).bind(ordinal).execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn begin_poll(
    tx: &mut Transaction<'_, Postgres>,
    commit: &PollCommit,
) -> Result<(), sqlx::Error> {
    let source = commit.source_id.as_uuid().to_string();
    if commit
        .records
        .len()
        .checked_add(commit.remainder.len())
        .and_then(|length| i64::try_from(length).ok())
        .is_none()
    {
        return Err(sqlx::Error::Protocol(
            "source order exceeds storage range".into(),
        ));
    }
    lock_source(tx, &source).await?;
    // Register the entire response before fan-out, including subsequent batches.
    // PostgreSQL's parameter capacity bounds statements, never accepted records.
    let records: Vec<_> = commit
        .records
        .iter()
        .chain(&commit.remainder)
        .enumerate()
        .collect();
    for batch in records.chunks(65535 / 4) {
        let mut query = sqlx::QueryBuilder::<Postgres>::new("INSERT INTO ai_record_discovery(record,source,publication_order,source_order) SELECT d.record,d.source,reader_arrival_order(d.clock),d.ordinal FROM (");
        query.push_values(batch, |mut row, (ordinal, polled)| {
            row.push_bind(polled.record.id().as_uuid().to_string())
                .push_bind(&source)
                .push_bind(publication_clock(&polled.record))
                .push_bind(*ordinal as i64);
        });
        query.push(") AS d(record,source,clock,ordinal) ON CONFLICT(record) DO NOTHING");
        query.build().execute(&mut **tx).await?;
    }
    let record_ids: Vec<_> = records
        .iter()
        .map(|(_, polled)| polled.record.id().as_uuid().to_string())
        .collect();
    sqlx::query("INSERT INTO subscription_ai_initial_records(subscription,source,record) SELECT b.subscription,b.source,d.record FROM subscription_ai_bootstrap b JOIN ai_record_discovery d ON d.source=b.source WHERE b.source=$1 AND b.cutoff IS NULL AND d.record=ANY($2) ON CONFLICT DO NOTHING")
        .bind(&source).bind(record_ids).execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn finish_poll(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    incomplete: bool,
) -> Result<(), sqlx::Error> {
    if incomplete {
        return Ok(());
    }
    let subscriptions: Vec<String> = sqlx::query_scalar("UPDATE subscription_ai_bootstrap SET cutoff=COALESCE((SELECT max(sequence) FROM ai_record_discovery WHERE source=$1),0) WHERE source=$1 AND cutoff IS NULL RETURNING subscription")
        .bind(source).fetch_all(&mut **tx).await?;
    for sub in subscriptions {
        sqlx::query("SELECT reader_ai_finalize_subscription($1,$2)")
            .bind(sub)
            .bind(source)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub(crate) async fn create_subscription(
    tx: &mut Transaction<'_, Postgres>,
    subscription: &str,
    source: &str,
) -> Result<(), sqlx::Error> {
    lock_source(tx, source).await?;
    sqlx::query("INSERT INTO subscription_ai_bootstrap(subscription,owner,workspace,source,initial_articles) SELECT s.id,(w.document::jsonb->>'owner')::uuid,w.id,$2,p.initial_articles FROM subscriptions s JOIN workspaces w ON w.id=s.document::jsonb->>'workspace_id' CROSS JOIN ai_bootstrap_policy p WHERE s.id=$1 ON CONFLICT(subscription,source) DO NOTHING")
        .bind(subscription).bind(source).execute(&mut **tx).await?;
    // A subscription added while a source's initial response is draining must
    // include its remaining records even though they aren't in the identity cache.
    let pending: Vec<String> =
        sqlx::query_scalar("SELECT document FROM poll_backlog WHERE source_id=$1 ORDER BY ordinal")
            .bind(source)
            .fetch_all(&mut **tx)
            .await?;
    for document in pending {
        let record: SourceRecord = serde_json::from_str(&document)
            .map_err(|_| sqlx::Error::Protocol("invalid retained source record".into()))?;
        sqlx::query("INSERT INTO subscription_ai_initial_records(subscription,source,record) VALUES($1,$3,$2) ON CONFLICT DO NOTHING")
            .bind(subscription).bind(record.id().as_uuid().to_string()).bind(source).execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/ai_bootstrap.rs"]
mod tests;

/// Offline, batched projection initialization. Source documents, paid jobs and
/// user state are untouched. Existing pending source batches remain initial.
pub(crate) async fn install(
    tx: &mut Transaction<'_, Postgres>,
    batch: std::num::NonZeroU32,
) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(include_str!("schema/ai_bootstrap.sql"))
        .execute(&mut **tx)
        .await?;
    let mut after = String::new();
    loop {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id,document FROM source_records WHERE id>$1 ORDER BY id LIMIT $2",
        )
        .bind(&after)
        .bind(i64::from(batch.get()))
        .fetch_all(&mut **tx)
        .await?;
        if rows.is_empty() {
            break;
        }
        for (id, document) in rows {
            let record: SourceRecord = serde_json::from_str(&document).map_err(|_| {
                sqlx::Error::Protocol(format!(
                    "invalid source record {id}; bootstrap upgrade rolled back"
                ))
            })?;
            discover(tx, &record, 0).await?;
            after = id;
        }
    }
    let mut pending_after: Option<(String, i64)> = None;
    loop {
        let pending: Vec<(String, i64, String)> = sqlx::query_as("SELECT source_id,ordinal,document FROM poll_backlog WHERE $1::text IS NULL OR (source_id,ordinal)>($1,$2) ORDER BY source_id,ordinal LIMIT $3")
            .bind(pending_after.as_ref().map(|value| &value.0))
            .bind(pending_after.as_ref().map(|value| value.1))
            .bind(i64::from(batch.get())).fetch_all(&mut **tx).await?;
        if pending.is_empty() {
            break;
        }
        for (source, ordinal, document) in pending {
            let record: SourceRecord = serde_json::from_str(&document).map_err(|_| {
                sqlx::Error::Protocol(
                    "invalid pending source record; bootstrap upgrade rolled back".into(),
                )
            })?;
            discover(tx, &record, ordinal).await?;
            pending_after = Some((source, ordinal));
        }
    }
    sqlx::query("INSERT INTO subscription_ai_bootstrap(subscription,owner,workspace,source,initial_articles,cutoff) SELECT s.id,(w.document::jsonb->>'owner')::uuid,w.id,l.source_id,p.initial_articles,CASE WHEN h.document IS NULL OR h.document::jsonb->>'incomplete'='true' THEN NULL ELSE COALESCE((SELECT max(sequence) FROM ai_record_discovery WHERE source=l.source_id),0) END FROM subscriptions s JOIN subscription_sources l ON l.subscription_id=s.id JOIN workspaces w ON w.id=s.document::jsonb->>'workspace_id' CROSS JOIN ai_bootstrap_policy p LEFT JOIN source_health h ON h.source_id=l.source_id")
        .execute(&mut **tx).await?;
    sqlx::query("INSERT INTO subscription_ai_initial_records(subscription,source,record) SELECT DISTINCT b.subscription,b.source,d.record FROM subscription_ai_bootstrap b JOIN library_origins o ON o.subscription_id=b.subscription AND o.workspace_id=b.workspace JOIN ai_record_discovery d ON d.record=o.source_record_id AND d.source=b.source")
        .execute(&mut **tx).await?;
    let subscriptions: Vec<(String, String)> = sqlx::query_as(
        "SELECT subscription,source FROM subscription_ai_bootstrap ORDER BY subscription,source",
    )
    .fetch_all(&mut **tx)
    .await?;
    for (subscription, source) in subscriptions {
        sqlx::query("SELECT reader_ai_finalize_subscription($1,$2)")
            .bind(subscription)
            .bind(source)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
