use super::*;
use reader_application::{ArticleScope, SelectionLimit};

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
        if let Some(period) = request.read_period() {
            return self.read_history_page(workspace, request, period).await;
        }
        let workspace_id = workspace.as_uuid().to_string();
        let subscription_id = request
            .scope()
            .subscription()
            .map(|value| value.as_uuid().to_string());
        let commit_prefixes = request.commit_project().map(|project| project.prefixes());
        let cursor_time = request.cursor().map(|value| {
            value
                .arrived_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        });
        let cursor_id = request
            .cursor()
            .map(|value| article_key(workspace, value.article_id));
        // Keep the partial-index predicate literal even after PostgreSQL switches
        // a prepared statement to a generic plan. User input never becomes SQL.
        let state_predicate = match request.scope() {
            ArticleScope::Feed | ArticleScope::SubscriptionUnread(_) => {
                "NOT a.is_read AND reader_article_visible(a)"
            }
            ArticleScope::Later => "a.is_later AND reader_article_visible(a)",
            ArticleScope::Subscription(_) => "true",
        };
        let predicate = format!("a.workspace_key=$1 AND ($2::text IS NULL OR EXISTS (SELECT 1 FROM library_origins o WHERE o.workspace_id=$1 AND o.article_id=a.article_key AND o.subscription_id=$2)) AND {state_predicate} AND ($3::text[] IS NULL OR EXISTS (SELECT 1 FROM unnest($3::text[]) prefix WHERE starts_with(a.document::jsonb#>>'{{key,location,Url,exact}}',prefix)))");
        let total_sql = format!("SELECT COUNT(*) FROM articles a WHERE {predicate}");
        let total: i64 = sqlx::query_scalar(&total_sql)
            .bind(&workspace_id)
            .bind(&subscription_id)
            .bind(&commit_prefixes)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        let unread_total: i64 = if request.scope() == ArticleScope::Feed
            && subscription_id.is_none()
        {
            total
        } else {
            sqlx::query_scalar(
                    "SELECT COUNT(*) FROM articles a WHERE a.workspace_key=$1 AND NOT a.is_read AND reader_article_visible(a)",
                )
                .bind(&workspace_id)
                .fetch_one(&self.pool)
                .await
                .map_err(storage)?
        };
        let comparison = match request.direction() {
            ArticlePageDirection::Older => "<",
            ArticlePageDirection::Newer => ">",
        };
        let order = match request.direction() {
            ArticlePageDirection::Older => "DESC",
            ArticlePageDirection::Newer => "ASC",
        };
        let page_sql = format!("SELECT a.document FROM articles a WHERE {predicate} AND ($4::text IS NULL OR (a.arrival_order, a.id) {comparison} (reader_arrival_order($4), $5)) ORDER BY (a.arrival_order) {order}, a.id {order} LIMIT $6");
        let mut articles: Vec<Article> = sqlx::query_scalar::<_, String>(&page_sql)
            .bind(&workspace_id)
            .bind(&subscription_id)
            .bind(&commit_prefixes)
            .bind(&cursor_time)
            .bind(&cursor_id)
            .bind(request.limit().lookahead())
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?
            .into_iter()
            .map(|value| serde_json::from_str(&value).map_err(storage))
            .collect::<Result<_, _>>()?;
        let has_extra = articles.len() > request.limit().get();
        articles.truncate(request.limit().get());
        if request.direction() == ArticlePageDirection::Newer {
            articles.reverse();
        }
        let articles = self.summary_presentations(workspace, articles).await?;
        Ok(ArticlePage {
            articles,
            total: usize::try_from(total).map_err(storage)?,
            unread_total: usize::try_from(unread_total).map_err(storage)?,
            has_newer: request.cursor().is_some()
                && (request.direction() == ArticlePageDirection::Older || has_extra),
            has_older: (request.cursor().is_some()
                && request.direction() == ArticlePageDirection::Newer)
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
    async fn reading_activity(
        &self,
        workspace: WorkspaceId,
        timezone: &str,
    ) -> Result<Option<Vec<reader_application::ReadingDay>>, RepositoryError> {
        let valid: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_timezone_names WHERE name=$1)")
                .bind(timezone)
                .fetch_one(&self.pool)
                .await
                .map_err(storage)?;
        if !valid {
            return Ok(None);
        }
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(include_str!("reading_activity.sql"))
            .bind(workspace.as_uuid().to_string())
            .bind(timezone)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        rows.into_iter()
            .map(|(day, count, arrived)| {
                Ok(reader_application::ReadingDay {
                    day,
                    count: u32::try_from(count).map_err(storage)?,
                    arrived: u32::try_from(arrived).map_err(storage)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
    async fn source_activity(
        &self,
        workspace: WorkspaceId,
        timezone: &str,
        period: reader_application::SourceActivityPeriod,
    ) -> Result<Option<Vec<reader_application::SourceActivityDay>>, RepositoryError> {
        let valid: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_timezone_names WHERE name=$1)")
                .bind(timezone)
                .fetch_one(&self.pool)
                .await
                .map_err(storage)?;
        if !valid {
            return Ok(None);
        }
        type Row = (String, i64, Option<String>, Option<String>, bool, i64);
        let rows: Vec<Row> = sqlx::query_as(include_str!("source_activity.sql"))
            .bind(workspace.as_uuid().to_string())
            .bind(timezone)
            .bind(period.start())
            .bind(period.end())
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let mut days =
            std::collections::BTreeMap::<String, reader_application::SourceActivityDay>::new();
        for (day, total, subscription_id, name, present, count) in rows {
            let total = u32::try_from(total).map_err(storage)?;
            let value =
                days.entry(day.clone())
                    .or_insert_with(|| reader_application::SourceActivityDay {
                        day,
                        total,
                        sources: Vec::new(),
                    });
            value.sources.push(reader_application::SourceActivityCount {
                subscription_id,
                name: name
                    .ok_or_else(|| storage("article origin references missing subscription"))?,
                present,
                count: u32::try_from(count).map_err(storage)?,
            });
        }
        Ok(Some(days.into_values().collect()))
    }
    async fn save_article(
        &self,
        w: WorkspaceId,
        e: Option<u64>,
        v: Article,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let previous: Option<bool> =
            sqlx::query_scalar("SELECT is_read FROM articles WHERE id=$1 FOR UPDATE")
                .bind(article_key(w, v.id))
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        if cas_tx(&mut tx, "articles", article_key(w, v.id), e, v.revision, &v).await? != 1 {
            return Err(RepositoryError::Conflict);
        }
        if previous == Some(false) && v.state.read {
            record_read(&mut tx, w, &v, reader_application::ReadMethod::Single).await?;
        }
        tx.commit().await.map_err(storage)
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
    async fn mark_scope_read_atomic(
        &self,
        workspace: WorkspaceId,
        scope: ArticleScope,
        batch: SelectionLimit,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let mut after = String::new();
        loop {
            let rows: Vec<(String,String,i64)> = sqlx::query_as("SELECT a.id,a.document,a.revision FROM articles a WHERE a.workspace_key=$1 AND NOT a.is_read AND ($6 OR reader_article_visible(a)) AND (NOT $2 OR a.is_later) AND ($3::text IS NULL OR EXISTS(SELECT 1 FROM library_origins o WHERE o.workspace_id=$1 AND o.article_id=a.article_key AND o.subscription_id=$3)) AND a.id>$4 ORDER BY a.id LIMIT $5 FOR UPDATE OF a")
                .bind(workspace.as_uuid().to_string()).bind(scope==ArticleScope::Later)
                .bind(scope.subscription().map(|id|id.as_uuid().to_string())).bind(&after)
                .bind(i64::try_from(batch.get()).map_err(storage)?).bind(matches!(scope,ArticleScope::Subscription(_))).fetch_all(&mut *tx).await.map_err(storage)?;
            if rows.is_empty() {
                break;
            }
            after = rows.last().unwrap().0.clone();
            let mut values = Vec::with_capacity(rows.len());
            for (id, document, stored_revision) in rows {
                let article: Article = serde_json::from_str(&document).map_err(storage)?;
                if id != article_key(workspace, article.id)
                    || i64::try_from(article.revision).map_err(storage)? != stored_revision
                    || article.state.read
                {
                    return Err(storage(
                        "article identity/state/revision disagrees with retained document",
                    ));
                }
                let revision = article
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| storage("article revision exhausted"))?;
                values.push((id, i64::try_from(revision).map_err(storage)?));
            }
            // Statement capacity controls chunking, never accepted selection size.
            for chunk in values.chunks(65535 / 2) {
                let mut query=sqlx::QueryBuilder::<Postgres>::new("WITH changed AS (UPDATE articles a SET revision=v.revision,document=jsonb_set(jsonb_set(a.document::jsonb,'{state,read}','true'::jsonb,false),'{revision}',to_jsonb(v.revision),false)::text FROM (");
                query.push_values(chunk, |mut row, (id, revision)| {
                    row.push_bind(id).push_bind(*revision);
                });
                query.push(") v(id,revision) WHERE a.id=v.id AND a.revision=v.revision-1 AND NOT a.is_read RETURNING a.workspace_key,a.article_key,a.revision) INSERT INTO article_read_events(workspace_id,article_id,revision,method) SELECT workspace_key,article_key,revision,'bulk' FROM changed");
                if query
                    .build()
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?
                    .rows_affected()
                    != chunk.len() as u64
                {
                    return Err(RepositoryError::Conflict);
                }
            }
        }
        tx.commit().await.map_err(storage)
    }
}

pub(super) async fn record_read(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    value: &Article,
    method: reader_application::ReadMethod,
) -> Result<(), RepositoryError> {
    sqlx::query(
        "INSERT INTO article_read_events(workspace_id,article_id,revision,method) VALUES($1,$2,$3,$4)",
    )
    .bind(workspace.as_uuid().to_string())
    .bind(value.id.as_uuid().to_string())
    .bind(i64::try_from(value.revision).map_err(storage)?)
    .bind(method.as_str())
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}
