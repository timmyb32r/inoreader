use reader_ingest::{ContentManifestPointer, ContentRevision};
use sqlx::{Postgres, Transaction};

/// Lossless source storage is untouched. This derived projection indexes visible
/// HTML text (the same extraction used by article AI), never scripts or markup.
async fn plain(bytes: Vec<u8>) -> Result<String, sqlx::Error> {
    tokio::task::spawn_blocking(move || {
        let html = String::from_utf8(bytes)
            .map_err(|_| sqlx::Error::Protocol("search projection: invalid source UTF-8".into()))?;
        Ok(reader_ai::article_plain_text(&html))
    })
    .await
    .map_err(|_| sqlx::Error::Protocol("search projection worker failed".into()))?
}
async fn save(
    tx: &mut Transaction<'_, Postgres>,
    record: &str,
    text: String,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO search_content(record_id,text) VALUES($1,$2) ON CONFLICT(record_id) DO UPDATE SET text=EXCLUDED.text").bind(record).bind(text).execute(&mut **tx).await?;
    Ok(())
}
pub(crate) async fn publish(
    tx: &mut Transaction<'_, Postgres>,
    revision: &ContentRevision,
) -> Result<(), sqlx::Error> {
    let mut bytes = Vec::new();
    for (i, c) in revision.safe_html_chunks.iter().enumerate() {
        if c.ordinal as usize != i {
            return Err(sqlx::Error::Protocol(
                "search projection: noncontiguous content".into(),
            ));
        }
        bytes.extend_from_slice(&c.bytes);
    }
    save(
        tx,
        &revision.record_id.as_uuid().to_string(),
        plain(bytes).await?,
    )
    .await
}
pub(crate) async fn backfill(
    tx: &mut Transaction<'_, Postgres>,
    batch: std::num::NonZeroU32,
) -> Result<(), sqlx::Error> {
    let mut after = String::new();
    loop {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id,document FROM content_manifests WHERE id>$1 ORDER BY id LIMIT $2",
        )
        .bind(&after)
        .bind(i64::from(batch.get()))
        .fetch_all(&mut **tx)
        .await?;
        if rows.is_empty() {
            break;
        }
        for (id, document) in rows {
            let manifest: ContentManifestPointer =
                serde_json::from_str(&document).map_err(|_| {
                    sqlx::Error::Protocol("invalid content manifest for search projection".into())
                })?;
            let chunks:Vec<(i64,Vec<u8>)>=sqlx::query_as("SELECT ordinal,bytes FROM staged_content_chunks WHERE record_id=$1 AND refresh_id=$2 AND representation='safe' ORDER BY ordinal").bind(&id).bind(manifest.refresh_id.to_string()).fetch_all(&mut **tx).await?;
            if manifest.record_id.as_uuid().to_string() != id
                || chunks.len() != manifest.safe_html_chunks as usize
            {
                return Err(sqlx::Error::Protocol(
                    "search projection: missing content chunks".into(),
                ));
            }
            let mut bytes = Vec::new();
            for (i, (ordinal, chunk)) in chunks.into_iter().enumerate() {
                if usize::try_from(ordinal).ok() != Some(i) {
                    return Err(sqlx::Error::Protocol(
                        "search projection: invalid chunk ordinal".into(),
                    ));
                }
                bytes.extend(chunk);
            }
            save(tx, &id, plain(bytes).await?).await?;
            after = id;
        }
    }
    Ok(())
}
