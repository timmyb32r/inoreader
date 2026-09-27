//! PostgreSQL rules operations. Transaction boundaries stay local and explicit.
use super::*;

pub(super) async fn evaluate_article_rules(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    workspace: WorkspaceId,
    article: ArticleId,
    evaluations: &[PendingRuleEvaluation],
) -> Result<(), StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
    assert_lease(&mut tx, lease).await?;
    let workspace_text = workspace.as_uuid().to_string();
    let article_text = article.as_uuid().to_string();
    let key = format!("{workspace_text}/{article_text}");
    let Some(document) =
        sqlx::query_scalar::<_, String>("SELECT document FROM articles WHERE id = $1 FOR UPDATE")
            .bind(&key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
    else {
        tx.commit().await.map_err(storage)?;
        return Ok(());
    };
    let mut article: Article = decode(&document)?;
    let original_state = article.state;
    let (full_text, generations) = article_full_text(&mut tx, &article).await?;
    for pending in evaluations {
        let rule_key = format!("{workspace_text}/{}", pending.rule_id.as_uuid());
        let Some(rule_document) =
            sqlx::query_scalar::<_, String>("SELECT document FROM rules WHERE id = $1")
                .bind(rule_key)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
        else {
            continue;
        };
        let rule: Rule = decode(&rule_document)?;
        rule.validate().map_err(storage)?;
        if !pending.still_valid_for(&rule) {
            continue;
        }
        ensure_rule_content(&rule, &article, full_text.as_deref())?;
        let matched = rule.matches(&article.key.title, full_text.as_deref());
        let before = article.state;
        if matched {
            rule.apply(&mut article.state);
        }
        let reason = encode(&serde_json::json!({
            "matched": matched,
            "action": rule.action,
            "state_changed": before != article.state,
            "manual_override_preserved": matched && before == article.state,
            "content_generations": generations,
        }))?;
        upsert_rule_evaluation(
            &mut tx,
            &workspace_text,
            &article_text,
            rule.id,
            rule.version,
            &reason,
        )
        .await?;
    }
    if article.state != original_state {
        article.revision = article.revision.saturating_add(1);
        sqlx::query("UPDATE articles SET revision = $1, document = $2 WHERE id = $3")
            .bind(to_i64(article.revision, "article revision")?)
            .bind(encode(&article)?)
            .bind(key)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
    }
    tx.commit().await.map_err(storage)
}

