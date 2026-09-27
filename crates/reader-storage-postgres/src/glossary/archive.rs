use super::*;

impl PostgresGlossaryStore {
    pub async fn begin_archive(
        &self,
        owner: Uuid,
        workspace: Uuid,
        inventory: &str,
    ) -> Result<(), GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let inventory_value: serde_json::Value = decode(inventory)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query(
            "INSERT INTO glossary_channels(owner,workspace) VALUES($1,$2) ON CONFLICT DO NOTHING",
        )
        .bind(owner)
        .bind(workspace)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        sqlx::query("SELECT 1 FROM glossary_channels WHERE owner=$1 AND workspace=$2 FOR UPDATE")
            .bind(owner)
            .bind(workspace)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let previous:Option<String>=sqlx::query_scalar("SELECT raw FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='archive_inventory' ORDER BY received_at,id LIMIT 1")
            .bind(owner).bind(workspace).fetch_optional(&mut *tx).await.map_err(storage)?;
        if previous.as_deref() == Some(inventory) {
            return tx.commit().await.map_err(storage);
        }
        if previous.is_some() {
            receipt(
                &mut tx,
                owner,
                workspace,
                "archive_inventory_conflict",
                "inventory.json",
                inventory,
            )
            .await?;
            tx.commit().await.map_err(storage)?;
            return Err(GlossaryError::Conflict);
        }
        let note=format!("Исходный архив: {} постов; ненаблюдавшиеся ID: {}. Промежуток после {} сверяется по публичной истории; полнота и удаления не гарантируются.",inventory_value["post_count"],inventory_value["missing_id_ranges_within_observed_span"],inventory_value["collected_at"]);
        sqlx::query("UPDATE glossary_channels SET coverage_note=$3 WHERE owner=$1 AND workspace=$2 AND NOT import_complete").bind(owner).bind(workspace).bind(note).execute(&mut *tx).await.map_err(storage)?;
        receipt(
            &mut tx,
            owner,
            workspace,
            "archive_inventory",
            "inventory.json",
            inventory,
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    pub async fn import_post(
        &self,
        owner: Uuid,
        workspace: Uuid,
        post: &ObservedPost,
    ) -> Result<(), GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query(
            "SELECT 1 FROM glossary_channels WHERE owner=$1 AND workspace=$2 FOR UPDATE",
        )
        .bind(owner)
        .bind(workspace)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if row.is_none() {
            return Err(GlossaryError::NotFound);
        }
        let previous: Option<(Uuid,String)> = sqlx::query_as("SELECT id,raw FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='archive_post' AND source_key=$3 ORDER BY received_at,id LIMIT 1")
            .bind(owner).bind(workspace).bind(post.id().to_string()).fetch_optional(&mut *tx).await.map_err(storage)?;
        if let Some((id, raw)) = &previous {
            if raw == post.raw() {
                projection::apply_post(&mut tx, owner, workspace, post, *id, "archive").await?;
                return tx.commit().await.map_err(storage);
            }
        }
        let raw_id = receipt(
            &mut tx,
            owner,
            workspace,
            "archive_post",
            &post.id().to_string(),
            post.raw(),
        )
        .await?;
        if previous.is_some() {
            sqlx::query("UPDATE glossary_channels SET history_error='Conflicting archive record; both observations retained' WHERE owner=$1 AND workspace=$2")
                .bind(owner).bind(workspace).execute(&mut *tx).await.map_err(storage)?;
            tx.commit().await.map_err(storage)?;
            return Err(GlossaryError::Conflict);
        }
        projection::apply_post(&mut tx, owner, workspace, post, raw_id, "archive").await?;
        tx.commit().await.map_err(storage)
    }
    pub async fn import_artifact(
        &self,
        owner: Uuid,
        workspace: Uuid,
        name: &str,
        raw: &str,
    ) -> Result<(), GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let previous: Option<String> = sqlx::query_scalar("SELECT raw FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='archive_file' AND source_key=$3 ORDER BY received_at,id LIMIT 1")
            .bind(owner).bind(workspace).bind(name).fetch_optional(&mut *tx).await.map_err(storage)?;
        if previous.as_deref() == Some(raw) {
            return Ok(());
        }
        receipt(&mut tx, owner, workspace, "archive_file", name, raw).await?;
        tx.commit().await.map_err(storage)?;
        if previous.is_some() {
            Err(GlossaryError::Conflict)
        } else {
            Ok(())
        }
    }
    pub async fn finish_archive(
        &self,
        owner: Uuid,
        workspace: Uuid,
        expected_posts: usize,
    ) -> Result<(), GlossaryError> {
        owned(&self.pool, owner, workspace).await?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM glossary_receipts WHERE owner=$1 AND workspace=$2 AND kind='archive_post'")
            .bind(owner).bind(workspace).fetch_one(&self.pool).await.map_err(storage)?;
        if usize::try_from(count).ok() != Some(expected_posts) {
            return Err(GlossaryError::Conflict);
        }
        sqlx::query("UPDATE glossary_channels SET import_complete=true,next_history=now() WHERE owner=$1 AND workspace=$2")
            .bind(owner).bind(workspace).execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }
}
