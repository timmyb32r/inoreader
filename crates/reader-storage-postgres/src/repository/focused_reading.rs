use super::*;
use reader_application::{
    ArticleRating, CompleteReading, FocusedReadingRepository, ReadingCompletion, ReadingRevision,
    ReadingState,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    command: CompleteReading,
    previous: ReadingState,
    previous_protect_unread: bool,
    completed: ReadingCompletion,
    undone: Option<ReadingCompletion>,
}

#[async_trait::async_trait]
impl FocusedReadingRepository for PostgresRepository {
    async fn reading_state(
        &self,
        owner: AccountId,
        workspace: WorkspaceId,
        article: ArticleId,
    ) -> Result<ReadingState, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let current = locked_article(&mut tx, owner, workspace, article).await?;
        let state = snapshot(&mut tx, &article_key(workspace, article), &current).await?;
        tx.commit().await.map_err(storage)?;
        Ok(state)
    }
    async fn complete_reading(
        &self,
        owner: AccountId,
        workspace: WorkspaceId,
        article: ArticleId,
        command: CompleteReading,
    ) -> Result<ReadingCompletion, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut current = locked_article(&mut tx, owner, workspace, article).await?;
        let key = article_key(workspace, article);
        if let Some(receipt) = receipt(&mut tx, owner, command.operation_id, &key).await? {
            if receipt.command != command {
                return Err(RepositoryError::Conflict);
            }
            return Ok(receipt.undone.unwrap_or(receipt.completed));
        }
        if current.revision != command.expected_revision.get() || current.state.read {
            return Err(RepositoryError::Conflict);
        }
        let previous = snapshot(&mut tx, &key, &current).await?;
        let previous_protect_unread = current.state.protect_unread;
        current.revision = next_revision(current.revision)?;
        current.state.read = true;
        let now: DateTime<Utc> = sqlx::query_scalar("SELECT statement_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let state = ReadingState {
            revision: ReadingRevision::new(current.revision).map_err(storage)?,
            read: true,
            rating: Some(command.rating),
            rated_at: Some(now),
        };
        let completed = ReadingCompletion {
            operation_id: command.operation_id,
            article_id: article.as_uuid(),
            state,
            undone: false,
        };
        let receipt = Receipt {
            command,
            previous,
            previous_protect_unread,
            completed: completed.clone(),
            undone: None,
        };
        // Conflicting operation IDs across different articles roll back every write.
        let inserted = sqlx::query("INSERT INTO reading_completions(owner,id,article_key,document) VALUES($1,$2,$3,$4) ON CONFLICT(owner,id) DO NOTHING")
            .bind(owner.as_uuid().to_string()).bind(completed.operation_id).bind(&key).bind(serde_json::to_string(&receipt).map_err(storage)?)
            .execute(&mut *tx).await.map_err(storage)?.rows_affected();
        if inserted != 1 {
            return Err(RepositoryError::Conflict);
        }
        write_article(&mut tx, &key, &current).await?;
        write_rating(&mut tx, &key, &completed.state).await?;
        super::article::record_read(&mut tx, workspace, &current).await?;
        tx.commit().await.map_err(storage)?;
        Ok(completed)
    }
    async fn undo_reading(
        &self,
        owner: AccountId,
        workspace: WorkspaceId,
        article: ArticleId,
        operation: Uuid,
    ) -> Result<ReadingCompletion, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut current = locked_article(&mut tx, owner, workspace, article).await?;
        let key = article_key(workspace, article);
        let mut receipt = receipt(&mut tx, owner, operation, &key)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        if let Some(result) = receipt.undone {
            return Ok(result);
        }
        if current.revision != receipt.completed.state.revision.get() {
            return Err(RepositoryError::Conflict);
        }
        current.revision = next_revision(current.revision)?;
        current.state.read = receipt.previous.read;
        current.state.protect_unread = receipt.previous_protect_unread;
        let mut state = receipt.previous.clone();
        state.revision = ReadingRevision::new(current.revision).map_err(storage)?;
        let result = ReadingCompletion {
            operation_id: operation,
            article_id: article.as_uuid(),
            state,
            undone: true,
        };
        write_article(&mut tx, &key, &current).await?;
        write_rating(&mut tx, &key, &result.state).await?;
        receipt.undone = Some(result.clone());
        sqlx::query("UPDATE reading_completions SET document=$3 WHERE owner=$1 AND id=$2")
            .bind(owner.as_uuid().to_string())
            .bind(operation)
            .bind(serde_json::to_string(&receipt).map_err(storage)?)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
