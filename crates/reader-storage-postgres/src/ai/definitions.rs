use super::*;

type Row = (Uuid, Uuid, Uuid, Uuid, String);
fn checked((id, owner, workspace, article, document): Row) -> Result<DefinitionsRecord, AiError> {
    let record: DefinitionsRecord = decode(&document)?;
    let job = record.job();
    if job.id != id
        || record.owner() != owner
        || job.workspace_id != workspace
        || job.article_id != article
    {
        return Err(AiError::Storage);
    }
    Ok(record)
}
impl PostgresAiStore {
    pub(super) async fn latest_definitions(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Option<DefinitionsJob>, AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        let row:Option<Row>=sqlx::query_as("SELECT id,owner,workspace,article,document FROM ai_definitions WHERE owner=$1 AND workspace=$2 AND status<>'cancelled' AND (article=$3 OR article::text IN (SELECT history_article_id FROM article_history_links WHERE workspace_id=$2::text AND article_id=$3::text)) ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(owner).bind(workspace).bind(article).fetch_optional(&self.pool).await.map_err(storage)?;
        row.map(|r| Ok(checked(r)?.job().clone())).transpose()
    }
    pub(super) async fn insert_definitions(
        &self,
        record: DefinitionsRecord,
        operation: Uuid,
        regenerate: bool,
        automatic: bool,
    ) -> Result<DefinitionsJob, AiError> {
        if automatic && regenerate {
            return Err(AiError::Conflict);
        }
        if operation.is_nil() {
            return Err(AiError::Message);
        }
        let owner = record.owner();
        let j = record.job();
        owned_workspace(&self.pool, owner, j.workspace_id).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // The lock hash is never identity: exact input and scope determine reuse.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("definitions/{owner}"))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let old:Option<Row>=sqlx::query_as("SELECT d.id,d.owner,d.workspace,d.article,d.document FROM ai_definition_operations o JOIN ai_definitions d ON d.id=o.job WHERE o.owner=$1 AND o.operation=$2")
            .bind(owner).bind(operation).fetch_optional(&mut *tx).await.map_err(storage)?;
        if let Some(row) = old {
            let previous_mode: bool = sqlx::query_scalar(
                "SELECT regenerate FROM ai_definition_operations WHERE owner=$1 AND operation=$2",
            )
            .bind(owner)
            .bind(operation)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
            if previous_mode != regenerate {
                return Err(AiError::Conflict);
            }
            let old = checked(row)?;
            if old.job().workspace_id != j.workspace_id
                || old.job().article_id != j.article_id
                || if automatic {
                    old.job().prompt_version != j.prompt_version
                } else {
                    !old.input().same_request(record.input())?
                }
            {
                return Err(AiError::Conflict);
            }
            return Ok(old.job().clone());
        }
        if automatic {
            let corrupt: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ai_definitions WHERE owner=$1 AND workspace=$2 AND article=$3 AND status='quarantined')")
                .bind(owner).bind(j.workspace_id).bind(j.article_id).fetch_one(&mut *tx).await.map_err(storage)?;
            if corrupt {
                return Err(AiError::AutomaticExcluded);
            }
        }
        let input = encode(record.input())?;
        // Retain exact inputs for provenance. Manual cache equality excludes only
        // fields absent from the paid request; automatic admission is article/prompt scoped.
        let cached:Option<Row>=sqlx::query_as("SELECT id,owner,workspace,article,document FROM ai_definitions WHERE owner=$1 AND workspace=$2 AND article=$3 AND status NOT IN ('cancelled','quarantined') AND (($6 AND document::jsonb#>>'{job,promptVersion}'=$7) OR (NOT $6 AND (input::jsonb - 'limits' #- '{snapshot,source_revision}' #- '{snapshot,safe_html}' #- '{snapshot,source_url}')=($4::jsonb - 'limits' #- '{snapshot,source_revision}' #- '{snapshot,safe_html}' #- '{snapshot,source_url}') AND (status IN ('queued','generating') OR (status='completed' AND NOT $5)))) ORDER BY created_at DESC LIMIT 1")
            .bind(owner).bind(j.workspace_id).bind(j.article_id).bind(&input).bind(regenerate).bind(automatic).bind(&j.prompt_version).fetch_optional(&mut *tx).await.map_err(storage)?;
        let job = if let Some(row) = cached {
            checked(row)?.job().clone()
        } else {
            sqlx::query("INSERT INTO ai_definitions(id,owner,workspace,article,status,document,input,automatic) VALUES($1,$2,$3,$4,'queued',$5,$6,$7)")
                .bind(j.id).bind(owner).bind(j.workspace_id).bind(j.article_id).bind(encode(&record)?).bind(input).bind(automatic).execute(&mut *tx).await.map_err(storage)?;
            j.clone()
        };
        if !automatic {
            sqlx::query("UPDATE ai_definitions SET automatic=false,scheduled_at=now() WHERE owner=$1 AND id=$2")
                .bind(owner)
                .bind(job.id)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        sqlx::query("INSERT INTO ai_definition_operations(owner,operation,job,workspace,article,regenerate) VALUES($1,$2,$3,$4,$5,$6)").bind(owner).bind(operation).bind(job.id).bind(j.workspace_id).bind(j.article_id).bind(regenerate).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(job)
    }
    pub(super) async fn lease_definitions(
        &self,
        seconds: u64,
    ) -> Result<Option<ClaimedDefinitions>, AiError> {
        let seconds = i64::try_from(seconds).map_err(|_| AiError::Configuration)?;
        if seconds <= 0 {
            return Err(AiError::Configuration);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let duplicates: Vec<Row> = sqlx::query_as("SELECT d.id,d.owner,d.workspace,d.article,d.document FROM ai_definitions d JOIN workspaces w ON w.id=d.workspace::text WHERE d.status='queued' AND d.automatic AND w.document::jsonb->>'owner'=d.owner::text AND reader_ai_terms_superseded(d.id) ORDER BY d.created_at,d.id LIMIT $1 FOR UPDATE OF d SKIP LOCKED")
            .bind(i64::from(self.recovery_batch.get())).fetch_all(&mut *tx).await.map_err(storage)?;
        for row in duplicates {
            let id = row.0;
            match checked(row).and_then(|mut record| {
                record.cancel_duplicate()?;
                Ok(record)
            }) {
                Ok(record) => {
                    sqlx::query("UPDATE ai_definitions SET status='cancelled',document=$2,lease=NULL,lease_until=NULL WHERE id=$1")
                        .bind(id).bind(encode(&record)?).execute(&mut *tx).await.map_err(storage)?;
                }
                Err(_) => quarantine::definitions(&mut tx, id).await?,
            }
        }
        let expired: Vec<Row> = sqlx::query_as("SELECT id,owner,workspace,article,document FROM ai_definitions WHERE status='generating' AND lease_until<now() ORDER BY lease_until,id LIMIT $1 FOR UPDATE SKIP LOCKED").bind(i64::from(self.recovery_batch.get()))
            .fetch_all(&mut *tx).await.map_err(storage)?;
        for row in expired {
            let id = row.0;
            match checked(row).and_then(|mut record| {
                record.finish(
                    DefinitionState::Failed {
                        error: "Запрос прерван; повторите явно. Провайдер мог списать оплату."
                            .into(),
                    },
                    None,
                )?;
                Ok(record)
            }) {
                Ok(record) => {
                    sqlx::query("UPDATE ai_definitions SET status='failed',lease=NULL,lease_until=NULL,document=$2 WHERE id=$1")
                        .bind(id).bind(encode(&record)?).execute(&mut *tx).await.map_err(storage)?;
                }
                Err(_) => quarantine::definitions(&mut tx, id).await?,
            }
        }
        let row:Option<Row>=sqlx::query_as("SELECT d.id,d.owner,d.workspace,d.article,d.document FROM ai_definitions d JOIN workspaces w ON w.id=d.workspace::text WHERE d.status='queued' AND NOT reader_ai_terms_superseded(d.id) AND d.scheduled_at<=now() AND (NOT d.automatic OR (SELECT reader_ai_automatic_resume_at(statement_timestamp())) IS NULL) AND w.document::jsonb->>'owner'=d.owner::text ORDER BY d.created_at,d.id LIMIT 1 FOR UPDATE OF d SKIP LOCKED")
            .fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let id = row.0;
        let record = match checked(row).and_then(|mut record| {
            record.mark_running()?;
            Ok(record)
        }) {
            Ok(record) => record,
            Err(_) => {
                quarantine::definitions(&mut tx, id).await?;
                tx.commit().await.map_err(storage)?;
                return Ok(None);
            }
        };
        let lease = Uuid::new_v4();
        let queue_wait_us: i64 = sqlx::query_scalar("UPDATE ai_definitions SET status='generating',document=$2,lease=$3,lease_until=now()+$4::bigint*interval '1 second' WHERE id=$1 RETURNING (extract(epoch FROM now()-created_at)*1000000)::bigint")
            .bind(record.job().id).bind(encode(&record)?).bind(lease).bind(seconds).fetch_one(&mut *tx).await.map_err(storage)?;
        log::info!(
            "ai_claim class=definitions job_id={} queue_wait_us={}",
            record.job().id,
            queue_wait_us
        );
        tx.commit().await.map_err(storage)?;
        Ok(Some(ClaimedDefinitions { record, lease }))
    }
    pub(super) async fn complete_definitions(
        &self,
        claim: &ClaimedDefinitions,
        state: DefinitionState,
        usage: Option<Usage>,
    ) -> Result<(), AiError> {
        let status = match &state {
            DefinitionState::Completed { .. } => "completed",
            DefinitionState::Failed { .. } => "failed",
            DefinitionState::Cancelled { .. } => "cancelled",
            _ => return Err(AiError::Protocol),
        };
        let mut record = claim.record.clone();
        record.finish(state, usage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let n=sqlx::query("UPDATE ai_definitions d SET status=$4,document=$5,lease=NULL,lease_until=NULL FROM workspaces w WHERE d.id=$1 AND d.owner=$2 AND d.lease=$3 AND d.lease_until>now() AND d.status='generating' AND w.id=d.workspace::text AND w.document::jsonb->>'owner'=$2::text")
            .bind(record.job().id).bind(record.owner()).bind(claim.lease).bind(status).bind(encode(&record)?).execute(&mut *tx).await.map_err(storage)?.rows_affected();
        tx.commit().await.map_err(storage)?;
        if n != 1 {
            return Err(AiError::Cancelled);
        }
        Ok(())
    }
}
