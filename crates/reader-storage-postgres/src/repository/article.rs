use super::*;

#[async_trait::async_trait]
impl reader_application::ArticleRepository for PostgresRepository {
    async fn article(&self, w: WorkspaceId, id: ArticleId) -> Result<Article, RepositoryError> {
        self.read("articles", article_key(w, id)).await
    }
    async fn articles_by_workspace(&self, w: WorkspaceId) -> Result<Vec<Article>, RepositoryError> {
        self.list("articles", w.as_uuid().to_string()).await
    }
    async fn article_presentations_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        self.presentations(workspace, true).await
    }
    async fn article_summaries_by_workspace(
        &self,
        workspace: WorkspaceId,
    ) -> Result<Vec<ArticlePresentation>, RepositoryError> {
        self.presentations(workspace, false).await
    }
    async fn article_summary_page(
        &self,
        workspace: WorkspaceId,
        request: ArticlePageRequest,
    ) -> Result<ArticlePage, RepositoryError> {
        let workspace_id = workspace.as_uuid().to_string();
        let subscription_id = request
            .subscription_id
            .map(|value| value.as_uuid().to_string());
        let cursor_time = request.cursor.as_ref().map(|value| {
            value
                .arrived_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        });
        let cursor_id = request
            .cursor
            .as_ref()
            .map(|value| article_key(workspace, value.article_id));
        // Keep the partial-index predicate literal even after PostgreSQL switches
        // a prepared statement to a generic plan. User input never becomes SQL.
        let state_predicate = match request.view.as_str() {
            "feed" => "NOT a.is_read",
            "later" => "a.is_later",
            "subscription" => "true",
            _ => return Err(storage("unsupported article view")),
        };
        let predicate = format!("a.workspace_key=$1 AND ($2::text IS NULL OR EXISTS (SELECT 1 FROM library_origins o WHERE o.workspace_id=$1 AND o.article_id=a.article_key AND o.subscription_id=$2)) AND {state_predicate}");
        let total_sql = format!("SELECT COUNT(*) FROM articles a WHERE {predicate}");
        let total: i64 = sqlx::query_scalar(&total_sql)
            .bind(&workspace_id)
            .bind(&subscription_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        let unread_total: i64 = if request.view == "feed" && subscription_id.is_none() {
            total
        } else {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM articles a WHERE a.workspace_key=$1 AND NOT a.is_read",
            )
            .bind(&workspace_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?
        };
        let comparison = match request.direction {
            ArticlePageDirection::Older => "<",
            ArticlePageDirection::Newer => ">",
        };
        let order = match request.direction {
            ArticlePageDirection::Older => "DESC",
            ArticlePageDirection::Newer => "ASC",
        };
        let page_sql = format!("SELECT a.document FROM articles a WHERE {predicate} AND ($3::text IS NULL OR (a.arrival_order, a.id) {comparison} (reader_arrival_order($3), $4)) ORDER BY (a.arrival_order) {order}, a.id {order} LIMIT $5");
        let mut articles: Vec<Article> = sqlx::query_scalar::<_, String>(&page_sql)
            .bind(&workspace_id)
            .bind(&subscription_id)
            .bind(&cursor_time)
            .bind(&cursor_id)
            .bind(
                i64::try_from(
                    request
                        .limit
                        .checked_add(1)
                        .ok_or_else(|| storage("article page limit overflow"))?,
                )
                .map_err(storage)?,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .into_iter()
            .map(|value| serde_json::from_str(&value).map_err(storage))
            .collect::<Result<_, _>>()?;
        let has_extra = articles.len() > request.limit;
        articles.truncate(request.limit);
        if request.direction == ArticlePageDirection::Newer {
            articles.reverse();
        }
        let articles = self.summary_presentations(workspace, articles).await?;
        Ok(ArticlePage {
            articles,
            total: usize::try_from(total).map_err(storage)?,
            unread_total: usize::try_from(unread_total).map_err(storage)?,
            has_newer: request.cursor.is_some()
                && (request.direction == ArticlePageDirection::Older || has_extra),
            has_older: (request.cursor.is_some()
                && request.direction == ArticlePageDirection::Newer)
                || has_extra,
        })
    }
    async fn article_presentation(
        &self,
        workspace: WorkspaceId,
        id: ArticleId,
    ) -> Result<ArticlePresentation, RepositoryError> {
        let article = self.article(workspace, id).await?;
        self.presentation(workspace, article, true).await
    }
    async fn save_article(
        &self,
        w: WorkspaceId,
        e: Option<u64>,
        v: Article,
    ) -> Result<(), RepositoryError> {
        self.cas(
            "articles",
            article_key(w, v.id),
            w.as_uuid().to_string(),
            e,
            v.revision,
            &v,
        )
        .await
    }
    async fn enqueue_article_full_text_refresh(
        &self,
        workspace: WorkspaceId,
        value: &Article,
    ) -> Result<(), RepositoryError> {
        let value = self.article(workspace, value.id).await?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let mut enqueued = 0;
        for origin in &value.origins {
            let document: String =
                sqlx::query_scalar("SELECT document FROM source_records WHERE id=$1")
                    .bind(origin.as_uuid().to_string())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
                    .ok_or(RepositoryError::NotFound)?;
            let record: SourceRecord = serde_json::from_str(&document).map_err(storage)?;
            let Some(url) = record.key().location.fetch_url().cloned() else {
                continue;
            };
            let item = WorkItem::ExtractFullText {
                record_id: record.id(),
                source_revision: record.revision(),
                url,
                manual: true,
            };
            enqueue_work_tx(&mut tx, Uuid::new_v4(), &item).await?;
            enqueued += 1;
        }
        if enqueued == 0 {
            return Err(storage("article has no fetchable URL origin"));
        }
        tx.commit().await.map_err(storage)
    }
    async fn mark_articles_read_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Article>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        for value in &values {
            let expected = value
                .revision
                .checked_sub(1)
                .ok_or_else(|| storage("updated article revision is invalid"))?;
            let current: Option<i64> =
                sqlx::query_scalar("SELECT revision FROM articles WHERE id=$1 FOR UPDATE")
                    .bind(article_key(workspace, value.id))
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?;
            if current != Some(i64::try_from(expected).map_err(storage)?) {
                return Err(RepositoryError::Conflict);
            }
        }
        for value in values {
            let expected = value.revision - 1;
            if cas_tx(
                &mut tx,
                "articles",
                article_key(workspace, value.id),
                Some(expected),
                value.revision,
                &value,
            )
            .await?
                != 1
            {
                return Err(RepositoryError::Conflict);
            }
        }
        tx.commit().await.map_err(storage)
    }
}
