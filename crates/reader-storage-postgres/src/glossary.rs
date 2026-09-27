use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reader_glossary::*;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

mod archive;
mod polling;
mod projection;
mod replay;
pub(crate) mod schema;

#[derive(Clone)]
pub struct PostgresGlossaryStore {
    pool: PgPool,
}
impl PostgresGlossaryStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn storage(_: impl std::fmt::Display) -> GlossaryError {
    GlossaryError::Storage
}
fn encode(value: &impl Serialize) -> Result<String, GlossaryError> {
    serde_json::to_string(value).map_err(storage)
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T, GlossaryError> {
    serde_json::from_str(value).map_err(storage)
}
fn term_hash(term: &str) -> Result<String, GlossaryError> {
    let hash =
        murmur3::murmur3_x64_128(&mut std::io::Cursor::new(term.as_bytes()), 0).map_err(storage)?;
    Ok(format!("{hash:032x}"))
}
async fn owned(pool: &PgPool, owner: Uuid, workspace: Uuid) -> Result<(), GlossaryError> {
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
        Err(GlossaryError::NotFound)
    }
}
async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    workspace: Uuid,
    kind: &str,
    key: &str,
    raw: &str,
) -> Result<Uuid, GlossaryError> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO glossary_receipts(id,owner,workspace,kind,source_key,raw) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(id).bind(owner).bind(workspace).bind(kind).bind(key).bind(raw).execute(&mut **tx).await.map_err(storage)?;
    Ok(id)
}

