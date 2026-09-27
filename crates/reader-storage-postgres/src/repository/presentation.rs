use super::*;
type OriginPresentationRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);
impl PostgresRepository {
    pub(super) async fn presentation(
        &self,
        workspace: WorkspaceId,
        article: Article,
        include_content: bool,
    ) -> Result<ArticlePresentation, RepositoryError> {
        let rows: Vec<OriginPresentationRow> = sqlx::query_as(
            "SELECT o.source_record_id,s.document,m.document,f.document,r.document::jsonb->>'description_media_type' FROM library_origins o JOIN subscriptions s ON s.id=o.subscription_id LEFT JOIN source_records r ON r.id=o.source_record_id LEFT JOIN content_manifests m ON m.id=o.source_record_id LEFT JOIN content_refresh_state f ON f.id='failure/' || o.source_record_id WHERE o.workspace_id=$1 AND o.article_id=$2",
        ).bind(workspace.as_uuid().to_string()).bind(article.id.as_uuid().to_string()).fetch_all(&self.pool).await.map_err(storage)?;
        let mut subscription_ids = Vec::new();
        let mut subscription_titles = Vec::new();
        let mut manifests = Vec::new();
        let mut failure_reason = None;
        let mut description_media_type = None;
        for (record, subscription, manifest, failure, media_type) in rows {
            merge_media_type(&mut description_media_type, media_type)?;
            let subscription: Subscription =
                serde_json::from_str(&subscription).map_err(storage)?;
            subscription.validate(self.reason_policy).map_err(storage)?;
            if subscription.workspace_id() != workspace {
                return Err(storage("article origin crosses workspace ownership"));
            }
            if !subscription_ids.contains(&subscription.id()) {
                subscription_ids.push(subscription.id());
            }
            if !subscription_titles
                .iter()
                .any(|v| v == subscription.title())
            {
                subscription_titles.push(subscription.title().to_owned());
            }
            if let Some(manifest) = manifest {
                manifests.push((
                    record,
                    serde_json::from_str::<reader_ingest::ContentManifestPointer>(&manifest)
                        .map_err(storage)?,
                ));
            }
            if let Some(failure) = failure {
                failure_reason = serde_json::from_str::<serde_json::Value>(&failure)
                    .ok()
                    .and_then(|v| {
                        v.get("diagnostic")
                            .and_then(|v| v.as_str())
                            .map(str::to_owned)
                    });
            }
        }
        let latest = manifests.into_iter().max_by_key(|(_, v)| v.fetched_at);
        let content = if include_content {
            crate::content_snapshot::read(&self.pool, workspace.as_uuid(), article.id.as_uuid())
                .await
                .map_err(storage)?
        } else {
            None
        };
        let ready = if include_content {
            content.is_some()
        } else {
            latest.is_some()
        };
        let safe_html = content.map(|value| value.html);
        let full_text_status = if ready {
            reader_application::ContentStatus::Ready
        } else if failure_reason.is_some() {
            reader_application::ContentStatus::Failed
        } else {
            reader_application::ContentStatus::Pending
        };
        Ok(ArticlePresentation {
            article,
            subscription_ids,
            subscription_titles,
            safe_html,
            description_media_type,
            full_text_status,
            failure_reason,
        })
    }

    pub(super) async fn presentations(
        &self,
        workspace: WorkspaceId,
        include_content: bool,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        let articles = self.articles_by_workspace(workspace).await?;
        let mut result = Vec::with_capacity(articles.len());
        for article in articles {
            result.push(
                self.presentation(workspace, article, include_content)
                    .await?,
            );
        }
        Ok(result)
    }

    pub(super) async fn summary_presentations(
        &self,
        workspace: WorkspaceId,
        articles: Vec<Article>,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        if articles.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = articles
            .iter()
            .map(|value| value.id.as_uuid().to_string())
            .collect();
        let rows: Vec<PresentationRow> = sqlx::query_as(
            "SELECT o.article_id,o.source_record_id,s.document,m.document,f.document,r.document::jsonb->>'description_media_type' FROM library_origins o JOIN subscriptions s ON s.id=o.subscription_id LEFT JOIN source_records r ON r.id=o.source_record_id LEFT JOIN content_manifests m ON m.id=o.source_record_id LEFT JOIN content_refresh_state f ON f.id='failure/' || o.source_record_id WHERE o.workspace_id=$1 AND o.article_id = ANY($2)",
        )
        .bind(workspace.as_uuid().to_string())
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut origins: HashMap<String, Vec<PresentationOrigin>> = HashMap::new();
        for (article_id, record, subscription, manifest, failure, media_type) in rows {
            let subscription: Subscription =
                serde_json::from_str(&subscription).map_err(storage)?;
            subscription.validate(self.reason_policy).map_err(storage)?;
            if subscription.workspace_id() != workspace {
                return Err(storage("article origin crosses workspace ownership"));
            }
            let manifest = manifest
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(storage)?;
            let failure = failure
                .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
                .and_then(|value| {
                    value
                        .get("diagnostic")
                        .and_then(|value| value.as_str())
                        .map(str::to_owned)
                });
            origins.entry(article_id).or_default().push((
                record,
                subscription,
                manifest,
                failure,
                media_type,
            ));
        }
        articles
            .into_iter()
            .map(|article| {
                let rows = origins
                    .remove(&article.id.as_uuid().to_string())
                    .unwrap_or_default();
                let mut subscription_ids = Vec::new();
                let mut subscription_titles = Vec::new();
                let mut latest = None;
                let mut failure_reason = None;
                let mut description_media_type = None;
                for (record, subscription, manifest, failure, media_type) in rows {
                    merge_media_type(&mut description_media_type, media_type)?;
                    if !subscription_ids.contains(&subscription.id()) {
                        subscription_ids.push(subscription.id());
                    }
                    if !subscription_titles
                        .iter()
                        .any(|value| value == subscription.title())
                    {
                        subscription_titles.push(subscription.title().to_owned());
                    }
                    if let Some(manifest) = manifest {
                        if latest.as_ref().is_none_or(
                            |(_, current): &(String, reader_ingest::ContentManifestPointer)| {
                                current.fetched_at < manifest.fetched_at
                            },
                        ) {
                            latest = Some((record, manifest));
                        }
                    }
                    if failure.is_some() {
                        failure_reason = failure;
                    }
                }
                let full_text_status = if latest.is_some() {
                    reader_application::ContentStatus::Ready
                } else if failure_reason.is_some() {
                    reader_application::ContentStatus::Failed
                } else {
                    reader_application::ContentStatus::Pending
                };
                Ok(ArticlePresentation {
                    article,
                    subscription_ids,
                    subscription_titles,
                    safe_html: None,
                    description_media_type,
                    full_text_status,
                    failure_reason,
                })
            })
            .collect()
    }
}

// Identical raw text can have different meanings under different media types.
// Refuse contradictory known declarations; absence remains explicitly unknown.
fn merge_media_type(
    current: &mut Option<String>,
    next: Option<String>,
) -> Result<(), RepositoryError> {
    if let Some(next) = next {
        if current.as_ref().is_some_and(|value| value != &next) {
            return Err(storage("conflicting article summary media types"));
        }
        *current = Some(next);
    }
    Ok(())
}
