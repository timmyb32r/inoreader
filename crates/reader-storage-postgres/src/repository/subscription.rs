use super::*;

#[async_trait::async_trait]
impl reader_application::SubscriptionRepository for PostgresRepository {
    async fn subscription(&self, id: SubscriptionId) -> Result<Subscription, RepositoryError> {
        let v: Subscription = self.read("subscriptions", id.as_uuid().to_string()).await?;
        v.validate(self.reason_policy).map_err(storage)?;
        Ok(v)
    }
    async fn subscriptions_by_workspace(
        &self,
        w: WorkspaceId,
    ) -> Result<Vec<Subscription>, RepositoryError> {
        let values: Vec<Subscription> = self.list("subscriptions", w.as_uuid().to_string()).await?;
        for value in &values {
            value.validate(self.reason_policy).map_err(storage)?;
            if value.workspace_id() != w {
                return Err(storage("subscription index returned a foreign workspace"));
            }
        }
        Ok(values)
    }
    async fn subscription_stats(
        &self,
        workspace: WorkspaceId,
        s: &[Subscription],
    ) -> Result<HashMap<SubscriptionId, SubscriptionStats>, RepositoryError> {
        if s.iter().any(|value| value.workspace_id() != workspace) {
            return Err(RepositoryError::NotFound);
        }
        let mut result = s
            .iter()
            .map(|v| (v.id(), SubscriptionStats::default()))
            .collect::<HashMap<_, _>>();
        if s.is_empty() {
            return Ok(result);
        }
        let wanted_ids = s
            .iter()
            .map(|v| v.id().as_uuid().to_string())
            .collect::<Vec<_>>();
        type StatsRow = (
            String,
            Option<String>,
            Option<String>,
            bool,
            Option<String>,
            i64,
            i64,
        );
        let rows: Vec<StatsRow> = sqlx::query_as(
            r#"SELECT requested.subscription_id,
                      health.document,
                      source.document,
                      recipe.id IS NOT NULL,
                      icon.data_url,
                      COUNT(DISTINCT origin.article_id)::bigint,
                      COUNT(DISTINCT origin.article_id) FILTER (
                          WHERE NOT COALESCE(article.is_read, false)
                      )::bigint
               FROM unnest($1::text[]) AS requested(subscription_id)
               LEFT JOIN subscription_sources mapping ON mapping.subscription_id=requested.subscription_id
               LEFT JOIN sources source ON source.id=mapping.source_id
               LEFT JOIN source_health health ON health.source_id=mapping.source_id
               LEFT JOIN web_feed_recipes recipe ON recipe.id=requested.subscription_id
               LEFT JOIN subscription_icons icon ON icon.subscription_id=requested.subscription_id
               LEFT JOIN library_origins origin
                 ON origin.workspace_id=$2 AND origin.subscription_id=requested.subscription_id
               LEFT JOIN articles article ON article.id=$2 || '/' || origin.article_id
               GROUP BY requested.subscription_id,health.document,source.document,recipe.id,icon.data_url"#,
        )
        .bind(&wanted_ids)
        .bind(workspace.as_uuid().to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        for (id, health, definition, editable_web_feed, icon_data_url, count, unread_count) in rows
        {
            let health = health
                .as_deref()
                .map(serde_json::from_str::<SourceHealth>)
                .transpose()
                .map_err(storage)?;
            let source_type = definition
                .as_deref()
                .map(serde_json::from_str::<SourceDefinition>)
                .transpose()
                .map_err(storage)?
                .map(|v| match v.kind() {
                    SourceKind::WebPage(_) => reader_application::SourceType::Web,
                    SourceKind::BuiltIn(_) => reader_application::SourceType::BuiltIn,
                    _ => reader_application::SourceType::Feed,
                })
                .unwrap_or(reader_application::SourceType::Feed);
            let subscription_id = SubscriptionId::from_uuid(Uuid::parse_str(&id).map_err(storage)?);
            result.insert(
                subscription_id,
                SubscriptionStats {
                    article_count: usize::try_from(count).map_err(storage)?,
                    unread_count: usize::try_from(unread_count).map_err(storage)?,
                    last_success_at: health
                        .as_ref()
                        .and_then(|v| v.last_success_ms)
                        .and_then(DateTime::from_timestamp_millis),
                    last_error_at: health
                        .as_ref()
                        .and_then(|v| v.last_error_ms)
                        .and_then(DateTime::from_timestamp_millis),
                    consecutive_failures: health.as_ref().map_or(0, |v| v.consecutive_failures),
                    incomplete: health.as_ref().is_some_and(|v| v.incomplete),
                    continuation: health
                        .as_ref()
                        .is_some_and(|v| v.incomplete)
                        .then(|| "A durable continuation is queued".to_owned()),
                    error: health.and_then(|v| v.error),
                    editable_web_feed,
                    icon_data_url,
                    source_type,
                },
            );
        }
        Ok(result)
    }
    async fn publication_history(
        &self,
        owner: AccountId,
        id: SubscriptionId,
    ) -> Result<reader_application::PublicationHistory, RepositoryError> {
        let subscription = self.subscription(id).await?;
        if self.workspace(subscription.workspace_id()).await?.owner() != owner {
            return Err(RepositoryError::NotFound);
        }
        crate::publication_history::load(&self.pool, subscription.workspace_id(), id).await
    }
    async fn subscription_activity(
        &self,
        owner: AccountId,
        id: SubscriptionId,
        since: DateTime<Utc>,
    ) -> Result<Vec<SubscriptionActivity>, RepositoryError> {
        let s = self.subscription(id).await?;
        if self.workspace(s.workspace_id()).await?.owner() != owner {
            return Err(RepositoryError::NotFound);
        };
        sqlx::query_scalar::<_,String>("SELECT document FROM subscription_activity WHERE subscription_id=$1 AND occurred_at_ms >= $2 ORDER BY occurred_at_ms DESC").bind(id.as_uuid().to_string()).bind(since.timestamp_millis()).fetch_all(&self.pool).await.map_err(storage)?.into_iter().map(|v|serde_json::from_str(&v).map_err(storage)).collect()
    }
    async fn save_subscription(
        &self,
        e: Option<u64>,
        v: Subscription,
    ) -> Result<(), RepositoryError> {
        v.validate(self.reason_policy).map_err(storage)?;
        if e.is_none() {
            let source =
                SourceDefinition::new(SourceId::new(), v.source_url().clone(), SourceKind::Auto)
                    .map_err(storage)?;
            let item = WorkItem::PollSource {
                source_id: source.id(),
            };
            let mut tx = self.pool.begin().await.map_err(storage)?;
            provision_subscription_tx(
                &mut tx,
                &v,
                &source,
                item,
                JobId::new().as_uuid(),
                self.initial_scope,
                true,
                false,
                None,
            )
            .await?;
            return tx.commit().await.map_err(storage);
        }
        self.cas(
            "subscriptions",
            v.id().as_uuid().to_string(),
            v.workspace_id().as_uuid().to_string(),
            e,
            v.revision(),
            &v,
        )
        .await
    }
    async fn replace_subscription_source(
        &self,
        e: u64,
        v: Subscription,
    ) -> Result<(), RepositoryError> {
        v.validate(self.reason_policy).map_err(storage)?;
        let current = self.subscription(v.id()).await?;
        if current.revision() != e {
            return Err(RepositoryError::Conflict);
        }
        if current.workspace_id() != v.workspace_id() {
            return Err(storage("subscription source replacement changes workspace"));
        }
        let proposed =
            SourceDefinition::new(SourceId::new(), v.source_url().clone(), SourceKind::Auto)
                .map_err(storage)?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let old_key = format!(
            "{}\0{}",
            v.workspace_id().as_uuid(),
            current.source_url_exact()
        );
        let new_key = workspace_feed_url_key(v.workspace_id(), v.source_url_exact());
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT subscription_id FROM workspace_feed_urls WHERE id = $1 FOR UPDATE",
        )
        .bind(&new_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if existing
            .as_deref()
            .is_some_and(|owner| owner != v.id().as_uuid().to_string())
        {
            return Err(RepositoryError::Conflict);
        }
        let inserted = sqlx::query(
            "INSERT INTO source_urls(url,source_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
        )
        .bind(v.source_url().as_str())
        .bind(proposed.id().as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected()
            == 1;
        let source = if inserted {
            cas_tx(
                &mut tx,
                "sources",
                proposed.id().as_uuid().to_string(),
                None,
                0,
                &proposed,
            )
            .await?;
            proposed.id()
        } else {
            let id: String = sqlx::query_scalar("SELECT source_id FROM source_urls WHERE url=$1")
                .bind(v.source_url().as_str())
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
            SourceId::from_uuid(Uuid::parse_str(&id).map_err(storage)?)
        };
        if cas_tx(
            &mut tx,
            "subscriptions",
            v.id().as_uuid().to_string(),
            Some(e),
            v.revision(),
            &v,
        )
        .await?
            != 1
        {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("DELETE FROM workspace_feed_urls WHERE id=$1")
            .bind(old_key)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("INSERT INTO workspace_feed_urls(id,subscription_id) VALUES($1,$2) ON CONFLICT(id) DO UPDATE SET subscription_id=EXCLUDED.subscription_id").bind(new_key).bind(v.id().as_uuid().to_string()).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE subscription_sources SET source_id=$1 WHERE subscription_id=$2")
            .bind(source.as_uuid().to_string())
            .bind(v.id().as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        enqueue_work_tx(
            &mut tx,
            JobId::new().as_uuid(),
            &WorkItem::PollSource { source_id: source },
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn source_url_preview(
        &self,
        id: Uuid,
    ) -> Result<SourceUrlPreviewRecord, RepositoryError> {
        self.read("source_url_previews", id.to_string()).await
    }
    async fn save_source_url_preview(
        &self,
        v: SourceUrlPreviewRecord,
    ) -> Result<(), RepositoryError> {
        self.cas(
            "source_url_previews",
            v.id.to_string(),
            v.subscription_id.as_uuid().to_string(),
            None,
            v.revision,
            &v,
        )
        .await
    }
    async fn activate_subscription_with_refresh(
        &self,
        expected: u64,
        value: Subscription,
    ) -> Result<(), RepositoryError> {
        value.validate(self.reason_policy).map_err(storage)?;
        if !matches!(value.status(), SubscriptionStatus::Active) {
            return Err(storage(
                "catch-up activation requires an active subscription",
            ));
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        if cas_tx(
            &mut tx,
            "subscriptions",
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
        let source = source_for_subscription(&mut tx, value.id()).await?;
        enqueue_backfill_tx(&mut tx, source, value.id(), self.initial_scope).await?;
        let identity = format!(
            "subscription-activation/{}/{}",
            value.id().as_uuid(),
            value.revision()
        );
        enqueue_work_tx(
            &mut tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &WorkItem::RefreshSource { source_id: source },
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn enqueue_subscription_refresh(
        &self,
        value: &Subscription,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let source = source_for_subscription(&mut tx, value.id()).await?;
        let identity = format!("manual-poll/{}/{}", value.id().as_uuid(), value.revision());
        enqueue_work_tx(
            &mut tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &WorkItem::RefreshSource { source_id: source },
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn save_web_feed_subscription(
        &self,
        value: Subscription,
        recipe_json: String,
    ) -> Result<(), RepositoryError> {
        value.validate(self.reason_policy).map_err(storage)?;
        let (raw, recipe) = stored_web_recipe(&recipe_json)?;
        if raw.workspace_id != value.workspace_id().as_uuid()
            || raw.url != value.source_url().as_str()
        {
            return Err(storage(
                "web feed recipe scope or URL does not match subscription",
            ));
        }
        let source = SourceDefinition::new(
            SourceId::new(),
            value.source_url().clone(),
            SourceKind::WebPage(recipe),
        )
        .map_err(storage)?;
        let item = WorkItem::CollectWebFeed {
            source_id: source.id(),
        };
        let mut tx = self.pool.begin().await.map_err(storage)?;
        provision_subscription_tx(
            &mut tx,
            &value,
            &source,
            item,
            JobId::new().as_uuid(),
            self.initial_scope,
            false,
            true,
            Some(&recipe_json),
        )
        .await?;
        tx.commit().await.map_err(storage)
    }
    async fn web_feed_recipe(&self, id: SubscriptionId) -> Result<(u64, String), RepositoryError> {
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT revision,document FROM web_feed_recipes WHERE id=$1")
                .bind(id.as_uuid().to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        let (r, d) = row.ok_or(RepositoryError::NotFound)?;
        Ok((u64::try_from(r).map_err(storage)?, d))
    }
    async fn update_web_feed_recipe(
        &self,
        subscription: SubscriptionId,
        expected: u64,
        recipe_json: String,
    ) -> Result<u64, RepositoryError> {
        let value = self.subscription(subscription).await?;
        let (raw, recipe) = stored_web_recipe(&recipe_json)?;
        if raw.workspace_id != value.workspace_id().as_uuid()
            || raw.url != value.source_url().as_str()
        {
            return Err(storage(
                "web feed recipe scope or URL does not match subscription",
            ));
        }
        let next = expected
            .checked_add(1)
            .ok_or_else(|| storage("web feed recipe version overflow"))?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let source_id = source_for_subscription(&mut tx, subscription).await?;
        let document: String = sqlx::query_scalar("SELECT document FROM sources WHERE id=$1")
            .bind(source_id.as_uuid().to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .ok_or(RepositoryError::NotFound)?;
        let source: SourceDefinition = serde_json::from_str(&document).map_err(storage)?;
        let source = source
            .revise_kind(SourceKind::WebPage(recipe))
            .map_err(storage)?;
        let changed = sqlx::query(
            "UPDATE web_feed_recipes SET revision=$1,document=$2 WHERE id=$3 AND revision=$4",
        )
        .bind(i64::try_from(next).map_err(storage)?)
        .bind(&recipe_json)
        .bind(subscription.as_uuid().to_string())
        .bind(i64::try_from(expected).map_err(storage)?)
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected();
        if changed != 1 {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("UPDATE sources SET revision=$1,document=$2 WHERE id=$3")
            .bind(i64::try_from(next).map_err(storage)?)
            .bind(serde_json::to_string(&source).map_err(storage)?)
            .bind(source_id.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let identity = format!("web-recipe/{}/{next}", subscription.as_uuid());
        enqueue_work_tx(
            &mut tx,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, identity.as_bytes()),
            &WorkItem::CollectWebFeed { source_id },
        )
        .await?;
        tx.commit().await.map_err(storage)?;
        Ok(next)
    }
    async fn import_subscriptions_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<Subscription>,
    ) -> Result<(), RepositoryError> {
        for value in &values {
            if value.workspace_id() != workspace {
                return Err(storage(
                    "import subscription scope does not match target workspace",
                ));
            }
            value.validate(self.reason_policy).map_err(storage)?;
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        for value in values {
            let source = SourceDefinition::new(
                SourceId::new(),
                value.source_url().clone(),
                SourceKind::Auto,
            )
            .map_err(storage)?;
            let item = WorkItem::PollSource {
                source_id: source.id(),
            };
            provision_subscription_tx(
                &mut tx,
                &value,
                &source,
                item,
                JobId::new().as_uuid(),
                self.initial_scope,
                false,
                false,
                None,
            )
            .await?;
        }
        tx.commit().await.map_err(storage)
    }
    async fn apply_seed_atomic(
        &self,
        workspace: WorkspaceId,
        values: Vec<(String, Subscription, SeedSource, serde_json::Value)>,
    ) -> Result<(), RepositoryError> {
        struct Prepared {
            key: String,
            value: Subscription,
            configuration: String,
            source: SourceDefinition,
            item: WorkItem,
            recipe: Option<String>,
        }
        let mut prepared = Vec::with_capacity(values.len());
        for (key, value, kind, configuration) in values {
            if value.workspace_id() != workspace {
                return Err(storage(
                    "seed subscription scope does not match target workspace",
                ));
            }
            value.validate(self.reason_policy).map_err(storage)?;
            let source_id = SourceId::from_uuid(Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                value.source_url().as_str().as_bytes(),
            ));
            let source_kind = match kind {
                SeedSource::Feed => SourceKind::Auto,
                SeedSource::Imported => imported_source_kind(&configuration)?,
            };
            let recipe = editable_recipe_document(workspace, value.source_url(), &source_kind)?;
            let source =
                SourceDefinition::new(source_id, value.source_url().clone(), source_kind.clone())
                    .map_err(storage)?;
            let item = match source_kind {
                SourceKind::WebPage(_) | SourceKind::BuiltIn(_) => {
                    WorkItem::CollectWebFeed { source_id }
                }
                _ => WorkItem::PollSource { source_id },
            };
            prepared.push(Prepared {
                key,
                value,
                configuration: serde_json::to_string(&configuration).map_err(storage)?,
                source,
                item,
                recipe,
            });
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        for row in &prepared {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT document FROM subscriptions WHERE id=$1 FOR UPDATE",
            )
            .bind(row.value.id().as_uuid().to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            {
                if existing != serde_json::to_string(&row.value).map_err(storage)? {
                    return Err(storage(
                        "seed idempotency key conflicts with existing subscription",
                    ));
                }
            }
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT document FROM seed_items WHERE id=$1 FOR UPDATE",
            )
            .bind(&row.key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            {
                if existing != row.configuration {
                    return Err(storage(
                        "seed configuration conflicts with applied idempotency key",
                    ));
                }
            }
        }
        for row in prepared {
            let workspace = row.value.workspace_id().as_uuid().to_string();
            let exact_url = row.value.source_url_exact();
            let url_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM subscriptions \
                 WHERE document::jsonb ->> 'workspace_id'=$1 \
                   AND document::jsonb ->> 'source_url'=$2)",
            )
            .bind(workspace)
            .bind(exact_url)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
            if url_exists {
                sqlx::query(
                    "INSERT INTO seed_items(id,revision,document) VALUES($1,0,$2) \
                     ON CONFLICT(id) DO NOTHING",
                )
                .bind(row.key)
                .bind(row.configuration)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
                continue;
            }
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id=$1)")
                    .bind(row.value.id().as_uuid().to_string())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(storage)?;
            if exists {
                continue;
            }
            let job = Uuid::new_v5(
                &Uuid::NAMESPACE_OID,
                format!("seed-job/{}", row.key).as_bytes(),
            );
            provision_subscription_tx(
                &mut tx,
                &row.value,
                &row.source,
                row.item,
                job,
                self.initial_scope,
                false,
                row.recipe.is_some(),
                row.recipe.as_deref(),
            )
            .await?;
            sqlx::query("INSERT INTO seed_items(id,revision,document) VALUES($1,0,$2)")
                .bind(row.key)
                .bind(row.configuration)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)
    }
}
