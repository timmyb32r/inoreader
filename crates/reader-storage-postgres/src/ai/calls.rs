//! Durable per-request accounting and draft handoff, separate from publication.
use super::*;

pub(super) fn interrupt(record: &mut ChatRecord) {
    for call in &mut record.view.provider_calls {
        if call.status == CallStatus::Started {
            call.status = if record.view.status == ChatStatus::Cancelled {
                CallStatus::Cancelled
            } else {
                CallStatus::Interrupted
            };
        }
    }
}

async fn fenced(tx: &mut Transaction<'_, Postgres>, claim: &ClaimedChat) -> Result<bool, AiError> {
    sqlx::query_scalar("SELECT COALESCE(lease=$2 AND lease_until>now() AND status IN ('queued','waiting_content','generating','verifying'),false) FROM ai_chats WHERE id=$1")
        .bind(claim.record.view.id).bind(claim.lease).fetch_one(&mut **tx).await.map_err(storage)
}

pub(super) async fn begin(
    pool: &PgPool,
    claim: &ClaimedChat,
    phase: GenerationPhase,
) -> Result<Uuid, AiError> {
    let mut tx = pool.begin().await.map_err(storage)?;
    let mut record = locked(&mut tx, claim.record.owner, claim.record.view.id).await?;
    if !fenced(&mut tx, claim).await? {
        return Err(AiError::Cancelled);
    }
    let attempt = record.operations.last().ok_or(AiError::Storage)?;
    if attempt.id != claim.record.operations.last().ok_or(AiError::Storage)?.id
        || attempt.task.phase() != phase
        || record
            .view
            .provider_calls
            .iter()
            .any(|c| c.assistant_id == attempt.assistant_id && c.phase == phase)
    {
        return Err(AiError::Conflict);
    }
    let assistant_id = attempt.assistant_id;
    let id = Uuid::new_v4();
    record.view.provider_calls.push(ProviderCall {
        id,
        assistant_id,
        phase,
        status: CallStatus::Started,
        usage: None,
    });
    let message = record
        .view
        .messages
        .iter_mut()
        .find(|m| m.id == assistant_id)
        .ok_or(AiError::Storage)?;
    message.phase = Some(phase);
    message.status = MessageStatus::Streaming;
    record.view.status = match phase {
        GenerationPhase::Generating => ChatStatus::Generating,
        GenerationPhase::Verifying => ChatStatus::Verifying,
    };
    write_record(&mut tx, &record).await?;
    tx.commit().await.map_err(storage)?;
    Ok(id)
}

pub(super) async fn update(
    pool: &PgPool,
    claim: &ClaimedChat,
    id: Uuid,
    update: CallUpdate,
) -> Result<(), AiError> {
    let mut tx = pool.begin().await.map_err(storage)?;
    let mut record = locked(&mut tx, claim.record.owner, claim.record.view.id).await?;
    let assistant_id = claim
        .record
        .operations
        .last()
        .ok_or(AiError::Storage)?
        .assistant_id;
    if let CallUpdate::Usage(usage) = &update {
        let phase = record
            .view
            .provider_calls
            .iter()
            .find(|c| c.id == id && c.assistant_id == assistant_id)
            .ok_or(AiError::Conflict)?
            .phase;
        let rates = if phase == GenerationPhase::Verifying {
            &record.review.cost_rates
        } else {
            &record.cost_rates
        };
        if usage
            .prompt_cache_hit_tokens
            .checked_add(usage.prompt_cache_miss_tokens)
            != Some(usage.prompt_tokens)
            || usage.estimated_cost_usd.as_deref() != Some(rates.cost(usage)?.as_str())
        {
            return Err(AiError::Protocol);
        }
    }
    let call = record
        .view
        .provider_calls
        .iter_mut()
        .find(|c| c.id == id && c.assistant_id == assistant_id)
        .ok_or(AiError::Conflict)?;
    match update {
        CallUpdate::Usage(usage) => {
            if call.usage.as_ref().is_some_and(|old| old != &usage) {
                return Err(AiError::Conflict);
            }
            call.usage = Some(usage);
        }
        CallUpdate::Finished { status, draft } => {
            if status == CallStatus::Started {
                return Err(AiError::Conflict);
            }
            if matches!(call.status, CallStatus::Completed | CallStatus::Failed)
                && call.status != status
            {
                return Err(AiError::Conflict);
            }
            if let Some(draft) = draft {
                if status != CallStatus::Completed || call.phase != GenerationPhase::Generating {
                    return Err(AiError::Conflict);
                }
                let source = record.snapshot.as_ref().ok_or(AiError::Storage)?;
                // Revalidate retained transport against the exact snapshot at
                // this storage boundary; internal callers are not an exemption.
                CompletedGeneration::new(
                    draft.clone(),
                    call.usage.clone().ok_or(AiError::Storage)?,
                    &source.text,
                    record.limits.max_response_bytes,
                )?;
                let op = record
                    .operations
                    .iter_mut()
                    .find(|op| op.assistant_id == assistant_id)
                    .ok_or(AiError::Storage)?;
                let AttemptTask::Summary { draft: saved } = &mut op.task else {
                    return Err(AiError::Conflict);
                };
                if saved.as_ref().is_some_and(|old| old != &draft) {
                    return Err(AiError::Conflict);
                }
                *saved = Some(draft);
            }
            call.status = status;
        }
    }
    // An old worker may still report known billing after Stop or expiry. Keep
    // that accounting without reviving its job or writing into a newer attempt.
    if record.operations.last().is_some_and(|op| {
        op.assistant_id == assistant_id && op.task.phase() == GenerationPhase::Verifying
    }) && fenced(&mut tx, claim).await?
    {
        record.view.status = ChatStatus::Verifying;
        let message = record
            .view
            .messages
            .iter_mut()
            .find(|m| m.id == assistant_id)
            .ok_or(AiError::Storage)?;
        message.phase = Some(GenerationPhase::Verifying);
    }
    write_record(&mut tx, &record).await?;
    tx.commit().await.map_err(storage)
}