// Mirrors the existing semantic IngestStore port; transaction inputs stay explicit.
#[allow(clippy::too_many_arguments)]
pub(super) async fn apply_rule_batch(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    workspace_id: WorkspaceId,
    rule_id: RuleId,
    version: u64,
    after_id: Option<ArticleId>,
    through_id: Option<ArticleId>,
    limit: usize,
) -> Result<Option<ArticleId>, StoreError> {
    let mut tx = store.pool.begin().await.map_err(storage)?;
    assert_lease(&mut tx, lease).await?;
    let workspace = workspace_id.as_uuid().to_string();
    let rule_text = rule_id.as_uuid().to_string();
    let Some(rule_document) =
        sqlx::query_scalar::<_, String>("SELECT document FROM rules WHERE id = $1 FOR UPDATE")
            .bind(format!("{workspace}/{rule_text}"))
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?
    else {
        tx.commit().await.map_err(storage)?;
        return Ok(None);
    };
    let rule: Rule = decode(&rule_document)?;
    rule.validate().map_err(storage)?;
    if !rule.enabled || rule.version != version {
        tx.commit().await.map_err(storage)?;
        return Ok(None);
    }
    let prefix = format!("{workspace}/");
    let upper = format!("{workspace}0");
    let after = after_id
        .map(|value| format!("{workspace}/{}", value.as_uuid()))
        .unwrap_or_else(|| prefix.clone());
    let through = match through_id {
        Some(value) => value,
        None => {
            let boundary = sqlx::query_scalar::<_, String>(
                "SELECT id FROM articles
                     WHERE id > $1 AND id < $2 ORDER BY id DESC LIMIT 1",
            )
            .bind(&prefix)
            .bind(&upper)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
            let Some(boundary) = boundary else {
                tx.commit().await.map_err(storage)?;
                return Ok(None);
            };
            ArticleId::from_uuid(
                uuid::Uuid::parse_str(boundary.rsplit('/').next().unwrap_or_default())
                    .map_err(storage)?,
            )
        }
    };
    let rows = sqlx::query(
        "SELECT id, document FROM articles
             WHERE id > $1 AND id <= $2 ORDER BY id LIMIT $3 FOR UPDATE",
    )
    .bind(after)
    .bind(format!("{workspace}/{}", through.as_uuid()))
    .bind(i64::try_from(limit).map_err(storage)?)
    .fetch_all(&mut *tx)
    .await
    .map_err(storage)?;
    let count = rows.len();
    let mut last = None;
    for row in rows {
        let key: String = row.try_get("id").map_err(storage)?;
        let mut article: Article = decode(row.try_get("document").map_err(storage)?)?;
        let before = article.state;
        let (full_text, _) = article_full_text(&mut tx, &article).await?;
        ensure_rule_content(&rule, &article, full_text.as_deref())?;
        let matched = rule.matches(&article.key.title, full_text.as_deref());
        if matched {
            rule.apply(&mut article.state);
        }
        if article.state != before {
            article.revision = article.revision.saturating_add(1);
            sqlx::query("UPDATE articles SET revision = $1, document = $2 WHERE id = $3")
                .bind(to_i64(article.revision, "article revision")?)
                .bind(encode(&article)?)
                .bind(key)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        let reason = encode(&serde_json::json!({
            "matched": matched,
            "action": rule.action,
            "state_changed": before != article.state,
            "manual_override_preserved": matched && before == article.state,
        }))?;
        upsert_rule_evaluation(
            &mut tx,
            &workspace,
            &article.id.as_uuid().to_string(),
            rule.id,
            version,
            &reason,
        )
        .await?;
        last = Some(article.id);
    }
    if count == limit {
        if let Some(cursor) = last {
            if cursor != through {
                let next = WorkItem::ApplyRule {
                    workspace_id,
                    rule_id: rule.id,
                    rule_version: version,
                    after_article: Some(cursor),
                    through_article: Some(through),
                };
                enqueue_work(
                    &mut tx,
                    rule_job(rule.id, version, cursor).as_uuid().to_string(),
                    &next,
                    Utc::now().timestamp_millis(),
                )
                .await?;
            }
        }
    }
    tx.commit().await.map_err(storage)?;
    Ok(last)
}

pub(super) async fn enqueue_article_rule_jobs(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    subscription: &Subscription,
    article: &Article,
    record: &SourceRecord,
    subscriptions: Vec<String>,
) -> Result<(), StoreError> {
    let prefix = format!("{}/", workspace.as_uuid());
    let upper = format!("{}0", workspace.as_uuid());
    let documents =
        sqlx::query_scalar::<_, String>("SELECT document FROM rules WHERE id >= $1 AND id < $2")
            .bind(prefix)
            .bind(upper)
            .fetch_all(&mut **tx)
            .await
            .map_err(storage)?;
    let mut rules = Vec::new();
    for document in documents {
        let rule: Rule = decode(&document)?;
        rule.validate().map_err(storage)?;
        if rule.enabled {
            rules.push(rule);
        }
    }
    for linked in subscriptions {
        let linked = SubscriptionId::from_uuid(uuid::Uuid::parse_str(&linked).map_err(storage)?);
        let evaluations = rules
            .iter()
            .filter(|rule| rule.subscription_id == linked)
            .map(|rule| PendingRuleEvaluation {
                rule_id: rule.id,
                rule_version: rule.version,
                subscription_id: rule.subscription_id,
            })
            .collect::<Vec<_>>();
        if evaluations.is_empty() {
            continue;
        }
        let item = WorkItem::EvaluateArticleRules {
            workspace_id: subscription.workspace_id(),
            article_id: article.id,
            evaluations,
        };
        enqueue_work(
            tx,
            article_rule_job(article.id, linked, record.id(), record.revision())
                .as_uuid()
                .to_string(),
            &item,
            Utc::now().timestamp_millis(),
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn article_full_text(
    tx: &mut Transaction<'_, Postgres>,
    article: &Article,
) -> Result<(Option<String>, Vec<uuid::Uuid>), StoreError> {
    let records: Vec<String> = article
        .origins
        .iter()
        .map(|id| id.as_uuid().to_string())
        .collect();
    let snapshots = crate::content_snapshot::read_origins(tx, &records)
        .await
        .map_err(storage)?;
    let mut text = String::new();
    let mut generations = Vec::with_capacity(snapshots.len());
    for snapshot in snapshots {
        text.push_str(&snapshot.html);
        generations.push(snapshot.pointer.refresh_id);
    }
    Ok(((!text.is_empty()).then_some(text), generations))
}

pub(super) fn ensure_rule_content(
    rule: &Rule,
    article: &Article,
    full_text: Option<&str>,
) -> Result<(), StoreError> {
    let title_match = rule.matches(&article.key.title, None);
    if full_text.is_none()
        && (matches!(rule.field, RuleField::Text)
            || (matches!(rule.field, RuleField::Both) && !title_match))
    {
        return Err(StoreError::Unavailable(
            "rule evaluation awaits fulltext".into(),
        ));
    }
    Ok(())
}

pub(super) async fn upsert_rule_evaluation(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    article: &str,
    rule: RuleId,
    version: u64,
    document: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO rule_evaluations
         (workspace_id, article_id, rule_id, rule_version, document)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (workspace_id, article_id, rule_id, rule_version)
         DO UPDATE SET document = EXCLUDED.document",
    )
    .bind(workspace)
    .bind(article)
    .bind(rule.as_uuid().to_string())
    .bind(to_i64(version, "rule version")?)
    .bind(document)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}
