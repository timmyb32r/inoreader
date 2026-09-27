use sqlx::{Postgres, Transaction};

/// Offline conversion under the enclosing ACCESS EXCLUSIVE lock. Keyset batches
/// retain the old column until every byte array has been checked and written.
/// The transaction rolls back all progress on any malformed row or write error.
pub(super) async fn content_bytes(
    tx: &mut Transaction<'_, Postgres>,
    batch: std::num::NonZeroU32,
) -> Result<(), sqlx::Error> {
    sqlx::query("ALTER TABLE staged_content_chunks ADD COLUMN binary_bytes BYTEA")
        .execute(&mut **tx)
        .await?;
    let mut after: Option<(String, String, String, i64)> = None;
    loop {
        let rows: Vec<(String,String,String,i64,String)> = sqlx::query_as(
            "SELECT record_id,refresh_id,representation,ordinal,bytes FROM staged_content_chunks WHERE $1::text IS NULL OR (record_id,refresh_id,representation,ordinal)>($1,$2,$3,$4) ORDER BY record_id,refresh_id,representation,ordinal LIMIT $5")
            .bind(after.as_ref().map(|x| &x.0)).bind(after.as_ref().map(|x| &x.1)).bind(after.as_ref().map(|x| &x.2)).bind(after.as_ref().map(|x| x.3))
            .bind(i64::from(batch.get())).fetch_all(&mut **tx).await?;
        if rows.is_empty() {
            break;
        }
        let mut converted = Vec::with_capacity(rows.len());
        for (record, refresh, representation, ordinal, raw) in rows {
            let bytes: Vec<u8> = serde_json::from_str(&raw).map_err(|_| sqlx::Error::Protocol(format!("invalid byte array for content record {record}, refresh {refresh}, representation {representation}, ordinal {ordinal}; upgrade rolled back")))?;
            converted.push((record, refresh, representation, ordinal, bytes));
        }
        // Five parameters per row; this is a wire-protocol capacity, not a
        // payload rejection or implicit operational limit.
        for part in converted.chunks(65535 / 5) {
            let mut query = sqlx::QueryBuilder::<Postgres>::new(
                "UPDATE staged_content_chunks c SET binary_bytes=v.bytes FROM (",
            );
            query.push_values(
                part,
                |mut row, (record, refresh, representation, ordinal, bytes)| {
                    row.push_bind(record)
                        .push_bind(refresh)
                        .push_bind(representation)
                        .push_bind(*ordinal)
                        .push_bind(bytes);
                },
            );
            query.push(") AS v(record_id,refresh_id,representation,ordinal,bytes) WHERE (c.record_id,c.refresh_id,c.representation,c.ordinal)=(v.record_id,v.refresh_id,v.representation,v.ordinal)");
            let changed = query.build().execute(&mut **tx).await?.rows_affected();
            if changed != part.len() as u64 {
                return Err(sqlx::Error::Protocol(
                    "content upgrade row count differs; transaction rolled back".into(),
                ));
            }
        }
        after = converted
            .last()
            .map(|x| (x.0.clone(), x.1.clone(), x.2.clone(), x.3));
    }
    sqlx::raw_sql("ALTER TABLE staged_content_chunks ALTER COLUMN binary_bytes SET NOT NULL; ALTER TABLE staged_content_chunks DROP COLUMN bytes; ALTER TABLE staged_content_chunks RENAME COLUMN binary_bytes TO bytes; CLUSTER staged_content_chunks USING staged_content_chunks_pkey;")
        .execute(&mut **tx).await?;
    Ok(())
}
