//! PostgreSQL delivery operations. Transaction boundaries stay local and explicit.
use super::*;

pub(super) async fn delivery_targets(
    store: &PostgresIngestStore,
    source: SourceId,
    after: Option<SubscriptionId>,
    limit: usize,
) -> Result<Vec<DeliveryTarget>, StoreError> {
    let after = after.map(|id| id.as_uuid().to_string()).unwrap_or_default();
    let rows = sqlx::query(
        "SELECT s.document
             FROM subscription_sources AS link
             JOIN subscriptions AS s ON s.id = link.subscription_id
             WHERE link.source_id = $1 AND link.subscription_id > $2
             ORDER BY link.subscription_id
             LIMIT $3",
    )
    .bind(source.as_uuid().to_string())
    .bind(after)
    .bind(i64::try_from(limit).map_err(storage)?)
    .fetch_all(&store.pool)
    .await
    .map_err(storage)?;
    let mut targets = Vec::with_capacity(rows.len());
    for row in rows {
        let subscription: Subscription = decode(row.try_get("document").map_err(storage)?)?;
        if !matches!(subscription.status(), SubscriptionStatus::Active) {
            continue;
        }
        let workspace_document =
            sqlx::query_scalar::<_, String>("SELECT document FROM workspaces WHERE id = $1")
                .bind(subscription.workspace_id().as_uuid().to_string())
                .fetch_optional(&store.pool)
                .await
                .map_err(storage)?
                .ok_or(StoreError::NotFound)?;
        let workspace: Workspace = decode(&workspace_document)?;
        if workspace.accepts_delivery() {
            targets.push(DeliveryTarget {
                workspace_id: subscription.workspace_id(),
                subscription_id: subscription.id(),
            });
        }
    }
    Ok(targets)
}

