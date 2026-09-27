use super::*;

pub(super) type ChatRow = (Uuid, Uuid, Uuid, Uuid, String);
pub(super) fn checked_chat(
    (id, owner, workspace, article, document): ChatRow,
) -> Result<ChatRecord, AiError> {
    let record: ChatRecord = chat_document::decode(&document)?;
    if record.view.id != id
        || record.owner != owner
        || record.view.workspace_id != workspace
        || record.view.article_id != article
    {
        return Err(AiError::Storage);
    }
    Ok(record)
}

// Quarantine changes only queue metadata. The original invalid document and all
// provider accounting/raw responses remain available for explicit repair.
pub(super) async fn chat(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(), AiError> {
    sqlx::query("UPDATE ai_chats SET status='quarantined',lease=NULL,lease_until=NULL WHERE id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    log::error!(
        "ai_record_invalid class=chat operation_id={id} action=quarantined_original_retained"
    );
    Ok(())
}
pub(super) async fn definitions(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), AiError> {
    sqlx::query(
        "UPDATE ai_definitions SET status='quarantined',lease=NULL,lease_until=NULL WHERE id=$1",
    )
    .bind(id)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    log::error!("ai_record_invalid class=definitions operation_id={id} action=quarantined_original_retained");
    Ok(())
}
