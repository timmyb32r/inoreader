//! Explicit, resumable enrichment of retained pages. No network calls and no
//! article/state/feed-date changes. Each manifest update compares the complete
//! observed document, so a concurrent refresh is never overwritten.
use reader_core::resolve_publication;
use reader_ingest::{ContentManifestPointer, SourceRecord};
use serde::Serialize;
use sqlx::PgPool;
use std::{collections::BTreeMap, num::NonZeroU32};

#[derive(Default, Serialize)]
pub struct PublicationAudit {
    pub records: u64,
    pub feed_dates: u64,
    pub page_dates: u64,
    pub undated: u64,
    pub conflicting: u64,
    pub unavailable_content: u64,
    pub undecodable_content: u64,
    pub concurrent_refreshes: u64,
    pub updated: u64,
}

pub async fn backfill_publication_dates(
    pool: &PgPool,
    batch: NonZeroU32,
    max_input_bytes: usize,
    apply: bool,
) -> Result<BTreeMap<String, PublicationAudit>, sqlx::Error> {
    let mut report = BTreeMap::<String, PublicationAudit>::new();
    let mut after = String::new();
    loop {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id,document FROM source_records WHERE id>$1 ORDER BY id LIMIT $2",
        )
        .bind(&after)
        .bind(i64::from(batch.get()))
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            break;
        }
        for (id, document) in rows {
            let record: SourceRecord = serde_json::from_str(&document).map_err(protocol)?;
            let stats = report
                .entry(record.source_id().as_uuid().to_string())
                .or_default();
            stats.records += 1;
            if record.published_at().is_some() {
                stats.feed_dates += 1;
                after = id;
                continue;
            }
            let snapshot: Option<RawRow> = sqlx::query_as("SELECT m.document,c.ordinals,c.chunks FROM content_manifests m LEFT JOIN LATERAL (SELECT array_agg(ordinal ORDER BY ordinal) ordinals,array_agg(bytes ORDER BY ordinal) chunks FROM staged_content_chunks WHERE record_id=m.id AND refresh_id=m.document::jsonb->>'refresh_id' AND representation='raw') c ON true WHERE m.id=$1")
                .bind(&id).fetch_optional(pool).await?;
            let Some((manifest, ordinals, chunks)) = snapshot else {
                stats.unavailable_content += 1;
                stats.undated += 1;
                after = id;
                continue;
            };
            let pointer: ContentManifestPointer =
                serde_json::from_str(&manifest).map_err(protocol)?;
            let bytes = raw_bytes(&id, &pointer, ordinals, chunks, max_input_bytes)?;
            let source = match reader_ingest::decode_html(&bytes, None) {
                Ok(source) => source,
                Err(_) => {
                    stats.undecodable_content += 1;
                    stats.undated += 1;
                    after = id;
                    continue;
                }
            };
            let publication = reader_ingest::extract_publication(&source, &pointer.final_url);
            if resolve_publication(&publication).is_some() {
                stats.page_dates += 1;
            } else if publication.iter().any(|v| v.date().is_some()) {
                stats.conflicting += 1;
            } else {
                stats.undated += 1;
            }
            if apply && pointer.publication.as_ref() != Some(&publication) {
                let mut value: serde_json::Value =
                    serde_json::from_str(&manifest).map_err(protocol)?;
                value["publication"] = serde_json::to_value(&publication).map_err(protocol)?;
                let changed = sqlx::query(
                    "UPDATE content_manifests SET document=$3 WHERE id=$1 AND document=$2",
                )
                .bind(&id)
                .bind(&manifest)
                .bind(serde_json::to_string(&value).map_err(protocol)?)
                .execute(pool)
                .await?
                .rows_affected();
                if changed == 1 {
                    stats.updated += 1;
                } else {
                    stats.concurrent_refreshes += 1;
                }
            }
            after = id;
        }
    }
    Ok(report)
}

type RawRow = (String, Option<Vec<i64>>, Option<Vec<Vec<u8>>>);
fn raw_bytes(
    id: &str,
    pointer: &ContentManifestPointer,
    ordinals: Option<Vec<i64>>,
    chunks: Option<Vec<Vec<u8>>>,
    max_bytes: usize,
) -> Result<Vec<u8>, sqlx::Error> {
    let ordinals = ordinals.unwrap_or_default();
    let chunks = chunks.unwrap_or_default();
    if pointer.record_id.as_uuid().to_string() != id
        || chunks.len() != pointer.raw_chunks as usize
        || ordinals.len() != chunks.len()
    {
        return Err(protocol(format!(
            "invalid raw content manifest for record {id}"
        )));
    }
    let mut result = Vec::new();
    for (index, (ordinal, bytes)) in ordinals.into_iter().zip(chunks).enumerate() {
        if usize::try_from(ordinal).ok() != Some(index) {
            return Err(protocol(format!(
                "noncontiguous raw chunks for record {id}"
            )));
        }
        if result
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > max_bytes)
        {
            return Err(protocol(format!(
                "raw content exceeds configured max_input_bytes for record {id}"
            )));
        }
        result.extend(bytes);
    }
    Ok(result)
}
fn protocol(error: impl std::fmt::Display) -> sqlx::Error {
    sqlx::Error::Protocol(error.to_string())
}
