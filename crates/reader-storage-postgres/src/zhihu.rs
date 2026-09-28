//! Encrypted account session persistence. SQLx instrumentation logs timings, never
//! bind values. Public source records and articles are not deleted on disconnect.
use async_trait::async_trait;
use reader_ingest::{
    zhihu::{Error, Store},
    BuiltInAdapter, JobId, SourceDefinition, SourceKind, WorkItem,
};
use sqlx::PgPool;
use std::num::NonZeroUsize;
use uuid::Uuid;

pub struct PostgresZhihuStore {
    pool: PgPool,
}
impl PostgresZhihuStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
pub(crate) const SCHEMA:&str="CREATE TABLE zhihu_sessions (owner UUID PRIMARY KEY, encrypted BYTEA NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now())";
fn storage(_: impl std::fmt::Display) -> Error {
    Error::Storage
}
const OWNERS:&str="SELECT DISTINCT w.document::jsonb->>'owner' FROM subscription_sources ss JOIN subscriptions s ON s.id=ss.subscription_id JOIN workspaces w ON w.id=s.document::jsonb->>'workspace_id' WHERE ss.source_id=$1";
#[async_trait]
impl Store for PostgresZhihuStore {
    async fn load(&self, owner: Uuid) -> Result<Option<Vec<u8>>, Error> {
        sqlx::query_scalar("SELECT encrypted FROM zhihu_sessions WHERE owner=$1")
            .bind(owner)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)
    }
    async fn remove(&self, owner: Uuid) -> Result<(), Error> {
        sqlx::query("DELETE FROM zhihu_sessions WHERE owner=$1")
            .bind(owner)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn source_owner(&self, source: &SourceDefinition) -> Result<Uuid, Error> {
        let owners: Vec<String> = sqlx::query_scalar(OWNERS)
            .bind(source.id().as_uuid().to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        if owners.len() != 1 {
            return Err(Error::SharedSource);
        }
        Uuid::parse_str(&owners[0]).map_err(storage)
    }
    async fn save(
        &self,
        owner: Uuid,
        encrypted: Vec<u8>,
        pages: NonZeroUsize,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // Lock source definitions; retaining their IDs preserves every existing
        // article, record, origin and read state when switching collection method.
        let rows:Vec<String>=sqlx::query_scalar("SELECT src.document FROM sources src WHERE src.id IN (SELECT ss.source_id FROM subscription_sources ss JOIN subscriptions s ON s.id=ss.subscription_id JOIN workspaces w ON w.id=s.document::jsonb->>'workspace_id' WHERE w.document::jsonb->>'owner'=$1) AND src.document::jsonb->>'url' LIKE 'https://www.zhihu.com/%' ORDER BY src.id FOR UPDATE")
            .bind(owner.to_string()).fetch_all(&mut *tx).await.map_err(storage)?;
        for raw in rows {
            let source: SourceDefinition = serde_json::from_str(&raw).map_err(storage)?;
            if reader_ingest::zhihu::author(source.url()).is_none() {
                continue;
            }
            let owners: Vec<String> = sqlx::query_scalar(OWNERS)
                .bind(source.id().as_uuid().to_string())
                .fetch_all(&mut *tx)
                .await
                .map_err(storage)?;
            if owners != vec![owner.to_string()] {
                return Err(Error::SharedSource);
            }
            let changed = source
                .revise_kind(SourceKind::BuiltIn(BuiltInAdapter::Zhihu {
                    max_pages: pages,
                }))
                .map_err(storage)?;
            sqlx::query("UPDATE sources SET revision=$2,document=$3 WHERE id=$1")
                .bind(source.id().as_uuid().to_string())
                .bind(i64::try_from(changed.revision()).map_err(storage)?)
                .bind(serde_json::to_string(&changed).map_err(storage)?)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            // RefreshSource dispatches according to the current validated kind.
            crate::repository::enqueue_work_tx(
                &mut tx,
                JobId::new().as_uuid(),
                &WorkItem::RefreshSource {
                    source_id: source.id(),
                },
            )
            .await
            .map_err(storage)?;
        }
        sqlx::query("INSERT INTO zhihu_sessions(owner,encrypted) VALUES($1,$2) ON CONFLICT(owner) DO UPDATE SET encrypted=excluded.encrypted,updated_at=now()")
            .bind(owner).bind(encrypted).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
}
