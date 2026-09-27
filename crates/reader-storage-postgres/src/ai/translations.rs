use super::*;

type TranslationRow = (Uuid, Uuid, Uuid, Uuid, String);
fn checked_record(
    (id, owner, workspace, article, document): TranslationRow,
) -> Result<TranslationRecord, AiError> {
    let record: TranslationRecord = decode(&document)?;
    if record.owner != owner
        || record.job.id != id
        || record.job.workspace_id != workspace
        || record.job.article_id != article
        || record.job.source.trim().is_empty()
    {
        return Err(AiError::Storage);
    }
    if let TranslationState::Completed { result } = &record.job.state {
        if result.source() != record.job.source {
            return Err(AiError::Storage);
        }
    }
    Ok(record)
}

impl PostgresAiStore {
    pub(super) async fn list_translations(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ParagraphJob>, AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        let rows:Vec<TranslationRow>=sqlx::query_as("SELECT t.id,t.owner,t.workspace,t.article,t.document FROM ai_translations t JOIN workspaces w ON w.id=t.workspace::text WHERE t.owner=$1 AND t.workspace=$2 AND t.article=$3 AND w.document::jsonb->>'owner'=$1::text ORDER BY t.created_at DESC,t.id DESC")
            .bind(owner).bind(workspace).bind(article).fetch_all(&self.pool).await.map_err(storage)?;
        rows.into_iter()
            .map(|row| Ok(checked_record(row)?.job))
            .collect()
    }
    pub(super) async fn insert_translation(
        &self,
        record: TranslationRecord,
    ) -> Result<ParagraphJob, AiError> {
        let j = &record.job;
        owned_workspace(&self.pool, record.owner, j.workspace_id).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // Serializes duplicate clicks, including two tabs with different request
        // IDs. Hash selects only a lock; equality always compares exact fields.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!(
                "translation/{}/{}/{}",
                record.owner, j.workspace_id, j.article_id
            ))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let existing: Option<String> =
            sqlx::query_scalar("SELECT document FROM ai_translations WHERE id=$1 AND owner=$2")
                .bind(j.id)
                .bind(record.owner)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        if let Some(row) = existing {
            let r = checked_record((j.id, record.owner, j.workspace_id, j.article_id, row))?;
            if r.job.workspace_id != j.workspace_id
                || r.job.article_id != j.article_id
                || r.job.source != j.source
            {
                return Err(AiError::Conflict);
            }
            return Ok(r.job);
        }
        let cached:Option<String>=sqlx::query_scalar("SELECT document FROM ai_translations WHERE owner=$1 AND workspace=$2 AND article=$3 AND status IN ('queued','generating','completed') AND document::jsonb->>'source_revision'=$4 AND document::jsonb->'job'->>'source'=$5 ORDER BY created_at DESC LIMIT 1")
            .bind(record.owner).bind(j.workspace_id).bind(j.article_id).bind(&record.source_revision).bind(&j.source).fetch_optional(&mut *tx).await.map_err(storage)?;
        if let Some(row) = cached {
            let cached: TranslationRecord = decode(&row)?;
            return Ok(checked_record((
                cached.job.id,
                record.owner,
                j.workspace_id,
                j.article_id,
                row,
            ))?
            .job);
        }
        sqlx::query("INSERT INTO ai_translations(id,owner,workspace,article,status,document) VALUES($1,$2,$3,$4,'queued',$5)")
            .bind(j.id).bind(record.owner).bind(j.workspace_id).bind(j.article_id).bind(encode(&record)?).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(record.job)
    }
    pub(super) async fn lease_translation(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<ClaimedTranslation>, AiError> {
        let seconds = i64::try_from(lease_seconds).map_err(|_| AiError::Configuration)?;
        if seconds <= 0 {
            return Err(AiError::Configuration);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("UPDATE ai_translations SET status='failed',lease=NULL,lease_until=NULL,document=jsonb_set(document::jsonb,'{job}',document::jsonb->'job' || jsonb_build_object('status','failed','error','Translation was interrupted; retry explicitly. The request may have been charged.'))::text WHERE status='generating' AND lease_until<now()")
            .execute(&mut *tx).await.map_err(storage)?;
        let row:Option<TranslationRow>=sqlx::query_as("SELECT t.id,t.owner,t.workspace,t.article,t.document FROM ai_translations t JOIN workspaces w ON w.id=t.workspace::text WHERE t.status='queued' AND w.document::jsonb->>'owner'=t.owner::text ORDER BY t.created_at,t.id LIMIT 1 FOR UPDATE OF t SKIP LOCKED")
            .fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let mut record = checked_record(row)?;
        record.job.state = TranslationState::Generating;
        let lease = Uuid::new_v4();
        sqlx::query("UPDATE ai_translations SET status='generating',document=$2,lease=$3,lease_until=now()+$4::bigint*interval '1 second' WHERE id=$1")
            .bind(record.job.id).bind(encode(&record)?).bind(lease).bind(seconds).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(Some(ClaimedTranslation { record, lease }))
    }
    pub(super) async fn complete_translation(
        &self,
        claim: &ClaimedTranslation,
        state: TranslationState,
        usage: Option<Usage>,
    ) -> Result<(), AiError> {
        let status = match &state {
            TranslationState::Completed { result } => {
                if result.source() != claim.record.job.source {
                    return Err(AiError::Protocol);
                }
                "completed"
            }
            TranslationState::Failed { .. } => "failed",
            _ => return Err(AiError::Protocol),
        };
        let mut record = claim.record.clone();
        record.job.state = state;
        record.job.usage = usage;
        let n=sqlx::query("UPDATE ai_translations t SET status=$4,document=$5,lease=NULL,lease_until=NULL FROM workspaces w WHERE t.id=$1 AND t.owner=$2 AND t.lease=$3 AND t.lease_until>now() AND t.status='generating' AND w.id=t.workspace::text AND w.document::jsonb->>'owner'=$2::text")
            .bind(record.job.id).bind(record.owner).bind(claim.lease).bind(status).bind(encode(&record)?).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if n != 1 {
            return Err(AiError::Cancelled);
        }
        Ok(())
    }
}
