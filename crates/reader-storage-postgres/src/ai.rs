//! Account-scoped persisted AI credentials, snapshots and fenced generation jobs.
use async_trait::async_trait;
use chrono::Utc;
use reader_ai::*;
use reader_application::ArticleRepository;
use reader_core::{ArticleId, WorkspaceId};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;

use crate::PostgresRepository;

mod calls;
mod definitions;
mod translations;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS ai_definitions (
 id UUID PRIMARY KEY, owner UUID NOT NULL, workspace UUID NOT NULL, article UUID NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), status TEXT NOT NULL, document TEXT NOT NULL,
 input TEXT NOT NULL, lease UUID, lease_until TIMESTAMPTZ,
 raw_response BYTEA, response_status INTEGER, response_interrupted BOOLEAN
);
CREATE INDEX IF NOT EXISTS ai_definitions_article ON ai_definitions(owner,workspace,article,created_at DESC);
CREATE INDEX IF NOT EXISTS ai_definitions_pending ON ai_definitions(status,created_at);
CREATE TABLE IF NOT EXISTS ai_definition_operations (
 owner UUID NOT NULL, operation UUID NOT NULL, job UUID NOT NULL REFERENCES ai_definitions(id),
 workspace UUID NOT NULL, article UUID NOT NULL, regenerate BOOLEAN NOT NULL, PRIMARY KEY(owner,operation)
);

