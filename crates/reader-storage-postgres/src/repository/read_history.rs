use super::*;

impl PostgresRepository {
    /// One result per article, ordered by its latest event within the requested
    /// interval. Only currently read articles are visible; raw events remain intact.
    pub(super) async fn read_history_page(
        &self,
        workspace: WorkspaceId,
        request: ArticlePageRequest,
        period: reader_application::ReadPeriod,
    ) -> Result<ArticlePage, RepositoryError> {
        let workspace_key = workspace.as_uuid().to_string();
        let events = "WITH events AS (SELECT DISTINCT ON(article_id) article_id, occurred_at AS marked_at, method FROM article_read_events WHERE workspace_id=$1 AND occurred_at >= $2 AND occurred_at < $3 ORDER BY article_id,occurred_at DESC,revision DESC)";
        let total: i64 = sqlx::query_scalar(&format!("{events} SELECT count(*) FROM events e JOIN articles a ON a.workspace_key=$1 AND a.article_key=e.article_id AND a.is_read"))
            .bind(&workspace_key).bind(period.start()).bind(period.end())
            .fetch_one(&self.pool).await.map_err(storage)?;
        let unread: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM articles WHERE workspace_key=$1 AND NOT is_read",
        )
        .bind(&workspace_key)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        let (comparison, order) = match request.direction() {
            ArticlePageDirection::Older => ("<", "DESC"),
            ArticlePageDirection::Newer => (">", "ASC"),
        };
        let query = format!("{events} SELECT a.document,e.marked_at,e.method FROM events e JOIN articles a ON a.workspace_key=$1 AND a.article_key=e.article_id AND a.is_read WHERE ($4::timestamptz IS NULL OR (e.marked_at,a.article_key) {comparison} ($4,$5)) ORDER BY e.marked_at {order},a.article_key {order} LIMIT $6");
        let mut rows: Vec<(String, chrono::DateTime<chrono::Utc>, String)> = sqlx::query_as(&query)
            .bind(&workspace_key)
            .bind(period.start())
            .bind(period.end())
            .bind(request.cursor().map(|cursor| cursor.arrived_at))
            .bind(
                request
                    .cursor()
                    .map(|cursor| cursor.article_id.as_uuid().to_string()),
            )
            .bind(request.limit().lookahead())
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let extra = rows.len() > request.limit().get();
        rows.truncate(request.limit().get());
        if request.direction() == ArticlePageDirection::Newer {
            rows.reverse();
        }
        let timestamps: Vec<_> = rows
            .iter()
            .map(|(_, at, method)| (*at, method.clone()))
            .collect();
        let articles = rows
            .into_iter()
            .map(|(doc, _, _)| serde_json::from_str(&doc).map_err(storage))
            .collect::<Result<Vec<Article>, _>>()?;
        let mut articles = self.summary_presentations(workspace, articles).await?;
        for (article, (at, method)) in articles.iter_mut().zip(timestamps) {
            article.read_method =
                Some(serde_json::from_value(serde_json::Value::String(method)).map_err(storage)?);
            article.marked_read_at = Some(at);
        }
        Ok(ArticlePage {
            articles,
            total: usize::try_from(total).map_err(storage)?,
            unread_total: usize::try_from(unread).map_err(storage)?,
            has_newer: request.cursor().is_some()
                && (request.direction() == ArticlePageDirection::Older || extra),
            has_older: (request.cursor().is_some()
                && request.direction() == ArticlePageDirection::Newer)
                || extra,
        })
    }
}
