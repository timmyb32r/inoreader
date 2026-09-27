use super::*;

impl PostgresGlossaryStore {
    pub(super) async fn lease_poll(
        &self,
        seconds: u64,
    ) -> Result<Option<PollClaim>, GlossaryError> {
        let seconds = i64::try_from(seconds).map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query("SELECT c.* FROM glossary_channels c JOIN workspaces w ON w.id=c.workspace::text AND w.document::jsonb->>'owner'=c.owner::text WHERE encrypted_token IS NOT NULL AND next_poll<=now() AND (poll_until IS NULL OR poll_until<=now()) ORDER BY next_poll FOR UPDATE OF c SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let owner = row.try_get("owner").map_err(storage)?;
        let workspace = row.try_get("workspace").map_err(storage)?;
        let lease = Uuid::new_v4();
        sqlx::query("UPDATE glossary_channels SET poll_lease=$3,poll_until=now()+$4::bigint*interval '1 second',next_history=CASE WHEN last_poll<now()-interval '24 hours' THEN now() ELSE next_history END,history_error=CASE WHEN last_poll<now()-interval '24 hours' THEN 'Bot delivery gap exceeds Telegram retention; public reconciliation required' ELSE history_error END,coverage_note=CASE WHEN last_poll<now()-interval '24 hours' THEN 'Непроверенный интервал после ' || last_poll::text || '; публичная история не доказывает полноту' ELSE coverage_note END WHERE owner=$1 AND workspace=$2")
            .bind(owner).bind(workspace).bind(lease).bind(seconds).execute(&mut *tx).await.map_err(storage)?;
        let claim = PollClaim {
            owner,
            workspace,
            lease,
            binding: decode(row.try_get::<&str, _>("binding").map_err(storage)?)?,
            encrypted_token: row.try_get("encrypted_token").map_err(storage)?,
            offset: row.try_get("poll_cursor").map_err(storage)?,
        };
        tx.commit().await.map_err(storage)?;
        Ok(Some(claim))
    }
    pub(super) async fn persist_batch(
        &self,
        claim: &PollClaim,
        batch: &UpdateBatch,
    ) -> Result<(), GlossaryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let valid = sqlx::query("SELECT 1 FROM glossary_channels WHERE owner=$1 AND workspace=$2 AND poll_lease=$3 AND poll_until>now() FOR UPDATE")
            .bind(claim.owner).bind(claim.workspace).bind(claim.lease).fetch_optional(&mut *tx).await.map_err(storage)?;
        if valid.is_none() {
            return Err(GlossaryError::Conflict);
        }
        if !batch.updates().is_empty() {
            let raw_id = receipt(
                &mut tx,
                claim.owner,
                claim.workspace,
                "bot_batch",
                &batch
                    .next_offset()
                    .ok_or(GlossaryError::Protocol)?
                    .to_string(),
                batch.raw(),
            )
            .await?;
            for event in batch.updates() {
                let id = event["update_id"].as_i64().ok_or(GlossaryError::Protocol)?;
                let document = encode(event)?;
                let previous: Option<String> = sqlx::query_scalar("SELECT document FROM glossary_events WHERE owner=$1 AND workspace=$2 AND update_id=$3")
                    .bind(claim.owner).bind(claim.workspace).bind(id).fetch_optional(&mut *tx).await.map_err(storage)?;
                if let Some(previous) = previous {
                    if previous != document {
                        sqlx::query("UPDATE glossary_events SET conflict=true,error='Conflicting payload for update ID; both raw responses retained' WHERE owner=$1 AND workspace=$2 AND update_id=$3")
                            .bind(claim.owner).bind(claim.workspace).bind(id).execute(&mut *tx).await.map_err(storage)?;
                    }
                } else {
                    sqlx::query("INSERT INTO glossary_events(owner,workspace,update_id,receipt,document) VALUES($1,$2,$3,$4,$5)")
                        .bind(claim.owner).bind(claim.workspace).bind(id).bind(raw_id).bind(document).execute(&mut *tx).await.map_err(storage)?;
                }
            }
            sqlx::query(
                "UPDATE glossary_channels SET poll_cursor=$3 WHERE owner=$1 AND workspace=$2",
            )
            .bind(claim.owner)
            .bind(claim.workspace)
            .bind(batch.next_offset())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
    pub(super) async fn release_poll(
        &self,
        claim: &PollClaim,
        error: Option<&str>,
        retry: u64,
    ) -> Result<(), GlossaryError> {
        let retry = i64::try_from(retry).map_err(storage)?;
        let changed = sqlx::query("UPDATE glossary_channels SET poll_lease=NULL,poll_until=NULL,poll_error=$4,next_poll=now()+$5::bigint*interval '1 second',last_poll=CASE WHEN $4::text IS NULL THEN now() ELSE last_poll END WHERE owner=$1 AND workspace=$2 AND poll_lease=$3 AND poll_until>now()")
            .bind(claim.owner).bind(claim.workspace).bind(claim.lease).bind(error).bind(retry).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(GlossaryError::Conflict);
        }
        Ok(())
    }
    pub(super) async fn lease_history(
        &self,
        seconds: u64,
    ) -> Result<Option<HistoryClaim>, GlossaryError> {
        let seconds = i64::try_from(seconds).map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query("SELECT c.owner,c.workspace,c.history_before FROM glossary_channels c JOIN workspaces w ON w.id=c.workspace::text AND w.document::jsonb->>'owner'=c.owner::text WHERE import_complete AND next_history<=now() AND (history_until IS NULL OR history_until<=now()) ORDER BY next_history FOR UPDATE OF c SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let claim = HistoryClaim {
            owner: row.try_get("owner").map_err(storage)?,
            workspace: row.try_get("workspace").map_err(storage)?,
            before: row.try_get("history_before").map_err(storage)?,
            lease: Uuid::new_v4(),
        };
        sqlx::query("UPDATE glossary_channels SET history_lease=$3,history_until=now()+$4::bigint*interval '1 second' WHERE owner=$1 AND workspace=$2")
            .bind(claim.owner).bind(claim.workspace).bind(claim.lease).bind(seconds).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(Some(claim))
    }
    pub(super) async fn persist_page(
        &self,
        claim: &HistoryClaim,
        page: &PublicPage,
        delay: u64,
    ) -> Result<(), GlossaryError> {
        if page
            .before
            .is_some_and(|next| claim.before.is_some_and(|before| next >= before))
        {
            return Err(GlossaryError::Conflict);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let valid = sqlx::query("SELECT 1 FROM glossary_channels WHERE owner=$1 AND workspace=$2 AND history_lease=$3 AND history_until>now() FOR UPDATE")
            .bind(claim.owner).bind(claim.workspace).bind(claim.lease).fetch_optional(&mut *tx).await.map_err(storage)?;
        if valid.is_none() {
            return Err(GlossaryError::Conflict);
        }
        let raw_id = receipt(
            &mut tx,
            claim.owner,
            claim.workspace,
            "public_page",
            &claim
                .before
                .map(|n| n.to_string())
                .unwrap_or_else(|| "latest".into()),
            &page.raw,
        )
        .await?;
        for post in &page.posts {
            projection::apply_post(
                &mut tx,
                claim.owner,
                claim.workspace,
                post,
                raw_id,
                "public",
            )
            .await?;
        }
        sqlx::query("UPDATE glossary_channels SET history_before=$4,history_lease=NULL,history_until=NULL,next_history=now()+$5::bigint*interval '1 second',history_error=NULL,last_history=CASE WHEN $4::bigint IS NULL THEN now() ELSE last_history END WHERE owner=$1 AND workspace=$2 AND history_lease=$3")
            .bind(claim.owner).bind(claim.workspace).bind(claim.lease).bind(page.before).bind(i64::try_from(delay).map_err(storage)?).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    pub(super) async fn release_history(
        &self,
        claim: &HistoryClaim,
        error: &str,
        retry: u64,
    ) -> Result<(), GlossaryError> {
        let changed = sqlx::query("UPDATE glossary_channels SET history_lease=NULL,history_until=NULL,history_error=$4,next_history=now()+$5::bigint*interval '1 second' WHERE owner=$1 AND workspace=$2 AND history_lease=$3 AND history_until>now()")
            .bind(claim.owner).bind(claim.workspace).bind(claim.lease).bind(error).bind(i64::try_from(retry).map_err(storage)?).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed == 1 {
            Ok(())
        } else {
            Err(GlossaryError::Conflict)
        }
    }
}
