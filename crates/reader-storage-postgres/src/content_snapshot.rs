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
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT m.id,m.document,
          (SELECT json_agg(json_build_array(c.ordinal,c.bytes) ORDER BY c.ordinal)::text
           FROM staged_content_chunks c WHERE c.record_id=m.id
             AND c.refresh_id=m.document::jsonb->>'refresh_id' AND c.representation='safe')
         FROM library_origins o JOIN subscriptions s ON s.id=o.subscription_id
         JOIN content_manifests m ON m.id=o.source_record_id
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
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT m.id,m.document,
          (SELECT json_agg(json_build_array(c.ordinal,c.bytes) ORDER BY c.ordinal)::text
           FROM staged_content_chunks c WHERE c.record_id=m.id
             AND c.refresh_id=m.document::jsonb->>'refresh_id' AND c.representation='safe')
         FROM unnest($1::text[]) WITH ORDINALITY AS input(id,position)
         JOIN content_manifests m ON m.id=input.id ORDER BY input.position",
    )
    .bind(records)
    .fetch_all(connection)
    .await?;
    rows.into_iter().map(decode).collect()
}

fn decode(
    (record, manifest, chunks): (String, String, Option<String>),
) -> Result<ContentSnapshot, sqlx::Error> {
    let invalid = || {
        sqlx::Error::Protocol(format!(
            "invalid published content generation for record {record}"
        ))
    };
    let pointer: ContentManifestPointer = serde_json::from_str(&manifest).map_err(|_| invalid())?;
    let chunks: Vec<(u32, String)> =
        serde_json::from_str(chunks.as_deref().unwrap_or("[]")).map_err(|_| invalid())?;
    if pointer.record_id.as_uuid().to_string() != record
        || chunks.len() != pointer.safe_html_chunks as usize
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    for (expected, (ordinal, chunk)) in chunks.into_iter().enumerate() {
        if ordinal as usize != expected {
            return Err(invalid());
        }
        bytes.extend(serde_json::from_str::<Vec<u8>>(&chunk).map_err(|_| invalid())?);
    }
    let html = String::from_utf8(bytes).map_err(|_| invalid())?;
    Ok(ContentSnapshot {
        record,
        pointer,
        html,
    })
}
