use super::*;

#[async_trait::async_trait]
impl reader_application::OperationsRepository for PostgresRepository {
    async fn readiness(&self) -> Result<(), RepositoryError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
    async fn save_job(&self, e: Option<u64>, v: DurableJob) -> Result<(), RepositoryError> {
        self.cas("jobs", v.id.to_string(), String::new(), e, v.revision, &v)
            .await
    }
}
