//! Durable per-request accounting and draft handoff, separate from publication.
use super::*;

pub(super) fn interrupt(view: &mut ArticleChat) {
    for call in &mut view.provider_calls {
        if call.status == CallStatus::Started {
            call.status = if view.status == ChatStatus::Cancelled {
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
    reservation: &SpendReservation,
    model: &CallModel,
) -> Result<Uuid, AiError> {
    let mut tx = pool.begin().await.map_err(storage)?;
    let mut state =
        chat_document::locked_progress(&mut tx, claim.record.owner, claim.record.view.id).await?;
    let record = &mut state.record;
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
    budget::reserve(&mut tx, claim.record.owner, id, reservation).await?;
    sqlx::query("INSERT INTO ai_call_models(id,owner,document) VALUES($1,$2,$3)")
        .bind(id)
        .bind(claim.record.owner)
        .bind(encode(model)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    if phase == GenerationPhase::Generating {
        record.view.model = model.model().id().into();
    }
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
    record.view.error = None;
    record.view.status = match phase {
        GenerationPhase::Generating => ChatStatus::Generating,
        GenerationPhase::Verifying => ChatStatus::Verifying,
    };
    chat_document::write_progress(&mut tx, &state).await?;
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
    let mut state =
        chat_document::locked_progress(&mut tx, claim.record.owner, claim.record.view.id).await?;
    let record = &mut state.record;
    let assistant_id = claim
        .record
        .operations
        .last()
        .ok_or(AiError::Storage)?
        .assistant_id;
    if let CallUpdate::Usage(usage) = &update {
        let raw: String =
            sqlx::query_scalar("SELECT document FROM ai_call_models WHERE id=$1 AND owner=$2")
                .bind(id)
                .bind(claim.record.owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        let model: CallModel = decode(&raw)?;
        let rates = model.rates();
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
        CallUpdate::Response { content, system } => {
            if content.len() > claim.record.limits.max_response_bytes || system.is_empty() {
                return Err(AiError::Conflict);
            }
            let changed = sqlx::query("INSERT INTO ai_call_responses(id,owner,content,system_prompt) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO UPDATE SET content=EXCLUDED.content WHERE ai_call_responses.owner=EXCLUDED.owner AND ai_call_responses.content=EXCLUDED.content AND ai_call_responses.system_prompt=EXCLUDED.system_prompt")
                .bind(id).bind(claim.record.owner).bind(content).bind(system).execute(&mut *tx).await.map_err(storage)?.rows_affected();
            if changed != 1 {
                return Err(AiError::Conflict);
            }
        }
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
                let source = claim.record.snapshot.as_ref().ok_or(AiError::Storage)?;
                // Revalidate retained transport against the exact snapshot at
                // this storage boundary; internal callers are not an exemption.
                let completed = CompletedGeneration::new(
                    draft.clone(),
                    call.usage.clone().ok_or(AiError::Storage)?,
                    &source.text,
                    claim.record.limits.max_response_bytes,
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
                state
                    .published
                    .messages
                    .iter_mut()
                    .find(|m| m.id == assistant_id)
                    .ok_or(AiError::Storage)?
                    .content = completed.content().to_owned();
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
    chat_document::write_progress(&mut tx, &state).await?;
    tx.commit().await.map_err(storage)
}
