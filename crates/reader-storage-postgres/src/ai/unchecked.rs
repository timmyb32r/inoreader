//! Explicit account opt-out publishes the retained draft without inventing a
//! verification call or discarding the original paid response and accounting.
use super::*;

pub(super) async fn finish(pool: &PgPool, claim: &ClaimedChat) -> Result<bool, AiError> {
    let mut tx = pool.begin().await.map_err(storage)?;
    let mut state =
        chat_document::locked_progress(&mut tx, claim.record.owner, claim.record.view.id).await?;
    if !calls::fenced(&mut tx, claim).await? {
        return Err(AiError::Cancelled);
    }
    // This read is the admission point for unchecked publication. A subsequent
    // preference edit affects future stages, never a result already admitted.
    let raw: Option<String> =
        sqlx::query_scalar("SELECT document FROM ai_model_preferences WHERE owner=$1")
            .bind(claim.record.owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
    let preferences: ModelPreferences =
        raw.map(|raw| decode(&raw)).transpose()?.unwrap_or_default();
    if preferences.verification.is_some() {
        return Ok(false);
    }
    let record = &mut state.record;
    let attempt = record.operations.last().ok_or(AiError::Storage)?;
    if attempt.id != claim.record.operations.last().ok_or(AiError::Storage)?.id {
        return Err(AiError::Conflict);
    }
    let AttemptTask::Summary { draft: Some(draft) } = &attempt.task else {
        return Err(AiError::Conflict);
    };
    // A retry carries the exact draft from an earlier attempt. Its paid
    // generation remains attached to that original attempt, not the retry.
    let generated = record.view.provider_calls.iter().find(|call| {
        call.phase == GenerationPhase::Generating && call.status == CallStatus::Completed
            && record.operations.iter().any(|op| {
                op.assistant_id == call.assistant_id
                    && matches!(&op.task, AttemptTask::Summary { draft: Some(saved) } if saved == draft)
            })
    }).ok_or(AiError::Conflict)?;
    if record.view.provider_calls.iter().any(|call| {
        call.assistant_id == attempt.assistant_id && call.phase == GenerationPhase::Verifying
    }) {
        return Err(AiError::Conflict);
    }
    let source = claim.record.snapshot.as_ref().ok_or(AiError::Storage)?;
    let output = CompletedGeneration::new(
        draft.clone(),
        generated.usage.clone().ok_or(AiError::Storage)?,
        &source.text,
        claim.record.limits.max_response_bytes,
    )?;
    let message = record
        .view
        .messages
        .iter_mut()
        .find(|m| m.id == attempt.assistant_id)
        .ok_or(AiError::Storage)?;
    message.content = output.content().to_owned();
    message.phase = Some(GenerationPhase::Generating);
    message.status = MessageStatus::Complete;
    record.view.status = ChatStatus::Completed;
    record.view.error = None;
    chat_document::write_progress(&mut tx, &state).await?;
    sqlx::query("UPDATE ai_chats SET lease=NULL,lease_until=NULL,scheduled_at=now() WHERE id=$1 AND owner=$2")
        .bind(claim.record.view.id).bind(claim.record.owner)
        .execute(&mut *tx).await.map_err(storage)?;
    tx.commit().await.map_err(storage)?;
    Ok(true)
}