#[async_trait]
impl GlossaryStore for PostgresGlossaryStore {
    async fn status(&self, owner: Uuid, workspace: Uuid) -> Result<ChannelStatus, GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let row = sqlx::query("SELECT c.*, (SELECT count(*) FROM glossary_posts p WHERE p.owner=c.owner AND p.workspace=c.workspace) AS posts, (SELECT count(*) FROM glossary_definitions d WHERE d.owner=c.owner AND d.workspace=c.workspace) AS definitions, (SELECT count(*) FROM glossary_posts p WHERE p.owner=c.owner AND p.workspace=c.workspace AND projection_error IS NOT NULL) + (SELECT count(*) FROM glossary_events e WHERE e.owner=c.owner AND e.workspace=c.workspace AND e.error IS NOT NULL) AS unindexed, (SELECT count(*) FROM glossary_posts p WHERE p.owner=c.owner AND p.workspace=c.workspace AND conflicted) + (SELECT count(*) FROM glossary_events e WHERE e.owner=c.owner AND e.workspace=c.workspace AND e.conflict) AS conflicts, (SELECT count(*) FROM glossary_events e WHERE e.owner=c.owner AND e.workspace=c.workspace AND NOT applied) AS pending FROM glossary_channels c WHERE owner=$1 AND workspace=$2")
            .bind(owner).bind(workspace).fetch_optional(&self.pool).await.map_err(storage)?;
        let mut view = ChannelStatus {
            channel: CHANNEL_USERNAME.into(),
            configured: false,
            bot_username: None,
            index_ready: false,
            revision: 0,
            posts: 0,
            definitions: 0,
            unindexed: 0,
            pending: 0,
            conflicts: 0,
            last_poll: None,
            last_history: None,
            poll_error: None,
            history_error: None,
            history_incomplete: true,
            coverage_note: "Публичная история может быть неполной; удаления не отслеживаются"
                .into(),
            sync_pending: false,
            generation_allowed: false,
        };
        if let Some(row) = row {
            let binding: Option<String> = row.try_get("binding").map_err(storage)?;
            if let Some(binding) = binding {
                view.bot_username = Some(decode::<ChannelBinding>(&binding)?.bot_username().into());
                view.configured = true;
            }
            view.index_ready = row.try_get("import_complete").map_err(storage)?;
            view.revision = row.try_get("revision").map_err(storage)?;
            view.posts = row.try_get("posts").map_err(storage)?;
            view.definitions = row.try_get("definitions").map_err(storage)?;
            view.unindexed = row.try_get("unindexed").map_err(storage)?;
            view.pending = row.try_get("pending").map_err(storage)?;
            view.conflicts = row.try_get("conflicts").map_err(storage)?;
            view.last_poll = row.try_get("last_poll").map_err(storage)?;
            view.last_history = row.try_get("last_history").map_err(storage)?;
            view.poll_error = row.try_get("poll_error").map_err(storage)?;
            view.coverage_note = row.try_get("coverage_note").map_err(storage)?;
            view.history_error = row.try_get("history_error").map_err(storage)?;
            let next: DateTime<Utc> = row.try_get("next_history").map_err(storage)?;
            let lease: Option<Uuid> = row.try_get("history_lease").map_err(storage)?;
            view.sync_pending = next <= Utc::now() || lease.is_some();
        }
        Ok(view)
    }
    async fn configure(
        &self,
        owner: Uuid,
        workspace: Uuid,
        binding: &ChannelBinding,
        encrypted_token: Vec<u8>,
    ) -> Result<(), GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query(
            "INSERT INTO glossary_channels(owner,workspace) VALUES($1,$2) ON CONFLICT DO NOTHING",
        )
        .bind(owner)
        .bind(workspace)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        let row = sqlx::query("SELECT bot_id,poll_until FROM glossary_channels WHERE owner=$1 AND workspace=$2 FOR UPDATE").bind(owner).bind(workspace).fetch_one(&mut *tx).await.map_err(storage)?;
        let previous: Option<i64> = row.try_get("bot_id").map_err(storage)?;
        let until: Option<DateTime<Utc>> = row.try_get("poll_until").map_err(storage)?;
        if until.is_some_and(|until| until > Utc::now()) {
            return Err(GlossaryError::Conflict);
        }
        if previous.is_some_and(|id| id != binding.bot_id()) {
            return Err(GlossaryError::ReceiverConflict);
        }
        let conflict: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM glossary_channels WHERE bot_id=$1 AND (owner<>$2 OR workspace<>$3))").bind(binding.bot_id()).bind(owner).bind(workspace).fetch_one(&mut *tx).await.map_err(storage)?;
        if conflict {
            return Err(GlossaryError::ReceiverConflict);
        }
        sqlx::query("UPDATE glossary_channels SET binding=$3,bot_id=$4,encrypted_token=$5,next_poll=now(),poll_error=NULL,next_history=now() WHERE owner=$1 AND workspace=$2")
            .bind(owner).bind(workspace).bind(encode(binding)?).bind(binding.bot_id()).bind(encrypted_token).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    async fn request_sync(&self, owner: Uuid, workspace: Uuid) -> Result<(), GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let changed = sqlx::query("UPDATE glossary_channels SET next_history=now(),history_error=NULL WHERE owner=$1 AND workspace=$2")
            .bind(owner).bind(workspace).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(GlossaryError::NotFound);
        }
        Ok(())
    }
    async fn lookup(
        &self,
        owner: Uuid,
        workspace: Uuid,
        terms: &[String],
    ) -> Result<Vec<KnownDefinition>, GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let hashes: Vec<_> = terms
            .iter()
            .map(|term| term_hash(term))
            .collect::<Result<_, _>>()?;
        let rows = sqlx::query("SELECT DISTINCT ON(d.term) d.document,d.message_id,p.published_at,p.conflicted FROM glossary_definitions d JOIN glossary_posts p USING(owner,workspace,message_id) WHERE d.owner=$1 AND d.workspace=$2 AND d.term_hash=ANY($4) AND d.term=ANY($3) ORDER BY d.term,d.message_id DESC,d.position DESC")
            .bind(owner).bind(workspace).bind(terms).bind(hashes).fetch_all(&self.pool).await.map_err(storage)?;
        rows.into_iter()
            .map(|row| {
                Ok(KnownDefinition {
                    definition: decode(row.try_get::<&str, _>("document").map_err(storage)?)?,
                    permalink: format!(
                        "https://t.me/{CHANNEL_USERNAME}/{}",
                        row.try_get::<i64, _>("message_id").map_err(storage)?
                    ),
                    published_at: row.try_get("published_at").map_err(storage)?,
                    stale: row.try_get("conflicted").map_err(storage)?,
                })
            })
            .collect()
    }
    async fn claim_poll(&self, lease_seconds: u64) -> Result<Option<PollClaim>, GlossaryError> {
        self.lease_poll(lease_seconds).await
    }
    async fn commit_batch(
        &self,
        claim: &PollClaim,
        batch: &UpdateBatch,
    ) -> Result<(), GlossaryError> {
        self.persist_batch(claim, batch).await
    }
    async fn finish_poll(
        &self,
        claim: &PollClaim,
        error: Option<&str>,
        retry_seconds: u64,
    ) -> Result<(), GlossaryError> {
        self.release_poll(claim, error, retry_seconds).await
    }
    async fn apply_pending(&self, batch_size: u16) -> Result<usize, GlossaryError> {
        self.project_events(batch_size).await
    }
    async fn claim_history(
        &self,
        lease_seconds: u64,
    ) -> Result<Option<HistoryClaim>, GlossaryError> {
        self.lease_history(lease_seconds).await
    }
    async fn commit_page(
        &self,
        claim: &HistoryClaim,
        page: &PublicPage,
        delay: u64,
    ) -> Result<(), GlossaryError> {
        self.persist_page(claim, page, delay).await
    }
    async fn fail_history(
        &self,
        claim: &HistoryClaim,
        error: &str,
        retry: u64,
    ) -> Result<(), GlossaryError> {
        self.release_history(claim, error, retry).await
    }
}