fn next_revision(current: u64) -> Result<u64, RepositoryError> {
    let next = current
        .checked_add(1)
        .ok_or_else(|| storage("article revision exhausted"))?;
    ReadingRevision::new(next).map_err(storage)?;
    Ok(next)
}
async fn locked_article(
    tx: &mut Transaction<'_, Postgres>,
    owner: AccountId,
    workspace: WorkspaceId,
    article: ArticleId,
) -> Result<Article, RepositoryError> {
    let row: Option<(i64,String)> = sqlx::query_as("SELECT a.revision,a.document FROM articles a JOIN workspaces w ON w.id=a.workspace_key WHERE a.id=$1 AND w.document::jsonb->>'owner'=$2 FOR UPDATE OF a")
        .bind(article_key(workspace,article)).bind(owner.as_uuid().to_string()).fetch_optional(&mut **tx).await.map_err(storage)?;
    let (revision, document) = row.ok_or(RepositoryError::NotFound)?;
    let current: Article = serde_json::from_str(&document).map_err(storage)?;
    if i64::try_from(current.revision).map_err(storage)? != revision || current.id != article {
        return Err(storage("article revision/identity mismatch"));
    }
    Ok(current)
}
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
    article: &Article,
) -> Result<ReadingState, RepositoryError> {
    let value: Option<(i16, DateTime<Utc>)> =
        sqlx::query_as("SELECT rating,rated_at FROM article_ratings WHERE article_key=$1")
            .bind(key)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?;
    let (rating, rated_at) = match value {
        Some((value, at)) => (
            Some(ArticleRating::try_from(u8::try_from(value).map_err(storage)?).map_err(storage)?),
            Some(at),
        ),
        None => (None, None),
    };
    Ok(ReadingState {
        revision: ReadingRevision::new(article.revision).map_err(storage)?,
        read: article.state.read,
        rating,
        rated_at,
    })
}
async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    owner: AccountId,
    operation: Uuid,
    key: &str,
) -> Result<Option<Receipt>, RepositoryError> {
    let value: Option<(String, String)> = sqlx::query_as(
        "SELECT article_key,document FROM reading_completions WHERE owner=$1 AND id=$2",
    )
    .bind(owner.as_uuid().to_string())
    .bind(operation)
    .fetch_optional(&mut **tx)
    .await
    .map_err(storage)?;
    value
        .map(|(stored, document)| {
            if stored != key {
                return Err(RepositoryError::Conflict);
            }
            serde_json::from_str(&document).map_err(storage)
        })
        .transpose()
}
async fn write_article(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
    article: &Article,
) -> Result<(), RepositoryError> {
    sqlx::query("UPDATE articles SET revision=$2,document=$3 WHERE id=$1")
        .bind(key)
        .bind(i64::try_from(article.revision).map_err(storage)?)
        .bind(serde_json::to_string(article).map_err(storage)?)
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    Ok(())
}
async fn write_rating(
    tx: &mut Transaction<'_, Postgres>,
    key: &str,
    state: &ReadingState,
) -> Result<(), RepositoryError> {
    match (state.rating, state.rated_at) {
        (Some(rating), Some(at)) => {
            sqlx::query("INSERT INTO article_ratings(article_key,rating,rated_at) VALUES($1,$2,$3) ON CONFLICT(article_key) DO UPDATE SET rating=EXCLUDED.rating,rated_at=EXCLUDED.rated_at")
                .bind(key).bind(i16::from(u8::from(rating))).bind(at).execute(&mut **tx).await.map_err(storage)?;
        }
        (None, None) => {
            sqlx::query("DELETE FROM article_ratings WHERE article_key=$1")
                .bind(key)
                .execute(&mut **tx)
                .await
                .map_err(storage)?;
        }
        _ => return Err(storage("inconsistent stored rating")),
    }
    Ok(())
}
