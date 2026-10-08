use super::*;
impl PostgresAiStore {
    pub(super) async fn enroll_automatic(&self, owners: &[Uuid]) -> Result<(), AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // INSERT RETURNING freezes the unread backfill exactly once per account.
        sqlx::query("WITH enrolled AS (INSERT INTO ai_automatic_accounts(owner) SELECT owner FROM ai_profiles WHERE owner=ANY($1) ON CONFLICT DO NOTHING RETURNING owner) INSERT INTO ai_summary_queue(owner,workspace,article) SELECT e.owner,a.workspace_key::uuid,a.article_key::uuid FROM enrolled e JOIN workspaces w ON w.document::jsonb->>'owner'=e.owner::text JOIN articles a ON a.workspace_key=w.id WHERE NOT a.is_read AND reader_ai_automatic_allowed(e.owner,w.id,a.article_key) ON CONFLICT DO NOTHING")
            .bind(owners).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    pub(super) async fn automatic_next(
        &self,
        owners: &[Uuid],
        attempts: u32,
        retry_seconds: u64,
    ) -> Result<Option<AutoSummary>, AiError> {
        let row:Option<(Uuid,Uuid,Uuid)>=sqlx::query_as("WITH candidate AS (SELECT q.owner,q.workspace,q.article FROM ai_summary_queue q JOIN ai_profiles p ON p.owner=q.owner JOIN workspaces w ON w.id=q.workspace::text AND w.document::jsonb->>'owner'=q.owner::text WHERE (SELECT reader_ai_automatic_resume_at(statement_timestamp())) IS NULL AND q.owner=ANY($1) AND reader_ai_automatic_allowed(q.owner,q.workspace::text,q.article::text) AND NOT q.dispatched AND q.attempts<$2 AND q.scheduled_at<=now() AND EXISTS(SELECT 1 FROM library_origins o JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=q.workspace::text AND o.article_id=q.article::text) ORDER BY q.scheduled_at,q.article LIMIT 1 FOR UPDATE OF q SKIP LOCKED) UPDATE ai_summary_queue q SET attempts=q.attempts+1,scheduled_at=now()+$3::bigint*interval '1 second' FROM candidate c WHERE q.owner=c.owner AND q.workspace=c.workspace AND q.article=c.article RETURNING q.owner,q.workspace,q.article")
            .bind(owners).bind(i64::from(attempts)).bind(i64::try_from(retry_seconds).map_err(storage)?).fetch_optional(&self.pool).await.map_err(storage)?;
        Ok(row.map(|(owner, workspace, article)| AutoSummary {
            owner,
            workspace,
            article,
        }))
    }
    pub(super) async fn defer_chat(
        &self,
        claim: &ClaimedChat,
        reason: reader_ai::AiDeferral,
    ) -> Result<(), AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut state =
            chat_document::locked_progress(&mut tx, claim.record.owner, claim.record.view.id)
                .await?;
        let active: bool = sqlx::query_scalar(
            "SELECT COALESCE(lease=$2 AND lease_until>now(),false) FROM ai_chats WHERE id=$1",
        )
        .bind(claim.record.view.id)
        .bind(claim.lease)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if !active {
            return Err(AiError::Cancelled);
        }
        state.record.view.status = ChatStatus::Queued;
        state.record.view.error = Some(
            match reason {
                reader_ai::AiDeferral::DailyBudget => AiError::Budget,
                reader_ai::AiDeferral::PeakHours => AiError::AutomaticPaused,
            }
            .to_string(),
        );
        set_attempt(
            &mut state.record.view,
            &state.record.operations,
            MessageStatus::Pending,
        )?;
        chat_document::write_progress(&mut tx, &state).await?;
        sqlx::query("UPDATE ai_chats SET lease=NULL,lease_until=NULL,scheduled_at=reader_ai_deferred_until($2,clock_timestamp()) WHERE id=$1").bind(claim.record.view.id).bind(reason==reader_ai::AiDeferral::PeakHours).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
}
