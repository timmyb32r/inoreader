use super::*;

#[async_trait::async_trait]
impl reader_application::RuleRepository for PostgresRepository {
    async fn rules_by_workspace(&self, w: WorkspaceId) -> Result<Vec<Rule>, RepositoryError> {
        let values: Vec<Rule> = self.list("rules", w.as_uuid().to_string()).await?;
        for value in &values {
            value.validate().map_err(storage)?;
        }
        Ok(values)
    }
    async fn rule(&self, w: WorkspaceId, id: RuleId) -> Result<Rule, RepositoryError> {
        let value: Rule = self.read("rules", rule_key(w, id)).await?;
        value.validate().map_err(storage)?;
        Ok(value)
    }
    async fn save_rule(
        &self,
        w: WorkspaceId,
        e: Option<u64>,
        v: Rule,
    ) -> Result<(), RepositoryError> {
        v.validate().map_err(storage)?;
        self.cas(
            "rules",
            rule_key(w, v.id),
            w.as_uuid().to_string(),
            e,
            v.version,
            &v,
        )
        .await
    }
    async fn delete_rule(&self, w: WorkspaceId, id: RuleId, e: u64) -> Result<(), RepositoryError> {
        let n = sqlx::query("DELETE FROM rules WHERE id=$1 AND revision=$2")
            .bind(rule_key(w, id))
            .bind(i64::try_from(e).map_err(storage)?)
            .execute(&self.pool)
            .await
            .map_err(storage)?
            .rows_affected();
        if n == 1 {
            Ok(())
        } else {
            Err(RepositoryError::Conflict)
        }
    }
    async fn enqueue_rule_application(
        &self,
        workspace: WorkspaceId,
        rule: Rule,
    ) -> Result<Uuid, RepositoryError> {
        rule.validate().map_err(storage)?;
        let identity = format!(
            "apply-rule/{}/{}/{}",
            workspace.as_uuid(),
            rule.id.as_uuid(),
            rule.version
        );
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes());
        let through_article = self
            .articles_by_workspace(workspace)
            .await?
            .into_iter()
            .map(|v| v.id)
            .max_by_key(|v| v.as_uuid());
        let item = WorkItem::ApplyRule {
            workspace_id: workspace,
            rule_id: rule.id,
            rule_version: rule.version,
            after_article: None,
            through_article,
        };
        let mut tx = self.pool.begin().await.map_err(storage)?;
        enqueue_work_tx(&mut tx, id, &item).await?;
        tx.commit().await.map_err(storage)?;
        Ok(id)
    }
    async fn rule_application_progress(
        &self,
        workspace: WorkspaceId,
        operation: Uuid,
    ) -> Result<RuleApplicationProgress, RepositoryError> {
        let initial: String = sqlx::query_scalar("SELECT item FROM ingest_jobs WHERE id=$1")
            .bind(operation.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
        let WorkItem::ApplyRule {
            workspace_id,
            rule_id,
            rule_version,
            ..
        } = serde_json::from_str(&initial).map_err(storage)?
        else {
            return Err(RepositoryError::NotFound);
        };
        if workspace_id != workspace {
            return Err(RepositoryError::NotFound);
        }
        let cancel_reason = match self.rule(workspace, rule_id).await {
            Err(RepositoryError::NotFound) => Some("rule was deleted".to_owned()),
            Err(error) => return Err(error),
            Ok(rule) if rule.version != rule_version => {
                Some(format!("rule version changed to {}", rule.version))
            }
            Ok(rule) if !rule.enabled => Some("rule was disabled".to_owned()),
            Ok(_) => None,
        };
        let rows: Vec<(String, String, Option<String>)> =
            sqlx::query_as("SELECT status,item,diagnostic FROM ingest_jobs WHERE origin_key=$1")
                .bind(format!("internal:workspace:{}", workspace.as_uuid()))
                .fetch_all(&self.pool)
                .await
                .map_err(storage)?;
        let mut pending = false;
        let mut failure = None;
        for (status, item, diagnostic) in rows {
            let Ok(WorkItem::ApplyRule {
                workspace_id: job_workspace,
                rule_id: job_rule,
                rule_version: job_version,
                ..
            }) = serde_json::from_str::<WorkItem>(&item)
            else {
                continue;
            };
            if job_workspace != workspace || job_rule != rule_id || job_version != rule_version {
                continue;
            };
            match status.as_str() {
                "ready" | "leased" | "retry" => pending = true,
                "failed" => failure = diagnostic.or(Some("rule application failed".to_owned())),
                _ => {}
            }
        }
        let evaluated: i64 = sqlx::query_scalar("SELECT count(*) FROM rule_evaluations WHERE workspace_id=$1 AND rule_id=$2 AND rule_version=$3").bind(workspace.as_uuid().to_string()).bind(rule_id.as_uuid().to_string()).bind(i64::try_from(rule_version).map_err(storage)?).fetch_one(&self.pool).await.map_err(storage)?;
        let (status, cancel_reason) = if let Some(reason) = cancel_reason {
            ("cancelled".to_owned(), Some(reason))
        } else if let Some(reason) = failure {
            ("failed".to_owned(), Some(reason))
        } else if pending {
            ("running".to_owned(), None)
        } else {
            ("completed".to_owned(), None)
        };
        Ok(RuleApplicationProgress {
            operation_id: operation,
            workspace_id: workspace,
            rule_id,
            rule_version,
            status,
            evaluated: usize::try_from(evaluated).map_err(storage)?,
            cancel_reason,
        })
    }
}
