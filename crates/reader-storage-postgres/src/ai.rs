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

mod automatic;
mod budget;
mod calls;
mod chat_document;
pub(crate) use chat_document::backfill_public_views;
mod definitions;
mod quarantine;
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
    inputs TEXT NOT NULL,
    public_view TEXT NOT NULL,
    public_revision BIGINT NOT NULL DEFAULT 0 CHECK(public_revision>=0),
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
    recovery_batch: std::num::NonZeroU32,
}
impl PostgresAiStore {
    pub fn new(
        pool: PgPool,
        reader: Arc<PostgresRepository>,
        recovery_batch: std::num::NonZeroU32,
    ) -> Self {
        Self {
            pool,
            reader,
            recovery_batch,
        }
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
fn checked_public(
    id: Uuid,
    workspace: Uuid,
    article: Uuid,
    raw: &str,
) -> Result<ArticleChat, AiError> {
    let view: ArticleChat = decode(raw)?;
    if view.id != id || view.workspace_id != workspace || view.article_id != article {
        return Err(AiError::Storage);
    }
    Ok(view)
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
    let document: Option<String> = sqlx::query_scalar("SELECT json_build_array(c.document,c.inputs)::text FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.id=$1 AND c.owner=$2 AND w.document::jsonb->>'owner'=$2::text FOR UPDATE OF c")
        .bind(id).bind(owner).fetch_optional(&mut **tx).await.map_err(storage)?;
    let record: ChatRecord = chat_document::decode(&document.ok_or(AiError::NotFound)?)?;
    if record.owner != owner || record.view.id != id {
        return Err(AiError::Storage);
    }
    Ok(record)
}
async fn write_record(
    tx: &mut Transaction<'_, Postgres>,
    record: &ChatRecord,
) -> Result<(), AiError> {
    let (document, inputs) = chat_document::encode(record)?;
    let changed = sqlx::query("UPDATE ai_chats SET document=$3,status=$4,inputs=CASE WHEN inputs::jsonb=$5::jsonb THEN inputs ELSE $5 END,public_view=$6,public_revision=public_revision+1 WHERE id=$1 AND owner=$2 AND (inputs::jsonb=$5::jsonb OR (inputs::jsonb->'snapshot'='null'::jsonb AND inputs::jsonb-'snapshot'=$5::jsonb-'snapshot'))")
        .bind(record.view.id).bind(record.owner).bind(document)
        .bind(status(record.view.status)).bind(inputs).bind(encode(&record.clone().into_public_view()?)?)
        .execute(&mut **tx).await.map_err(storage)?.rows_affected();
    if changed != 1 {
        return Err(AiError::Conflict);
    }

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
fn set_attempt(
    view: &mut ArticleChat,
    operations: &[Operation],
    state: MessageStatus,
) -> Result<(), AiError> {
    let id = operations.last().ok_or(AiError::Storage)?.assistant_id;
    let message = view
        .messages
        .iter_mut()
        .find(|m| m.id == id)
        .ok_or(AiError::Storage)?;
    message.status = state;
    Ok(())
}

#[async_trait]
impl AiStore for PostgresAiStore {
    async fn spending(&self, owner: Uuid, limit: &str) -> Result<AiSpending, AiError> {
        self.spending_view(owner, limit).await
    }
    async fn reserve(
        &self,
        owner: Uuid,
        id: Uuid,
        reservation: &SpendReservation,
    ) -> Result<(), AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        budget::reserve(&mut tx, owner, id, reservation).await?;
        tx.commit().await.map_err(storage)
    }
    async fn settle(&self, owner: Uuid, id: Uuid, amount: &str) -> Result<(), AiError> {
        DecimalRate::parse(amount)?;
        let count=sqlx::query("UPDATE ai_spending SET actual=$3::numeric WHERE id=$1 AND owner=$2 AND (actual IS NULL OR actual=$3::numeric)").bind(id).bind(owner).bind(amount).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if count != 1 {
            return Err(AiError::Conflict);
        }
        Ok(())
    }
    async fn enroll_summaries(&self, owners: &[Uuid]) -> Result<(), AiError> {
        self.enroll_automatic(owners).await
    }
    async fn next_summary(
        &self,
        owners: &[Uuid],
        attempts: u32,
        retry_seconds: u64,
    ) -> Result<Option<AutoSummary>, AiError> {
        self.automatic_next(owners, attempts, retry_seconds).await
    }
    async fn finish_summary(
        &self,
        target: &AutoSummary,
        error: Option<&str>,
    ) -> Result<(), AiError> {
        sqlx::query("UPDATE ai_summary_queue SET dispatched=$4,error=$5 WHERE owner=$1 AND workspace=$2 AND article=$3").bind(target.owner).bind(target.workspace).bind(target.article).bind(error.is_none()).bind(error).execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }
    async fn prioritize(&self, owner: Uuid, chat: Uuid) -> Result<(), AiError> {
        sqlx::query("UPDATE ai_chats SET priority_at=now() WHERE owner=$1 AND id=$2")
            .bind(owner)
            .bind(chat)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn defer_budget(&self, claim: &ClaimedChat) -> Result<(), AiError> {
        self.budget_wait(claim).await
    }

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

    async fn model_preferences(&self, owner: Uuid) -> Result<ModelPreferences, AiError> {
        let raw: Option<String> =
            sqlx::query_scalar("SELECT document FROM ai_model_preferences WHERE owner=$1")
                .bind(owner)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        raw.map(|value| decode(&value))
            .transpose()
            .map(|value| value.unwrap_or_default())
    }
    async fn save_model_preferences(
        &self,
        owner: Uuid,
        models: ModelPreferences,
    ) -> Result<(), AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("INSERT INTO ai_model_preferences(owner,document) VALUES($1,$2) ON CONFLICT(owner) DO UPDATE SET document=EXCLUDED.document").bind(owner).bind(encode(&models)?).execute(&mut *tx).await.map_err(storage)?;
        // A cheaper choice may now fit today's remainder. Preserve the paid
        // draft and attempt identity; re-admission still enforces the budget.
        sqlx::query("UPDATE ai_chats SET scheduled_at=now() WHERE owner=$1 AND status='queued' AND lease IS NULL AND document::jsonb#>>'{view,error}'=$2").bind(owner).bind(AiError::Budget.to_string()).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(())
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
        let documents: Vec<String> = sqlx::query_scalar("SELECT json_build_array(document,inputs)::text FROM ai_chats WHERE owner=$1 AND status IN ('waiting_content','queued','generating','verifying') FOR UPDATE").bind(owner).fetch_all(&mut *tx).await.map_err(storage)?;
        for document in documents {
            let mut record: ChatRecord = chat_document::decode(&document)?;
            record.view.status = ChatStatus::Cancelled;
            record.view.error = Some(AiError::MissingKey.to_string());
            calls::interrupt(&mut record.view);
            set_attempt(
                &mut record.view,
                &record.operations,
                MessageStatus::Interrupted,
            )?;
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
        let content = crate::content_snapshot::read(&self.pool, workspace, article)
            .await
            .map_err(storage)?;
        let Some(content) = content else {
            let failed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM library_origins o JOIN content_refresh_state f ON f.id='failure/'||o.source_record_id WHERE o.workspace_id=$1 AND o.article_id=$2)").bind(workspace.to_string()).bind(article.to_string()).fetch_one(&self.pool).await.map_err(storage)?;
            return Ok(if failed {
                ArticleInput::Failed
            } else {
                ArticleInput::Waiting { title, source_url }
            });
        };
        let source_revision = format!(
            "{}/{}/{}",
            content.record, content.pointer.source_revision, content.pointer.refresh_id
        );
        let html = content.html;
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
            let existing: Option<String> = sqlx::query_scalar("SELECT json_build_array(document,inputs)::text FROM ai_chats WHERE owner=$1 AND workspace=$2 AND article=$3 ORDER BY created_at DESC,id DESC LIMIT 1")
                .bind(owner).bind(record.view.workspace_id).bind(record.view.article_id).fetch_optional(&mut *tx).await.map_err(storage)?;
            if let Some(document) = existing {
                let prior: ChatRecord = chat_document::decode(&document)?;
                save_operation(&mut tx, owner, prior.view.id, operation, &kind).await?;
                tx.commit().await.map_err(storage)?;
                return Ok(prior);
            }
        }
        let (document, inputs) = chat_document::encode(&record)?;
        sqlx::query("INSERT INTO ai_chats(id,owner,workspace,article,created_at,status,document,inputs,public_view) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(record.view.id).bind(owner).bind(record.view.workspace_id).bind(record.view.article_id).bind(record.view.created_at).bind(status(record.view.status)).bind(document).bind(inputs).bind(encode(&record.clone().into_public_view()?)?).execute(&mut *tx).await.map_err(storage)?;
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
        let rows: Vec<String> = sqlx::query_scalar("SELECT json_build_array(document,inputs)::text FROM ai_chats WHERE owner=$1 AND workspace=$2 AND (article=$3 OR article::text IN (SELECT history_article_id FROM article_history_links WHERE workspace_id=$2::text AND article_id=$3::text)) ORDER BY created_at DESC,id DESC").bind(owner).bind(workspace).bind(article).fetch_all(&self.pool).await.map_err(storage)?;
        rows.iter().map(|v| chat_document::decode(v)).collect()
    }
    async fn public_chats(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
    ) -> Result<Vec<ArticleChat>, AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        self.reader
            .article(
                WorkspaceId::from_uuid(workspace),
                ArticleId::from_uuid(article),
            )
            .await
            .map_err(|_| AiError::NotFound)?;
        let rows: Vec<(Uuid,Uuid,Uuid,String,String)> = sqlx::query_as("SELECT id,workspace,article,public_view,status FROM ai_chats WHERE owner=$1 AND workspace=$2 AND (article=$3 OR article::text IN (SELECT history_article_id FROM article_history_links WHERE workspace_id=$2::text AND article_id=$3::text)) ORDER BY created_at DESC,id DESC")
            .bind(owner).bind(workspace).bind(article).fetch_all(&self.pool).await.map_err(storage)?;
        rows.into_iter()
            .map(|(id, workspace, article, raw, state)| {
                if state == "quarantined" {
                    return Err(AiError::Storage);
                }
                checked_public(id, workspace, article, &raw)
            })
            .collect()
    }
    async fn public_chat(
        &self,
        owner: Uuid,
        id: Uuid,
        after: Option<&str>,
    ) -> Result<ChatPoll, AiError> {
        let row: Option<(Uuid,Uuid,String,Option<String>,String)> = sqlx::query_as("SELECT c.workspace,c.article,c.public_revision::text,CASE WHEN c.public_revision::text=$3 THEN NULL ELSE c.public_view END,c.status FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.id=$1 AND c.owner=$2 AND w.document::jsonb->>'owner'=$2::text")
            .bind(id).bind(owner).bind(after).fetch_optional(&self.pool).await.map_err(storage)?;
        let (workspace, article, revision, raw, state) = row.ok_or(AiError::NotFound)?;
        if state == "quarantined" {
            return Err(AiError::Storage);
        }
        Ok(ChatPoll {
            revision,
            chat: raw
                .map(|raw| checked_public(id, workspace, article, &raw))
                .transpose()?,
        })
    }
    async fn chat(&self, owner: Uuid, id: Uuid) -> Result<ChatRecord, AiError> {
        let document: Option<String> = sqlx::query_scalar("SELECT json_build_array(c.document,c.inputs)::text FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.id=$1 AND c.owner=$2 AND w.document::jsonb->>'owner'=$2::text").bind(id).bind(owner).fetch_optional(&self.pool).await.map_err(storage)?;
        let record: ChatRecord = chat_document::decode(&document.ok_or(AiError::NotFound)?)?;
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
            calls::interrupt(&mut record.view);
            set_attempt(
                &mut record.view,
                &record.operations,
                MessageStatus::Interrupted,
            )?;
            write_record(&mut tx, &record).await?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(record)
    }
    async fn claim(&self, lease_seconds: u64) -> Result<Option<ClaimedChat>, AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // A previous process may have submitted a billable request. Expiry is an
        // interruption, not permission to repeat it automatically.
        let expired: Vec<quarantine::ChatRow> = sqlx::query_as("SELECT id,owner,workspace,article,json_build_array(document,inputs)::text FROM ai_chats WHERE lease_until<now() AND status IN ('generating','verifying','queued','waiting_content') ORDER BY lease_until,id LIMIT $1 FOR UPDATE SKIP LOCKED").bind(i64::from(self.recovery_batch.get())).fetch_all(&mut *tx).await.map_err(storage)?;
        for row in expired {
            let id = row.0;
            let recovered = quarantine::checked_chat(row).and_then(|mut record| {
                record.view.status = ChatStatus::Interrupted;
                record.view.error = Some("The worker was interrupted; provider billing may be unknown. Retry explicitly.".into());
                calls::interrupt(&mut record.view);
                set_attempt(&mut record.view, &record.operations, MessageStatus::Interrupted)?;
                // A decodable record can still contain a corrupt retained draft.
                // Quarantine it before a projection failure aborts healthy recovery.
                record.clone().into_public_view()?;
                Ok(record)
            });
            match recovered {
                Ok(record) => write_record(&mut tx, &record).await?,
                Err(_) => quarantine::chat(&mut tx, id).await?,
            }
        }
        let row: Option<quarantine::ChatRow> = sqlx::query_as("SELECT c.id,c.owner,c.workspace,c.article,json_build_array(c.document,c.inputs)::text FROM ai_chats c JOIN workspaces w ON w.id=c.workspace::text WHERE c.status IN ('queued','waiting_content') AND c.lease IS NULL AND c.scheduled_at<=now() AND w.document::jsonb->>'owner'=c.owner::text ORDER BY c.priority_at DESC NULLS LAST,CASE c.status WHEN 'queued' THEN 0 ELSE 1 END,c.scheduled_at,c.id LIMIT 1 FOR UPDATE OF c SKIP LOCKED").fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row) = row else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let id = row.0;
        let record = match quarantine::checked_chat(row) {
            Ok(record) => record,
            Err(_) => {
                quarantine::chat(&mut tx, id).await?;
                tx.commit().await.map_err(storage)?;
                return Ok(None);
            }
        };
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
        reservation: &SpendReservation,
        model: &CallModel,
    ) -> Result<Uuid, AiError> {
        calls::begin(&self.pool, claim, phase, reservation, model).await
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
        let mut state =
            chat_document::locked_progress(&mut tx, claim.record.owner, claim.record.view.id)
                .await?;
        let record = &mut state.record;
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
                let call = record
                    .view
                    .provider_calls
                    .iter()
                    .find(|c| {
                        c.assistant_id == operation.assistant_id
                            && c.phase == GenerationPhase::Verifying
                            && c.status == CallStatus::Completed
                    })
                    .ok_or(AiError::Conflict)?;
                let raw: String = sqlx::query_scalar(
                    "SELECT content FROM ai_call_responses WHERE id=$1 AND owner=$2",
                )
                .bind(call.id)
                .bind(record.owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
                let AttemptTask::Summary { draft: Some(draft) } = &operation.task else {
                    return Err(AiError::Conflict);
                };
                let source = claim.record.snapshot.as_ref().ok_or(AiError::Storage)?;
                let checked = CompletedGeneration::reviewed(
                    raw,
                    call.usage.clone().ok_or(AiError::Storage)?,
                    draft,
                    &source.title,
                    &source.text,
                    claim.record.limits.max_response_bytes,
                )?;
                if checked.content() != content.unwrap_or(current_content) {
                    return Err(AiError::Review);
                }
            }
        }
        if let Some(snapshot) = snapshot {
            if claim.record.snapshot.is_some() {
                return Err(AiError::Conflict);
            }
            record.view.title = snapshot.title.clone();
            record.view.source_url = snapshot.source_url.clone();
            let changed = sqlx::query("UPDATE ai_chats SET inputs=jsonb_set(inputs::jsonb,'{snapshot}',$2::jsonb)::text WHERE id=$1 AND inputs::jsonb->'snapshot'='null'::jsonb")
                .bind(record.view.id).bind(encode(snapshot)?).execute(&mut *tx).await.map_err(storage)?.rows_affected();
            if changed != 1 {
                return Err(AiError::Conflict);
            }
        }
        record.view.status = next;
        if !next.pending() {
            calls::interrupt(&mut record.view);
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
            &mut record.view,
            &record.operations,
            match next {
                ChatStatus::Completed => MessageStatus::Complete,
                ChatStatus::Generating | ChatStatus::Verifying => MessageStatus::Streaming,
                ChatStatus::Interrupted | ChatStatus::Cancelled => MessageStatus::Interrupted,
                ChatStatus::Failed => MessageStatus::Failed,
                _ => MessageStatus::Pending,
            },
        )?;
        let id = record.view.id;
        chat_document::write_progress(&mut tx, &state).await?;
        if !matches!(next, ChatStatus::Generating | ChatStatus::Verifying) {
            sqlx::query(
                "UPDATE ai_chats SET lease=NULL,lease_until=NULL,scheduled_at=now() WHERE id=$1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
}
