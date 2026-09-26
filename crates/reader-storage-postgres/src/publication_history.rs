use reader_application::{PublicationDay, PublicationHistory, RepositoryError};
use reader_core::{SubscriptionId, WorkspaceId};
use sqlx::PgPool;

/// Only aggregate delivered origins from the authorized workspace/subscription.
/// The existing by_subscription index bounds the join, and the wire response
/// contains one row per publication day, never article bodies or source records.
pub(crate) async fn load(
    pool: &PgPool,
    workspace: WorkspaceId,
    subscription: SubscriptionId,
) -> Result<PublicationHistory, RepositoryError> {
    let rows: Vec<(Option<String>, i64, i64)> = sqlx::query_as(
        r#"WITH articles AS (
            SELECT origin.article_id,
                   MIN((record.document::jsonb->>'published_at')::timestamptz AT TIME ZONE 'UTC')::date AS day,
                   COUNT(DISTINCT ((record.document::jsonb->>'published_at')::timestamptz AT TIME ZONE 'UTC')::date) AS dates
            FROM library_origins origin
            LEFT JOIN source_records record ON record.id=origin.source_record_id
            WHERE origin.workspace_id=$1 AND origin.subscription_id=$2
            GROUP BY origin.article_id
        )
        SELECT CASE WHEN dates=1 THEN day::text END,
               CASE WHEN dates>1 THEN 2 ELSE dates END AS category,
               COUNT(*)::bigint
        FROM articles
        GROUP BY 1,2 ORDER BY 1"#,
    )
    .bind(workspace.as_uuid().to_string())
    .bind(subscription.as_uuid().to_string())
    .fetch_all(pool)
    .await
    .map_err(|error| RepositoryError::Storage(error.to_string()))?;
    let mut history = PublicationHistory::default();
    for (day, category, count) in rows {
        let count = u64::try_from(count)
            .map_err(|_| RepositoryError::Storage("invalid publication count".into()))?;
        match (day, category) {
            (Some(day), 1) => history.days.push(PublicationDay {
                date: day
                    .parse()
                    .map_err(|_| RepositoryError::Storage("invalid publication date".into()))?,
                count,
            }),
            (None, 0) => history.undated = count,
            (None, 2) => history.conflicting = count,
            _ => {
                return Err(RepositoryError::Storage(
                    "invalid publication aggregate".into(),
                ))
            }
        }
    }
    Ok(history)
}