pub(super) async fn deliver(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    commit: DeliveryCommit,
) -> Result<DeliveryResult, StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
    assert_lease(&mut tx, lease).await?;
    let workspace_id = commit.target.workspace_id;
    let workspace = workspace_id.as_uuid().to_string();
    let subscription = commit.target.subscription_id.as_uuid().to_string();
    let sub_document: String =
        sqlx::query_scalar("SELECT document FROM subscriptions WHERE id = $1 FOR UPDATE")
            .bind(&subscription)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .ok_or(StoreError::NotFound)?;
    let sub_value: Subscription = decode(&sub_document)?;
    let workspace_document: String =
        sqlx::query_scalar("SELECT document FROM workspaces WHERE id = $1 FOR UPDATE")
            .bind(&workspace)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
            .ok_or(StoreError::NotFound)?;
    let workspace_value: Workspace = decode(&workspace_document)?;
    if sub_value.workspace_id() != workspace_id || workspace_value.id() != workspace_id {
        return Err(StoreError::NotFound);
    }
    if !matches!(sub_value.status(), SubscriptionStatus::Active)
        || !workspace_value.accepts_delivery()
    {
        tx.commit().await.map_err(storage)?;
        return Ok(DeliveryResult::SkippedInactive);
    }

    let record = commit.record;
    let origin_rows = sqlx::query(
        "SELECT article_id, subscription_id FROM library_origins
             WHERE workspace_id = $1 AND source_record_id = $2
             FOR UPDATE",
    )
    .bind(&workspace)
    .bind(record.id().as_uuid().to_string())
    .fetch_all(&mut *tx)
    .await
    .map_err(storage)?;
    let mut prior_by_article = std::collections::BTreeMap::<String, Vec<String>>::new();
    for row in origin_rows {
        prior_by_article
            .entry(row.try_get("article_id").map_err(storage)?)
            .or_default()
            .push(row.try_get("subscription_id").map_err(storage)?);
    }
    let mut inherited = Vec::new();
    let mut moved_articles = Vec::new();
    let mut moved_subscriptions = Vec::new();
    let mut retained_identity = None;
    for (old_id, subscriptions) in prior_by_article {
        let old_key = format!("{workspace}/{old_id}");
        let old_document: String =
            sqlx::query_scalar("SELECT document FROM articles WHERE id = $1 FOR UPDATE")
                .bind(&old_key)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or_else(|| {
                    StoreError::Unavailable("library origin references a missing article".into())
                })?;
        let mut old: Article = decode(&old_document)?;
        if old.key == *record.key() {
            continue;
        }
        moved_articles.push(old.id);
        inherited.push((old.state, old.first_arrived_at));
        old.detach_origin(record.id());
        if old.origins.is_empty() && retained_identity.is_none() {
            let mut retained = old.clone();
            retained.key = record.key().clone();
            retained_identity = Some(retained);
        }
        sqlx::query(
            "DELETE FROM library_origins
                 WHERE workspace_id = $1 AND article_id = $2 AND source_record_id = $3",
        )
        .bind(&workspace)
        .bind(&old_id)
        .bind(record.id().as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        for linked in subscriptions {
            if !moved_subscriptions.contains(&linked) {
                moved_subscriptions.push(linked);
            }
        }
        let old_dedup = encode(&old.key)?;
        if old.origins.is_empty() {
            sqlx::query("DELETE FROM articles WHERE id = $1")
                .bind(old_key)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            sqlx::query(
                "DELETE FROM library_dedup \
                     WHERE workspace_id = $1 AND dedup_hash = $2 AND dedup_key = $3",
            )
            .bind(&workspace)
            .bind(dedup_fingerprint(&old_dedup))
            .bind(old_dedup)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        } else {
            upsert_article(&mut tx, &workspace, &old, &old_dedup).await?;
        }
    }

    let dedup_key = encode(record.key())?;
    let dedup_hash = dedup_fingerprint(&dedup_key);
    let found = sqlx::query_as::<_, (String, String)>(
        "SELECT dedup_key, article_id FROM library_dedup
             WHERE workspace_id = $1 AND dedup_hash = $2 FOR UPDATE",
    )
    .bind(&workspace)
    .bind(&dedup_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(storage)?;
    let found = match found {
        Some((stored_key, _)) if stored_key != dedup_key => {
            return Err(StoreError::Unavailable(format!(
                "dedup fingerprint collision in workspace {workspace}"
            )));
        }
        Some((_, id)) => {
            // The index identifies an article; it never owns mutable user
            // state. Lock the authoritative row against concurrent commands.
            let document: String =
                sqlx::query_scalar("SELECT document FROM articles WHERE id = $1 FOR UPDATE")
                    .bind(format!("{workspace}/{id}"))
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
                    .ok_or_else(|| {
                        StoreError::Unavailable("dedup index references a missing article".into())
                    })?;
            let article: Article = decode(&document)?;
            if article.id.as_uuid().to_string() != id || article.key != *record.key() {
                return Err(StoreError::Unavailable(
                    "dedup index disagrees with article identity".into(),
                ));
            }
            Some(article)
        }
        None => None,
    };
    let inherited_time = inherited.iter().map(|(_, time)| *time).min();
    let inherited_state = ArticleState::merge(inherited.into_iter().map(|(state, _)| state));
    let found = match (found, inherited_state) {
        (Some(mut article), Some(state)) => {
            article.state = ArticleState::merge([article.state, state]).expect("two states");
            if let Some(time) = inherited_time {
                article.first_arrived_at = article.first_arrived_at.min(time);
            }
            Some(article)
        }
        (Some(article), None) => Some(article),
        (None, Some(state)) => {
            let mut article = retained_identity.unwrap_or(Article {
                id: commit.proposed_article_id,
                key: record.key().clone(),
                state,
                first_arrived_at: inherited_time.unwrap_or(commit.delivered_at),
                origins: vec![],
                revision: 0,
            });
            article.state = state;
            article.first_arrived_at = inherited_time.unwrap_or(article.first_arrived_at);
            Some(article)
        }
        (None, None) => None,
    };
    let (mut article, result) = match found {
        Some(article) => {
            let id = article.id;
            (article, DeliveryResult::AlreadyDelivered(id))
        }
        None => (
            Article {
                id: commit.proposed_article_id,
                key: record.key().clone(),
                state: ArticleState::default(),
                first_arrived_at: commit.delivered_at,
                origins: vec![],
                revision: 0,
            },
            DeliveryResult::Delivered(commit.proposed_article_id),
        ),
    };
    article.attach_origin(record.id());
    upsert_article(&mut tx, &workspace, &article, &dedup_key).await?;

    if !moved_subscriptions.contains(&subscription) {
        moved_subscriptions.push(subscription);
    }
    for linked in &moved_subscriptions {
        sqlx::query(
            "INSERT INTO library_origins
                 (workspace_id, article_id, subscription_id, source_record_id)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT DO NOTHING",
        )
        .bind(&workspace)
        .bind(article.id.as_uuid().to_string())
        .bind(linked)
        .bind(record.id().as_uuid().to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
    }

    migrate_rule_provenance(&mut tx, &workspace, article.id, &moved_articles).await?;
    for old in &moved_articles {
        if *old != article.id {
            // History is immutable and can belong to both branches of a
            // split. Record lineage instead of rewriting paid job inputs
            // or racing their workers. Edges are workspace-scoped closure.
            sqlx::query(
                "INSERT INTO article_history_links(workspace_id,article_id,history_article_id)
                     SELECT $1,$2,$3 UNION
                     SELECT workspace_id,$2,history_article_id FROM article_history_links
                     WHERE workspace_id=$1 AND article_id=$3
                     ON CONFLICT DO NOTHING",
            )
            .bind(&workspace)
            .bind(article.id.as_uuid().to_string())
            .bind(old.as_uuid().to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
    }
    enqueue_article_rule_jobs(
        &mut tx,
        workspace_id,
        &sub_value,
        &article,
        &record,
        moved_subscriptions,
    )
    .await?;
    tx.commit().await.map_err(storage)?;
    Ok(result)
}

pub(super) async fn advance_fanout(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    after: SubscriptionId,
) -> Result<(), StoreError> {
    let WorkItem::FanOut {
        source_id,
        record_id,
        ..
    } = lease.item
    else {
        return Ok(());
    };
    let item = WorkItem::FanOut {
        source_id,
        record_id,
        after_subscription: Some(after),
    };
    let id = durable_job_id(&format!(
        "fanout-continuation/{}/{}",
        lease.job_id.as_uuid(),
        after.as_uuid()
    ));
    let mut tx = store.pool.begin().await.map_err(storage)?;
    assert_lease(&mut tx, lease).await?;
    enqueue_work(
        &mut tx,
        id.as_uuid().to_string(),
        &item,
        Utc::now().timestamp_millis(),
    )
    .await?;
    tx.commit().await.map_err(storage)
}

pub(super) async fn upsert_article(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    article: &Article,
    dedup_key: &str,
) -> Result<(), StoreError> {
    let document = encode(article)?;
    let revision = to_i64(article.revision, "article revision")?;
    let dedup_hash = dedup_fingerprint(dedup_key);
    sqlx::query(
        "INSERT INTO articles (id, revision, document) VALUES ($1, $2, $3)
         ON CONFLICT (id) DO UPDATE
         SET revision = EXCLUDED.revision, document = EXCLUDED.document",
    )
    .bind(format!("{workspace}/{}", article.id.as_uuid()))
    .bind(revision)
    .bind(&document)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    let result = sqlx::query(
        "INSERT INTO library_dedup
         (workspace_id, dedup_hash, dedup_key, article_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (workspace_id, dedup_hash) DO UPDATE
         SET article_id = EXCLUDED.article_id
         WHERE library_dedup.dedup_key = EXCLUDED.dedup_key",
    )
    .bind(workspace)
    .bind(dedup_hash)
    .bind(dedup_key)
    .bind(article.id.as_uuid().to_string())
    .execute(&mut **tx)
    .await
    .map_err(storage)?
    .rows_affected();
    if result == 0 {
        return Err(StoreError::Unavailable(format!(
            "dedup fingerprint collision in workspace {workspace}"
        )));
    }
    Ok(())
}

pub(super) async fn migrate_rule_provenance(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    article: ArticleId,
    moved_articles: &[ArticleId],
) -> Result<(), StoreError> {
    for old_article in moved_articles {
        if *old_article == article {
            continue;
        }
        let rows = sqlx::query(
            "SELECT rule_id, rule_version, document FROM rule_evaluations
             WHERE workspace_id = $1 AND article_id = $2 FOR UPDATE",
        )
        .bind(workspace)
        .bind(old_article.as_uuid().to_string())
        .fetch_all(&mut **tx)
        .await
        .map_err(storage)?;
        for row in rows {
            let rule_id: String = row.try_get("rule_id").map_err(storage)?;
            let version: i64 = row.try_get("rule_version").map_err(storage)?;
            let old_document: String = row.try_get("document").map_err(storage)?;
            let existing = sqlx::query_scalar::<_, String>(
                "SELECT document FROM rule_evaluations
                 WHERE workspace_id = $1 AND article_id = $2
                   AND rule_id = $3 AND rule_version = $4 FOR UPDATE",
            )
            .bind(workspace)
            .bind(article.as_uuid().to_string())
            .bind(&rule_id)
            .bind(version)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?;
            let document = match existing {
                Some(current) => encode(&serde_json::json!({
                    "merged_provenance": [
                        serde_json::from_str::<serde_json::Value>(&current).map_err(storage)?,
                        serde_json::from_str::<serde_json::Value>(&old_document).map_err(storage)?,
                    ],
                }))?,
                None => old_document,
            };
            sqlx::query(
                "INSERT INTO rule_evaluations
                 (workspace_id, article_id, rule_id, rule_version, document)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (workspace_id, article_id, rule_id, rule_version)
                 DO UPDATE SET document = EXCLUDED.document",
            )
            .bind(workspace)
            .bind(article.as_uuid().to_string())
            .bind(&rule_id)
            .bind(version)
            .bind(document)
            .execute(&mut **tx)
            .await
            .map_err(storage)?;
            sqlx::query(
                "DELETE FROM rule_evaluations
                 WHERE workspace_id = $1 AND article_id = $2
                   AND rule_id = $3 AND rule_version = $4",
            )
            .bind(workspace)
            .bind(old_article.as_uuid().to_string())
            .bind(rule_id)
            .bind(version)
            .execute(&mut **tx)
            .await
            .map_err(storage)?;
        }
    }
    Ok(())
}