CREATE TABLE IF NOT EXISTS ai_translations (
    id UUID PRIMARY KEY, owner UUID NOT NULL, workspace UUID NOT NULL, article UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(), status TEXT NOT NULL, document TEXT NOT NULL,
    lease UUID, lease_until TIMESTAMPTZ
);
ALTER TABLE ai_translations ADD COLUMN IF NOT EXISTS raw_response BYTEA;
ALTER TABLE ai_translations ADD COLUMN IF NOT EXISTS response_status INTEGER;
ALTER TABLE ai_translations ADD COLUMN IF NOT EXISTS response_interrupted BOOLEAN;
CREATE INDEX IF NOT EXISTS ai_translations_article ON ai_translations(owner,workspace,article,created_at DESC);
CREATE INDEX IF NOT EXISTS ai_translations_pending ON ai_translations(status,created_at);
CREATE TABLE IF NOT EXISTS ai_profiles (
    owner UUID PRIMARY KEY,
    encrypted_key BYTEA NOT NULL,
    balance TEXT,
    error TEXT
);
CREATE TABLE IF NOT EXISTS ai_chats (
    id UUID PRIMARY KEY,
    owner UUID NOT NULL,
    workspace UUID NOT NULL,
    article UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL,
    document TEXT NOT NULL,
    lease UUID,
    lease_until TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ai_chats_article ON ai_chats(owner, workspace, article, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS ai_chats_pending ON ai_chats(status, created_at);
CREATE TABLE IF NOT EXISTS ai_operations (
    owner UUID NOT NULL,
    operation UUID NOT NULL,
    chat UUID NOT NULL REFERENCES ai_chats(id),
    command TEXT NOT NULL,
    PRIMARY KEY(owner, operation)
);
"#;

pub struct PostgresAiStore {
    pool: PgPool,
    reader: Arc<PostgresRepository>,
}
impl PostgresAiStore {
    pub fn new(pool: PgPool, reader: Arc<PostgresRepository>) -> Self {
        Self { pool, reader }
    }
}
fn storage(_: impl std::fmt::Display) -> AiError {
    AiError::Storage
}
fn encode(value: &impl Serialize) -> Result<String, AiError> {
    serde_json::to_string(value).map_err(storage)
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T, AiError> {
    serde_json::from_str(value).map_err(storage)
}
fn status(value: ChatStatus) -> &'static str {
    match value {
        ChatStatus::WaitingContent => "waiting_content",
        ChatStatus::Queued => "queued",
        ChatStatus::Generating => "generating",
        ChatStatus::Verifying => "verifying",
        ChatStatus::Completed => "completed",
        ChatStatus::Failed => "failed",
        ChatStatus::Interrupted => "interrupted",
        ChatStatus::Cancelled => "cancelled",
    }
}
async fn owned_workspace(pool: &PgPool, owner: Uuid, workspace: Uuid) -> Result<(), AiError> {
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=$1 AND document::jsonb->>'owner'=$2)",
    )
    .bind(workspace.to_string())
    .bind(owner.to_string())
    .fetch_one(pool)
    .await
    .map_err(storage)?;
    if valid {
        Ok(())
    } else {
        Err(AiError::NotFound)
    }
}
async fn locked(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    id: Uuid,
) -> Result<ChatRecord, AiError> {
    let document: Option<String> = sqlx::query_scalar("SELECT c.document FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.id=$1 AND c.owner=$2 AND w.document::jsonb->>'owner'=$2::text FOR UPDATE OF c")
        .bind(id).bind(owner).fetch_optional(&mut **tx).await.map_err(storage)?;
    let record: ChatRecord = decode(&document.ok_or(AiError::NotFound)?)?;
    if record.owner != owner || record.view.id != id {
        return Err(AiError::Storage);
    }
    Ok(record)
}
async fn write_record(
    tx: &mut Transaction<'_, Postgres>,
    record: &ChatRecord,
) -> Result<(), AiError> {
    sqlx::query("UPDATE ai_chats SET document=$3,status=$4 WHERE id=$1 AND owner=$2")
        .bind(record.view.id)
        .bind(record.owner)
        .bind(encode(record)?)
        .bind(status(record.view.status))
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}
async fn prior_operation(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    operation: Uuid,
) -> Result<Option<(Uuid, OperationKind)>, AiError> {
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT chat,command FROM ai_operations WHERE owner=$1 AND operation=$2")
            .bind(owner)
            .bind(operation)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?;
    row.map(|(id, command)| Ok((id, decode(&command)?)))
        .transpose()
}
async fn operation_lock(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    op: Uuid,
) -> Result<(), AiError> {
    // Hash only chooses a lock, never logical identity; equality uses exact UUIDs.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("ai-operation/{owner}/{op}"))
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}
async fn save_operation(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    chat: Uuid,
    operation: Uuid,
    kind: &OperationKind,
) -> Result<(), AiError> {
    sqlx::query("INSERT INTO ai_operations(owner,operation,chat,command) VALUES($1,$2,$3,$4)")
        .bind(owner)
        .bind(operation)
        .bind(chat)
        .bind(encode(kind)?)
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}
fn set_attempt(record: &mut ChatRecord, state: MessageStatus) -> Result<(), AiError> {
    let id = record
        .operations
        .last()
        .ok_or(AiError::Storage)?
        .assistant_id;
    let message = record
        .view
        .messages
        .iter_mut()
        .find(|m| m.id == id)
        .ok_or(AiError::Storage)?;
    message.status = state;
    Ok(())
}

#[async_trait]
impl AiStore for PostgresAiStore {
    async fn retain_reply(
        &self,
        kind: ReplyKind,
        owner: Uuid,
        job: Uuid,
        reply: &ProviderReply,
    ) -> Result<(), AiError> {
        let table = match kind {
            ReplyKind::Translation => "ai_translations",
            ReplyKind::Definitions => "ai_definitions",
        };
        // Immutable, idempotent capture: different bytes for the same attempt are
        // corruption, never a last-write-wins overwrite. Do not require a lease.
        let sql = format!("UPDATE {table} t SET raw_response=$3,response_status=$4,response_interrupted=$5 FROM workspaces w WHERE t.id=$1 AND t.owner=$2 AND w.id=t.workspace::text AND w.document::jsonb->>'owner'=$2::text AND (t.raw_response IS NULL OR (t.raw_response=$3 AND t.response_status=$4 AND t.response_interrupted=$5))");
        let changed = sqlx::query(&sql)
            .bind(job)
            .bind(owner)
            .bind(&reply.body)
            .bind(i32::from(reply.status))
            .bind(reply.interrupted)
            .execute(&self.pool)
            .await
            .map_err(storage)?
            .rows_affected();
        if changed != 1 {
            return Err(AiError::Storage);
        }
        Ok(())
    }

