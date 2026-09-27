use reader_ingest::ContentManifestPointer;
use sqlx::PgPool;
use uuid::Uuid;

/// A complete published content generation read in one PostgreSQL MVCC snapshot.
/// Manifest, chunk count, contiguous ordinals and UTF-8 must agree before exposure.
/// Source bytes are retained exactly; callers own any explicit rendering transform.
pub(crate) struct ContentSnapshot {
    pub record: String,
    pub pointer: ContentManifestPointer,
    pub html: String,
}

pub(crate) async fn read(
    pool: &PgPool,
    workspace: Uuid,
    article: Uuid,
) -> Result<Option<ContentSnapshot>, sqlx::Error> {
    let row: Option<SnapshotRow> = sqlx::query_as(
        "SELECT m.id,m.document,c.ordinals,c.chunks
         FROM library_origins o JOIN subscriptions s ON s.id=o.subscription_id
         JOIN content_manifests m ON m.id=o.source_record_id
         LEFT JOIN LATERAL (SELECT array_agg(ordinal ORDER BY ordinal) AS ordinals, array_agg(bytes ORDER BY ordinal) AS chunks FROM staged_content_chunks WHERE record_id=m.id AND refresh_id=m.document::jsonb->>'refresh_id' AND representation='safe') c ON true
         WHERE o.workspace_id=$1 AND o.article_id=$2 AND s.document::jsonb->>'workspace_id'=$1
         ORDER BY (m.document::jsonb->>'fetched_at')::timestamptz DESC,m.id DESC LIMIT 1",
    )
    .bind(workspace.to_string())
    .bind(article.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(decode).transpose()
}

/// Rule evaluation consumes all article origins in their authored order. One
/// statement pins every generation; a transaction alone at READ COMMITTED does not.
pub(crate) async fn read_origins(
    connection: &mut sqlx::PgConnection,
    records: &[String],
) -> Result<Vec<ContentSnapshot>, sqlx::Error> {
    let rows: Vec<SnapshotRow> = sqlx::query_as(
        "SELECT m.id,m.document,c.ordinals,c.chunks
         FROM unnest($1::text[]) WITH ORDINALITY AS input(id,position)
         JOIN content_manifests m ON m.id=input.id
         LEFT JOIN LATERAL (SELECT array_agg(ordinal ORDER BY ordinal) AS ordinals, array_agg(bytes ORDER BY ordinal) AS chunks FROM staged_content_chunks WHERE record_id=m.id AND refresh_id=m.document::jsonb->>'refresh_id' AND representation='safe') c ON true ORDER BY input.position",
    )
    .bind(records)
    .fetch_all(connection)
    .await?;
    rows.into_iter().map(decode).collect()
}

type SnapshotRow = (String, String, Option<Vec<i64>>, Option<Vec<Vec<u8>>>);

fn decode(
    (record, manifest, ordinals, chunks): SnapshotRow,
) -> Result<ContentSnapshot, sqlx::Error> {
    let invalid = || {
        sqlx::Error::Protocol(format!(
            "invalid published content generation for record {record}"
        ))
    };
    let pointer: ContentManifestPointer = serde_json::from_str(&manifest).map_err(|_| invalid())?;
    let chunks = chunks.unwrap_or_default();
    let ordinals = ordinals.unwrap_or_default();
    if pointer.record_id.as_uuid().to_string() != record
        || chunks.len() != pointer.safe_html_chunks as usize
        || ordinals.len() != chunks.len()
    {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(chunks.iter().map(Vec::len).sum());
    for (expected, (ordinal, chunk)) in ordinals.into_iter().zip(chunks).enumerate() {
        if usize::try_from(ordinal).ok() != Some(expected) {
            return Err(invalid());
        }
        bytes.extend(chunk);
    }
    let html = String::from_utf8(bytes).map_err(|_| invalid())?;
    Ok(ContentSnapshot {
        record,
        pointer,
        html,
    })
}
