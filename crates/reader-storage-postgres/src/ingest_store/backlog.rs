use super::*;

#[derive(Serialize, Deserialize)]
struct Context {
    source_revision: u64,
    fetched_at: DateTime<Utc>,
    final_url: url::Url,
    validators: CacheValidators,
}

impl PostgresIngestStore {
    pub(super) async fn pending_poll_batch(
        &self,
        source: SourceId,
        limit: usize,
    ) -> Result<Option<PollCommit>, StoreError> {
        if limit == 0 {
            return Err(StoreError::Unavailable(
                "backfill batch must be positive".into(),
            ));
        }
        let lookahead = limit
            .checked_add(1)
            .and_then(|v| i64::try_from(v).ok())
            .ok_or_else(|| {
                StoreError::Unavailable("backfill batch exceeds storage range".into())
            })?;
        let mut rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT document,action,context FROM poll_backlog WHERE source_id=$1 ORDER BY ordinal LIMIT $2",
        ).bind(source.as_uuid().to_string()).bind(lookahead).fetch_all(&self.pool).await.map_err(storage)?;
        if rows.is_empty() {
            return Ok(None);
        }
        let incomplete = rows.len() > limit;
        if incomplete {
            rows.pop();
        }
        let context = rows[0].2.clone();
        if rows.iter().any(|row| row.2 != context) {
            return Err(StoreError::Unavailable("mixed backfill generations".into()));
        }
        let context: Context = decode(&context)?;
        let records = rows
            .into_iter()
            .map(|(document, action, _)| {
                Ok(PolledRecord {
                    record: decode(&document)?,
                    action: decode(&action)?,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        if records.iter().any(|r| r.source_id() != source) {
            return Err(StoreError::Unavailable(
                "backfill source identity mismatch".into(),
            ));
        }
        Ok(Some(PollCommit {
            source_id: source,
            source_revision: context.source_revision,
            records,
            remainder: Vec::new(),
            fetched_at: context.fetched_at,
            final_url: context.final_url,
            validators: context.validators,
            incomplete,
            duration_ms: 0,
        }))
    }
}

pub(super) async fn persist(
    tx: &mut Transaction<'_, Postgres>,
    commit: &PollCommit,
) -> Result<(), StoreError> {
    if !commit.remainder.is_empty() && !commit.incomplete {
        return Err(StoreError::Unavailable(
            "backfill cannot be complete with remaining records".into(),
        ));
    }
    let mut identities = std::collections::HashSet::new();
    for value in commit.records.iter().chain(&commit.remainder) {
        if value.source_id() != commit.source_id || !identities.insert(value.upstream_id()) {
            return Err(StoreError::Unavailable(
                "invalid backfill source or duplicate upstream identity".into(),
            ));
        }
    }
    let context = encode(&Context {
        source_revision: commit.source_revision,
        fetched_at: commit.fetched_at,
        final_url: commit.final_url.clone(),
        validators: commit.validators.clone(),
    })?;
    for (ordinal, value) in commit.remainder.iter().enumerate() {
        sqlx::query("INSERT INTO poll_backlog(source_id,record_id,ordinal,document,action,context) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(commit.source_id.as_uuid().to_string()).bind(value.id().as_uuid().to_string())
            .bind(i64::try_from(ordinal).map_err(storage)?).bind(encode(&value.record)?)
            .bind(encode(&value.action)?).bind(&context).execute(&mut **tx).await.map_err(storage)?;
    }
    for value in &commit.records {
        let prior: Option<(String, String)> = sqlx::query_as("DELETE FROM poll_backlog WHERE source_id=$1 AND record_id=$2 RETURNING document,action")
            .bind(commit.source_id.as_uuid().to_string()).bind(value.id().as_uuid().to_string())
            .fetch_optional(&mut **tx).await.map_err(storage)?;
        if let Some((record, action)) = prior {
            if decode::<SourceRecord>(&record)? != value.record
                || decode::<PollAction>(&action)? != value.action
            {
                return Err(StoreError::Conflict);
            }
        }
    }
    Ok(())
}
