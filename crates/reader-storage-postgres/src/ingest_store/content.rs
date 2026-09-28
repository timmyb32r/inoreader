//! PostgreSQL content operations. Transaction boundaries stay local and explicit.
use super::*;

pub(super) async fn publish_content(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    revision: ContentRevision,
) -> Result<(), StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
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
        // PostgreSQL's protocol allows at most 65535 bind parameters. This
        // bounds one statement, never the accepted content or chunk count.
        for batch in chunks.chunks(65535 / 5) {
            let mut query = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                    "INSERT INTO staged_content_chunks (record_id,refresh_id,representation,ordinal,bytes) "
                );
            query.push_values(batch, |mut row, chunk| {
                row.push_bind(&record)
                    .push_bind(revision.refresh_id.to_string())
                    .push_bind(representation)
                    .push_bind(i64::from(chunk.ordinal))
                    .push_bind(&chunk.bytes);
            });
            query.push(" ON CONFLICT (record_id,refresh_id,representation,ordinal) DO UPDATE SET bytes=EXCLUDED.bytes");
            query.build().execute(&mut *tx).await.map_err(storage)?;
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
    crate::search::projection::publish(&mut tx, &revision)
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

pub(super) async fn cleanup_content(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    record: SourceRecordId,
    _requested_keep: uuid::Uuid,
) -> Result<(), StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
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

pub(super) async fn record_refresh_failure(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    record: SourceRecordId,
    diagnostic: &str,
) -> Result<(), StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
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
