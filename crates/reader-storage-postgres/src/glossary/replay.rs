use super::*;

impl PostgresGlossaryStore {
    /// Rebuild only the current retained observation of each post. Older events
    /// are never replayed over a newer edit. Ambiguous duplicate payloads remain
    /// quarantined; other failed raw events are queued for the current parser.
    pub async fn reindex(
        &self,
        owner: Uuid,
        workspace: Uuid,
        batch_size: u16,
    ) -> Result<usize, GlossaryError> {
        if batch_size == 0 {
            return Err(GlossaryError::Configuration);
        }
        owned(&self.pool, owner, workspace).await?;
        let mut cursor = 0_i64;
        let mut count = 0;
        loop {
            let mut tx = self.pool.begin().await.map_err(storage)?;
            let binding: Option<Option<String>> = sqlx::query_scalar(
                "SELECT binding FROM glossary_channels WHERE owner=$1 AND workspace=$2 FOR UPDATE",
            )
            .bind(owner)
            .bind(workspace)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
            let binding: Option<ChannelBinding> = binding
                .ok_or(GlossaryError::NotFound)?
                .map(|v| decode(&v))
                .transpose()?;
            let rows=sqlx::query("SELECT message_id,source_document,receipt,kind FROM glossary_posts WHERE owner=$1 AND workspace=$2 AND message_id>$3 ORDER BY message_id LIMIT $4 FOR UPDATE")
                .bind(owner).bind(workspace).bind(cursor).bind(i64::from(batch_size)).fetch_all(&mut *tx).await.map_err(storage)?;
            if rows.is_empty() {
                sqlx::query("UPDATE glossary_events SET error=NULL WHERE owner=$1 AND workspace=$2 AND NOT applied AND NOT conflict")
                    .bind(owner).bind(workspace).execute(&mut *tx).await.map_err(storage)?;
                tx.commit().await.map_err(storage)?;
                return Ok(count);
            }
            for row in rows {
                let raw: String = row.try_get("source_document").map_err(storage)?;
                let kind: String = row.try_get("kind").map_err(storage)?;
                let post = if kind == "bot" {
                    ObservedPost::telegram(
                        &decode::<serde_json::Value>(&raw)?,
                        binding
                            .as_ref()
                            .ok_or(GlossaryError::Identity)?
                            .channel_id(),
                    )?
                } else {
                    ObservedPost::archive(raw)?
                };
                let id: i64 = row.try_get("message_id").map_err(storage)?;
                if post.id() != id {
                    return Err(GlossaryError::Identity);
                }
                projection::apply_post(
                    &mut tx,
                    owner,
                    workspace,
                    &post,
                    row.try_get("receipt").map_err(storage)?,
                    &kind,
                )
                .await?;
                cursor = id;
                count += 1;
            }
            tx.commit().await.map_err(storage)?;
        }
    }
}
