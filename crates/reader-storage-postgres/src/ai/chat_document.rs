use super::*;
use serde_json::{Map, Value};

const INPUT_FIELDS: &[&str] = &[
    "snapshot",
    "system_prompt",
    "generation_mode",
    "cost_rates",
    "max_output_tokens",
    "limits",
    "review",
];

/// Mutable progress and pinned provider inputs have separate TOAST values.
/// Ordinary progress never rewrites the large article/prompt payload. Snapshot
/// resolution is the only permitted input mutation: absent -> pinned exactly once.
pub(super) fn encode(record: &ChatRecord) -> Result<(String, String), AiError> {
    let Value::Object(mut progress) = serde_json::to_value(record).map_err(storage)? else {
        return Err(AiError::Storage);
    };
    let mut inputs = Map::new();
    for &field in INPUT_FIELDS {
        inputs.insert(
            field.into(),
            progress.remove(field).ok_or(AiError::Storage)?,
        );
    }
    Ok((super::encode(&progress)?, super::encode(&inputs)?))
}

/// SQL returns a pair of opaque JSON strings, so malformed stored JSON reaches
/// the quarantine boundary instead of aborting the entire queue query in SQL.
pub(super) fn decode(pair: &str) -> Result<ChatRecord, AiError> {
    let (progress, inputs): (String, String) = super::decode(pair)?;
    let mut progress: Map<String, Value> = super::decode(&progress)?;
    let inputs: Map<String, Value> = super::decode(&inputs)?;
    if inputs.len() != INPUT_FIELDS.len()
        || INPUT_FIELDS
            .iter()
            .any(|field| !inputs.contains_key(*field) || progress.contains_key(*field))
    {
        return Err(AiError::Storage);
    }
    progress.extend(inputs);
    serde_json::from_value(Value::Object(progress)).map_err(storage)
}

/// Mutable state plus its validated public projection. It deliberately contains
/// no article or provider prompt; worker inputs are loaded once by claim().
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Progress {
    pub owner: Uuid,
    pub view: ArticleChat,
    pub operations: Vec<Operation>,
}

pub(super) struct LockedProgress {
    pub record: Progress,
    pub published: ArticleChat,
}

pub(super) async fn locked_progress(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    id: Uuid,
) -> Result<LockedProgress, AiError> {
    let row: Option<(Uuid,Uuid,String,String,String)> = sqlx::query_as("SELECT c.workspace,c.article,c.document,c.public_view,c.status FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.id=$1 AND c.owner=$2 AND w.document::jsonb->>'owner'=$2::text FOR UPDATE OF c")
        .bind(id).bind(owner).fetch_optional(&mut **tx).await.map_err(storage)?;
    let (workspace, article, document, public, status) = row.ok_or(AiError::NotFound)?;
    if status == "quarantined" {
        return Err(AiError::Storage);
    }
    let record: Progress = super::decode(&document)?;
    let published: ArticleChat = super::decode(&public)?;
    if record.owner != owner
        || record.view.id != id
        || record.view.workspace_id != workspace
        || record.view.article_id != article
        || published.id != id
        || published.workspace_id != record.view.workspace_id
        || published.article_id != record.view.article_id
    {
        return Err(AiError::Storage);
    }
    Ok(LockedProgress { record, published })
}

pub(super) async fn write_progress(
    tx: &mut Transaction<'_, Postgres>,
    state: &LockedProgress,
) -> Result<(), AiError> {
    let record = &state.record;
    let mut public = record.view.clone();
    for message in &mut public.messages {
        if message.purpose == Some(MessagePurpose::Summary)
            && message.status != MessageStatus::Complete
        {
            message.content = state
                .published
                .messages
                .iter()
                .find(|old| old.id == message.id)
                .map(|old| old.content.clone())
                .ok_or(AiError::Storage)?;
        }
    }
    let changed = sqlx::query("UPDATE ai_chats SET document=$3,status=$4,public_view=$5,public_revision=public_revision+1 WHERE id=$1 AND owner=$2")
        .bind(record.view.id).bind(record.owner).bind(super::encode(record)?).bind(status(record.view.status)).bind(super::encode(&public)?)
        .execute(&mut **tx).await.map_err(storage)?.rows_affected();
    if changed != 1 {
        return Err(AiError::Conflict);
    }
    Ok(())
}

/// Offline projection rebuild validates each exact retained input before creating
/// the public cache. Invalid history aborts the enclosing upgrade transaction.
pub(crate) async fn backfill_public_views(
    tx: &mut Transaction<'_, Postgres>,
    batch: std::num::NonZeroU32,
) -> Result<(), sqlx::Error> {
    sqlx::raw_sql("ALTER TABLE ai_chats ADD COLUMN public_view TEXT; ALTER TABLE ai_chats ADD COLUMN public_revision BIGINT NOT NULL DEFAULT 0 CHECK(public_revision>=0);").execute(&mut **tx).await?;
    let mut after: Option<Uuid> = None;
    loop {
        let rows: Vec<(Uuid,Uuid,Uuid,Uuid,String)> = sqlx::query_as("SELECT id,owner,workspace,article,json_build_array(document,inputs)::text FROM ai_chats WHERE $1::uuid IS NULL OR id>$1 ORDER BY id LIMIT $2")
            .bind(after).bind(i64::from(batch.get())).fetch_all(&mut **tx).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let id = row.0;
            let view = quarantine::checked_chat(row)
                .and_then(ChatRecord::into_public_view)
                .map_err(|_| {
                    sqlx::Error::Protocol(format!(
                        "invalid AI history for chat {id}; upgrade rolled back"
                    ))
                })?;
            sqlx::query("UPDATE ai_chats SET public_view=$2 WHERE id=$1")
                .bind(id)
                .bind(
                    serde_json::to_string(&view)
                        .map_err(|_| sqlx::Error::Protocol("public view encoding failed".into()))?,
                )
                .execute(&mut **tx)
                .await?;
            after = Some(id);
        }
    }
    sqlx::query("ALTER TABLE ai_chats ALTER COLUMN public_view SET NOT NULL")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
