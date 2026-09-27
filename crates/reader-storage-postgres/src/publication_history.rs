use futures_util::TryStreamExt;
use reader_application::{PublicationDay, PublicationHistory, RepositoryError};
use reader_core::{PublicationEvidence, SubscriptionId, WorkspaceId};
use sqlx::PgPool;
use std::collections::BTreeMap;

/// Stream date metadata, grouped by article, from the authorized subscription.
/// Memory holds calendar-day totals and one article's origins, never bodies.
pub(crate) async fn load(
    pool: &PgPool,
    workspace: WorkspaceId,
    subscription: SubscriptionId,
) -> Result<PublicationHistory, RepositoryError> {
    let mut rows = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT o.article_id,r.document::jsonb->>'published_at',m.document FROM library_origins o LEFT JOIN source_records r ON r.id=o.source_record_id LEFT JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=$1 AND o.subscription_id=$2 ORDER BY o.article_id,o.source_record_id")
        .bind(workspace.as_uuid().to_string()).bind(subscription.as_uuid().to_string()).fetch(pool);
    let mut current = None;
    let mut evidence = Vec::new();
    let mut days = BTreeMap::new();
    let mut history = PublicationHistory::default();
    while let Some((id, published, manifest)) = rows.try_next().await.map_err(storage)? {
        if current.as_ref().is_some_and(|value| value != &id) {
            accumulate(&evidence, &mut days, &mut history);
            evidence.clear();
        }
        current = Some(id);
        let manifest = manifest
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(storage)?;
        evidence.extend(crate::publication_dates::origin_publication(
            published,
            manifest.as_ref(),
        )?);
    }
    if current.is_some() {
        accumulate(&evidence, &mut days, &mut history);
    }
    history.days = days
        .into_iter()
        .map(|(date, count)| PublicationDay { date, count })
        .collect();
    Ok(history)
}
fn accumulate(
    evidence: &[PublicationEvidence],
    days: &mut BTreeMap<chrono::NaiveDate, u64>,
    history: &mut PublicationHistory,
) {
    let candidates: std::collections::BTreeSet<_> = evidence
        .iter()
        .filter_map(|v| v.date())
        .map(|v| v.day())
        .collect();
    if candidates.len() == 1 {
        *days.entry(*candidates.first().unwrap()).or_default() += 1;
    } else if candidates.len() > 1 {
        history.conflicting += 1;
    } else {
        history.undated += 1;
    }
}
fn storage(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Storage(error.to_string())
}