    async fn definitions(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Option<DefinitionsJob>, AiError> {
        self.latest_definitions(owner, workspace, article).await
    }
    async fn create_definitions(
        &self,
        record: DefinitionsRecord,
        operation: Uuid,
        regenerate: bool,
    ) -> Result<DefinitionsJob, AiError> {
        self.insert_definitions(record, operation, regenerate).await
    }
    async fn claim_definitions(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<ClaimedDefinitions>, AiError> {
        self.lease_definitions(lease_seconds).await
    }
    async fn finish_definitions(
        &self,
        claim: &ClaimedDefinitions,
        state: DefinitionState,
        usage: Option<Usage>,
    ) -> Result<(), AiError> {
        self.complete_definitions(claim, state, usage).await
    }

    async fn translations(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ParagraphJob>, AiError> {
        self.list_translations(owner, workspace, article).await
    }
    async fn create_translation(&self, record: TranslationRecord) -> Result<ParagraphJob, AiError> {
        self.insert_translation(record).await
    }
    async fn claim_translation(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<ClaimedTranslation>, AiError> {
        self.lease_translation(lease_seconds).await
    }
    async fn finish_translation(
        &self,
        claim: &ClaimedTranslation,
        state: TranslationState,
        usage: Option<Usage>,
    ) -> Result<(), AiError> {
        self.complete_translation(claim, state, usage).await
    }

    async fn credential(&self, owner: Uuid) -> Result<Option<Vec<u8>>, AiError> {
        sqlx::query_scalar("SELECT encrypted_key FROM ai_profiles WHERE owner=$1")
            .bind(owner)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }
    async fn profile(
        &self,
        owner: Uuid,
    ) -> Result<(bool, Option<Balance>, Option<String>), AiError> {
        let row: Option<(Option<String>, Option<String>)> =
            sqlx::query_as("SELECT balance,error FROM ai_profiles WHERE owner=$1")
                .bind(owner)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        match row {
            Some((balance, error)) => Ok((true, balance.map(|v| decode(&v)).transpose()?, error)),
            None => Ok((false, None, None)),
        }
    }
    async fn save_credential(
        &self,
        owner: Uuid,
        encrypted: Vec<u8>,
        balance: Balance,
    ) -> Result<(), AiError> {
        sqlx::query("INSERT INTO ai_profiles(owner,encrypted_key,balance) VALUES($1,$2,$3) ON CONFLICT(owner) DO UPDATE SET encrypted_key=EXCLUDED.encrypted_key,balance=EXCLUDED.balance,error=NULL")
            .bind(owner).bind(encrypted).bind(encode(&balance)?).execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }
    async fn delete_credential(&self, owner: Uuid) -> Result<(), AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("DELETE FROM ai_profiles WHERE owner=$1")
            .bind(owner)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let documents: Vec<String> = sqlx::query_scalar("SELECT document FROM ai_chats WHERE owner=$1 AND status IN ('waiting_content','queued','generating','verifying') FOR UPDATE").bind(owner).fetch_all(&mut *tx).await.map_err(storage)?;
        for document in documents {
            let mut record: ChatRecord = decode(&document)?;
            record.view.status = ChatStatus::Cancelled;
            record.view.error = Some(AiError::MissingKey.to_string());
            calls::interrupt(&mut record);
            set_attempt(&mut record, MessageStatus::Interrupted)?;
            write_record(&mut tx, &record).await?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn save_balance(
        &self,
        owner: Uuid,
        expected_credential: &[u8],
        balance: Option<Balance>,
        error: Option<String>,
    ) -> Result<(), AiError> {
        let updated=sqlx::query("UPDATE ai_profiles SET balance=COALESCE($2,balance),error=$3 WHERE owner=$1 AND encrypted_key=$4")
            .bind(owner)
            .bind(balance.map(|v| encode(&v)).transpose()?)
            .bind(error)
            .bind(expected_credential)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        if updated.rows_affected() != 1 {
            return Err(AiError::Conflict);
        }
        Ok(())
    }
    async fn article_intro_contains(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        source: &str,
    ) -> Result<bool, AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        let value = self
            .reader
            .article(
                WorkspaceId::from_uuid(workspace),
                ArticleId::from_uuid(article),
            )
            .await
            .map_err(|error| match error {
                reader_application::RepositoryError::NotFound => AiError::NotFound,
                _ => AiError::Storage,
            })?;
        Ok(!source.trim().is_empty()
            && (value.key.title == source || value.key.description.as_deref() == Some(source)))
    }
    async fn article_input(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<ArticleInput, AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        let value = self
            .reader
            .article(
                WorkspaceId::from_uuid(workspace),
                ArticleId::from_uuid(article),
            )
            .await
            .map_err(|e| match e {
                reader_application::RepositoryError::NotFound => AiError::NotFound,
                _ => AiError::Storage,
            })?;
        let title = value.key.title.clone();
        let source_url = value.key.location.exact_url().unwrap_or("").to_owned();
        // One PostgreSQL statement pins the manifest and its chunks in one MVCC
        // snapshot; concurrent extraction/cleanup cannot mix content revisions.
        let row:Option<(String,String,Option<String>)> = sqlx::query_as("SELECT m.id,m.document,(SELECT json_agg(c.bytes ORDER BY c.ordinal)::text FROM staged_content_chunks c WHERE c.record_id=m.id AND c.refresh_id=m.document::jsonb->>'refresh_id' AND c.representation='safe') FROM library_origins o JOIN subscriptions s ON s.id=o.subscription_id JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=$1 AND o.article_id=$2 AND s.document::jsonb->>'workspace_id'=$1 ORDER BY (m.document::jsonb->>'fetched_at')::timestamptz DESC,m.id DESC LIMIT 1")
            .bind(workspace.to_string()).bind(article.to_string()).fetch_optional(&self.pool).await.map_err(storage)?;
        let Some((record, manifest, chunks)) = row else {
            let failed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM library_origins o JOIN content_refresh_state f ON f.id='failure/'||o.source_record_id WHERE o.workspace_id=$1 AND o.article_id=$2)").bind(workspace.to_string()).bind(article.to_string()).fetch_one(&self.pool).await.map_err(storage)?;
            return Ok(if failed {
                ArticleInput::Failed
            } else {
                ArticleInput::Waiting { title, source_url }
            });
        };
        let pointer: reader_ingest::ContentManifestPointer = decode(&manifest)?;
        let chunks: Vec<String> = decode(&chunks.ok_or(AiError::FullText)?)?;
        if chunks.len() != pointer.safe_html_chunks as usize {
            return Err(AiError::Storage);
        }
        let mut bytes = Vec::new();
        for chunk in chunks {
            bytes.extend(decode::<Vec<u8>>(&chunk)?);
        }
        let html = String::from_utf8(bytes).map_err(storage)?;
        let source_revision = format!(
            "{record}/{}/{}",
            pointer.source_revision, pointer.refresh_id
        );
        // Text-node boundaries are explicit newline separators in the AI input.
        // Preserve the original safe HTML separately; citations target this exact
        // documented text snapshot, not whitespace guessed by the browser.
        let text = reader_ai::article_plain_text(&html);
        if text.trim().is_empty() {
            return Ok(ArticleInput::Failed);
        }
        Ok(ArticleInput::Ready(ArticleSnapshot {
            title,
            source_url,
            safe_html: html,
            text,
            source_revision,
        }))
    }
    async fn create_chat(
        &self,
        record: ChatRecord,
        operation: Uuid,
        regenerate: bool,
    ) -> Result<ChatRecord, AiError> {
        let owner = record.owner;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        operation_lock(&mut tx, owner, operation).await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!(
                "ai-article/{owner}/{}/{}",
                record.view.workspace_id, record.view.article_id
            ))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspaces w JOIN articles a ON a.id=$3 WHERE w.id=$1 AND w.document::jsonb->>'owner'=$2)")
            .bind(record.view.workspace_id.to_string()).bind(owner.to_string()).bind(format!("{}/{}",record.view.workspace_id,record.view.article_id)).fetch_one(&mut *tx).await.map_err(storage)?;
        if !exists {
            return Err(AiError::NotFound);
        }
        let kind = OperationKind::Start { regenerate };
        if let Some((id, old)) = prior_operation(&mut tx, owner, operation).await? {
            let prior = locked(&mut tx, owner, id).await?;
            if old != kind
                || prior.view.article_id != record.view.article_id
                || prior.view.workspace_id != record.view.workspace_id
            {
                return Err(AiError::Conflict);
            }
            return Ok(prior);
        }
        if !regenerate {
            let existing: Option<String> = sqlx::query_scalar("SELECT document FROM ai_chats WHERE owner=$1 AND workspace=$2 AND article=$3 ORDER BY created_at DESC,id DESC LIMIT 1")
                .bind(owner).bind(record.view.workspace_id).bind(record.view.article_id).fetch_optional(&mut *tx).await.map_err(storage)?;
            if let Some(document) = existing {
                let prior: ChatRecord = decode(&document)?;
                save_operation(&mut tx, owner, prior.view.id, operation, &kind).await?;
                tx.commit().await.map_err(storage)?;
                return Ok(prior);
            }
        }
        sqlx::query("INSERT INTO ai_chats(id,owner,workspace,article,created_at,status,document) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(record.view.id).bind(owner).bind(record.view.workspace_id).bind(record.view.article_id).bind(record.view.created_at).bind(status(record.view.status)).bind(encode(&record)?).execute(&mut *tx).await.map_err(storage)?;
        save_operation(&mut tx, owner, record.view.id, operation, &kind).await?;
        tx.commit().await.map_err(storage)?;
        Ok(record)
    }
    async fn chats(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ChatRecord>, AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        // Verify the actual article even for an empty list: guessed IDs cannot be
        // used to create a conversation unrelated to the caller's library.
        self.reader
            .article(
                WorkspaceId::from_uuid(workspace),
                ArticleId::from_uuid(article),
            )
            .await
            .map_err(|_| AiError::NotFound)?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT document FROM ai_chats WHERE owner=$1 AND workspace=$2 AND article=$3 ORDER BY created_at DESC,id DESC").bind(owner).bind(workspace).bind(article).fetch_all(&self.pool).await.map_err(storage)?;
        rows.iter().map(|v| decode(v)).collect()
    }
    async fn chat(&self, owner: Uuid, id: Uuid) -> Result<ChatRecord, AiError> {
        let document: Option<String> = sqlx::query_scalar("SELECT c.document FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.id=$1 AND c.owner=$2 AND w.document::jsonb->>'owner'=$2::text").bind(id).bind(owner).fetch_optional(&self.pool).await.map_err(storage)?;
        let record: ChatRecord = decode(&document.ok_or(AiError::NotFound)?)?;
        if record.owner != owner || record.view.id != id {
            return Err(AiError::Storage);
        }
        Ok(record)
    }
    async fn append(
        &self,
        owner: Uuid,
        id: Uuid,
        operation: Uuid,
        kind: OperationKind,
    ) -> Result<ChatRecord, AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        operation_lock(&mut tx, owner, operation).await?;
        let mut record = locked(&mut tx, owner, id).await?;
        if let Some((prior_id, prior_kind)) = prior_operation(&mut tx, owner, operation).await? {
            if prior_id != id || prior_kind != kind {
                return Err(AiError::Conflict);
            }
            return Ok(record);
        }
        if record.view.status.pending() {
            return Err(AiError::Conflict);
        }
        let now = Utc::now();
        let task = match &kind {
            OperationKind::Retry => record
                .operations
                .last()
                .ok_or(AiError::Storage)?
                .task
                .clone(),
            OperationKind::Message { .. } => {
                if !record.view.messages.iter().any(|m| {
                    m.purpose == Some(MessagePurpose::Summary)
                        && m.status == MessageStatus::Complete
                }) {
                    return Err(AiError::Conflict);
                }
                AttemptTask::Reply
            }
            _ => return Err(AiError::Conflict),
        };
        match &kind {
            OperationKind::Message { content } => record.view.messages.push(ChatMessage {
                id: Uuid::new_v4(),
                role: MessageRole::User,
                content: content.clone(),
                status: MessageStatus::Complete,
                created_at: now,
                phase: None,
                purpose: None,
            }),
            OperationKind::Retry
                if matches!(
                    record.view.status,
                    ChatStatus::Failed | ChatStatus::Interrupted | ChatStatus::Cancelled
                ) => {}
            _ => return Err(AiError::Conflict),
        }
        let assistant_id = Uuid::new_v4();
        record.view.messages.push(ChatMessage {
            id: assistant_id,
            role: MessageRole::Assistant,
            content: String::new(),
            status: MessageStatus::Pending,
            created_at: now,
            phase: Some(task.phase()),
            purpose: Some(task.purpose()),
        });
        record.operations.push(Operation {
            id: operation,
            kind: kind.clone(),
            assistant_id,
            task,
        });
        record.view.status = if record.snapshot.is_some() {
            ChatStatus::Queued
        } else {
            ChatStatus::WaitingContent
        };
        record.view.error = None;
        write_record(&mut tx, &record).await?;
        sqlx::query("UPDATE ai_chats SET lease=NULL,lease_until=NULL WHERE id=$1 AND owner=$2")
            .bind(id)
            .bind(owner)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        save_operation(&mut tx, owner, id, operation, &kind).await?;
        tx.commit().await.map_err(storage)?;
        Ok(record)
    }
    async fn stop(&self, owner: Uuid, id: Uuid) -> Result<ChatRecord, AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut record = locked(&mut tx, owner, id).await?;
        if record.view.status.pending() {
            record.view.status = ChatStatus::Cancelled;
            calls::interrupt(&mut record);
            set_attempt(&mut record, MessageStatus::Interrupted)?;
            write_record(&mut tx, &record).await?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(record)
    }
    async fn claim(&self, lease_seconds: u64) -> Result<Option<ClaimedChat>, AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // A previous process may have submitted a billable request. Expiry is an
        // interruption, not permission to repeat it automatically.
        let expired: Vec<String> = sqlx::query_scalar("SELECT document FROM ai_chats WHERE lease_until<now() AND status IN ('generating','verifying','queued','waiting_content') FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await.map_err(storage)?;
        for document in expired {
            let mut record: ChatRecord = decode(&document)?;
            record.view.status = ChatStatus::Interrupted;
            record.view.error = Some(
                "The worker was interrupted; provider billing may be unknown. Retry explicitly."
                    .into(),
            );
            calls::interrupt(&mut record);
            set_attempt(&mut record, MessageStatus::Interrupted)?;
            write_record(&mut tx, &record).await?;
        }
        let row: Option<String> = sqlx::query_scalar("SELECT c.document FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.status IN ('queued','waiting_content') AND c.lease IS NULL AND w.document::jsonb->>'owner'=c.owner::text ORDER BY CASE c.status WHEN 'queued' THEN 0 ELSE 1 END,c.scheduled_at,c.id LIMIT 1 FOR UPDATE OF c SKIP LOCKED").fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(document) = row else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let record: ChatRecord = decode(&document)?;
        let lease = Uuid::new_v4();
        let queue_wait_us: i64 = sqlx::query_scalar(
            "UPDATE ai_chats SET lease=$2,lease_until=now()+$3*interval '1 second' WHERE id=$1 RETURNING (extract(epoch FROM now()-scheduled_at)*1000000)::bigint",
        )
        .bind(record.view.id)
        .bind(lease)
        .bind(i64::try_from(lease_seconds).map_err(storage)?)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        log::info!(
            "ai_claim class=chat job_id={} queue_wait_us={}",
            record.view.id,
            queue_wait_us
        );
        tx.commit().await.map_err(storage)?;
        Ok(Some(ClaimedChat { record, lease }))
    }
    async fn active(&self, owner: Uuid, id: Uuid, lease: Uuid) -> Result<bool, AiError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text JOIN ai_profiles p ON p.owner=c.owner WHERE c.id=$1 AND c.owner=$2 AND c.lease=$3 AND c.lease_until>now() AND c.status IN ('queued','waiting_content','generating','verifying') AND w.document::jsonb->>'owner'=c.owner::text)").bind(id).bind(owner).bind(lease).fetch_one(&self.pool).await.map_err(storage)
    }
    async fn begin_call(
        &self,
        claim: &ClaimedChat,
        phase: GenerationPhase,
    ) -> Result<Uuid, AiError> {
        calls::begin(&self.pool, claim, phase).await
    }
    async fn update_call(
        &self,
        claim: &ClaimedChat,
        id: Uuid,
        update: CallUpdate,
    ) -> Result<(), AiError> {
        calls::update(&self.pool, claim, id, update).await
    }
    async fn update_claim(
        &self,
        claim: &ClaimedChat,
        next: ChatStatus,
        content: Option<&str>,
        snapshot: Option<&ArticleSnapshot>,
        error: Option<&str>,
    ) -> Result<(), AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut record = locked(&mut tx, claim.record.owner, claim.record.view.id).await?;
        let valid: bool = sqlx::query_scalar("SELECT COALESCE(lease=$2 AND lease_until>now() AND status IN ('queued','waiting_content','generating','verifying'),false) FROM ai_chats WHERE id=$1").bind(record.view.id).bind(claim.lease).fetch_one(&mut *tx).await.map_err(storage)?;
        if !valid {
            return Err(AiError::Cancelled);
        }
        let operation = record.operations.last().ok_or(AiError::Storage)?;
        if content.is_some() && matches!(operation.task, AttemptTask::Summary { draft: None }) {
            return Err(AiError::Conflict);
        }
        if next == ChatStatus::Completed {
            let required_phase = match &operation.task {
                AttemptTask::Summary { draft: Some(_) } => GenerationPhase::Verifying,
                AttemptTask::Summary { draft: None } => return Err(AiError::Conflict),
                AttemptTask::Reply => GenerationPhase::Generating,
            };
            if !record.view.provider_calls.iter().any(|c| {
                c.assistant_id == operation.assistant_id
                    && c.phase == required_phase
                    && c.status == CallStatus::Completed
            }) {
                return Err(AiError::Conflict);
            }
            if required_phase == GenerationPhase::Verifying {
                let current_content = record
                    .view
                    .messages
                    .iter()
                    .find(|m| m.id == operation.assistant_id)
                    .ok_or(AiError::Storage)?
                    .content
                    .as_str();
                validate_summary_title(
                    content.unwrap_or(current_content),
                    &record.snapshot.as_ref().ok_or(AiError::Storage)?.title,
                )?;
            }
        }
        if let Some(snapshot) = snapshot {
            if record.snapshot.is_some() {
                return Err(AiError::Conflict);
            }
            record.view.title = snapshot.title.clone();
            record.view.source_url = snapshot.source_url.clone();
            record.snapshot = Some(snapshot.clone());
        }
        record.view.status = next;
        if !next.pending() {
            calls::interrupt(&mut record);
        }
        record.view.error = error.map(str::to_owned);
        let assistant_id = record
            .operations
            .last()
            .ok_or(AiError::Storage)?
            .assistant_id;
        if let Some(content) = content {
            record
                .view
                .messages
                .iter_mut()
                .find(|m| m.id == assistant_id)
                .ok_or(AiError::Storage)?
                .content = content.to_owned();
        }
        set_attempt(
            &mut record,
            match next {
                ChatStatus::Completed => MessageStatus::Complete,
                ChatStatus::Generating | ChatStatus::Verifying => MessageStatus::Streaming,
                ChatStatus::Interrupted | ChatStatus::Cancelled => MessageStatus::Interrupted,
                ChatStatus::Failed => MessageStatus::Failed,
                _ => MessageStatus::Pending,
            },
        )?;
        write_record(&mut tx, &record).await?;
        if !matches!(next, ChatStatus::Generating | ChatStatus::Verifying) {
            sqlx::query(
                "UPDATE ai_chats SET lease=NULL,lease_until=NULL,scheduled_at=now() WHERE id=$1",
            )
            .bind(record.view.id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
}
