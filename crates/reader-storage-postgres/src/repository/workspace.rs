use super::*;

#[async_trait::async_trait]
impl reader_application::WorkspaceRepository for PostgresRepository {
    async fn workspace(&self, id: WorkspaceId) -> Result<Workspace, RepositoryError> {
        let v: Workspace = self.read("workspaces", id.as_uuid().to_string()).await?;
        v.validate(self.reason_policy).map_err(storage)?;
        Ok(v)
    }
    async fn workspaces_by_owner(&self, o: AccountId) -> Result<Vec<Workspace>, RepositoryError> {
        let values: Vec<Workspace> = self.list("workspaces", o.as_uuid().to_string()).await?;
        for value in &values {
            value.validate(self.reason_policy).map_err(storage)?;
            if value.owner() != o {
                return Err(storage(
                    "workspace owner index returned a foreign workspace",
                ));
            }
        }
        Ok(values)
    }
    async fn save_workspace(&self, e: Option<u64>, v: Workspace) -> Result<(), RepositoryError> {
        v.validate(self.reason_policy).map_err(storage)?;
        self.cas(
            "workspaces",
            v.id().as_uuid().to_string(),
            v.owner().as_uuid().to_string(),
            e,
            v.revision(),
            &v,
        )
        .await
    }
    async fn restore_workspace_with_refreshes(
        &self,
        expected: u64,
        value: Workspace,
        active: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        value.validate(self.reason_policy).map_err(storage)?;
        if !value.accepts_delivery() {
            return Err(storage("workspace catch-up requires an active workspace"));
        }
        for subscription in &active {
            if subscription.workspace_id() != value.id()
                || !matches!(subscription.status(), SubscriptionStatus::Active)
            {
                return Err(storage(
                    "workspace catch-up contains an invalid subscription",
                ));
            }
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if cas_tx(
            &mut tx,
            "workspaces",
            value.id().as_uuid().to_string(),
            Some(expected),
            value.revision(),
            &value,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        for subscription in active {
            let source = source_for_subscription(&mut tx, subscription.id()).await?;
            enqueue_backfill_tx(&mut tx, source, subscription.id(), self.initial_scope).await?;
            let item = WorkItem::RefreshSource { source_id: source };
            let identity = format!(
                "workspace-restore/{}/{}/{}",
                value.id().as_uuid(),
                value.revision(),
                subscription.id().as_uuid()
            );
            enqueue_work_tx(
                &mut tx,
                Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
                &item,
            )
            .await?;
        }
        tx.commit().await.map_err(storage)
    }
}
