use super::*;

pub(super) async fn reserve(
    tx: &mut Transaction<'_, Postgres>,
    owner: Uuid,
    id: Uuid,
    r: &SpendReservation,
) -> Result<(), AiError> {
    // One owner lock covers every mode and every worker; exact NUMERIC arithmetic.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("ai-budget/{owner}"))
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    let existing: Option<(Uuid, String, String)> =
        sqlx::query_as("SELECT owner,mode,reserved::text FROM ai_spending WHERE id=$1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(storage)?;
    if let Some((old_owner, mode, amount)) = existing {
        if old_owner != owner || mode != r.mode().name() || amount != r.amount() {
            return Err(AiError::Conflict);
        }
        return Ok(());
    }
    // Capture the admission date once under the owner lock, including across midnight.
    let day: chrono::NaiveDate =
        sqlx::query_scalar("SELECT (clock_timestamp() AT TIME ZONE 'Europe/Moscow')::date")
            .fetch_one(&mut **tx)
            .await
            .map_err(storage)?;
    let allowed:bool=sqlx::query_scalar("SELECT COALESCE(sum(COALESCE(actual,reserved)),0)+$2::numeric <= COALESCE((SELECT limit_usd FROM ai_budget_overrides WHERE owner=$1 AND day=$4),$3::numeric) FROM ai_spending WHERE owner=$1 AND day=$4")
        .bind(owner).bind(r.amount()).bind(r.limit()).bind(day).fetch_one(&mut **tx).await.map_err(storage)?;
    if !allowed {
        return Err(AiError::Budget);
    }
    sqlx::query(
        "INSERT INTO ai_spending(id,owner,mode,reserved,day) VALUES($1,$2,$3,$4::numeric,$5)",
    )
    .bind(id)
    .bind(owner)
    .bind(r.mode().name())
    .bind(r.amount())
    .bind(day)
    .execute(&mut **tx)
    .await
    .map_err(storage)?;
    Ok(())
}
impl PostgresAiStore {
    pub(super) async fn spending_view(
        &self,
        owner: Uuid,
        limit: &str,
    ) -> Result<AiSpending, AiError> {
        DecimalRate::parse(limit)?;
        let (today, limit): (chrono::NaiveDate, String) = sqlx::query_as("WITH d AS (SELECT (statement_timestamp() AT TIME ZONE 'Europe/Moscow')::date AS day) SELECT d.day, COALESCE((SELECT limit_usd::text FROM ai_budget_overrides WHERE owner=$1 AND day=d.day),$2) FROM d").bind(owner).bind(limit).fetch_one(&self.pool).await.map_err(storage)?;
        DecimalRate::parse(&limit)?;
        let (resets,spent,reserved,remaining):(String,String,String,String)=sqlx::query_as("SELECT (($3::date+1)::timestamp AT TIME ZONE 'Europe/Moscow')::text, COALESCE(sum(actual),0)::text,COALESCE(sum(reserved) FILTER(WHERE actual IS NULL),0)::text,GREATEST(0,$2::numeric-COALESCE(sum(COALESCE(actual,reserved)),0))::text FROM ai_spending WHERE owner=$1 AND day=$3")
            .bind(owner).bind(&limit).bind(today).fetch_one(&self.pool).await.map_err(storage)?;
        let rows:Vec<(String,String,String,String)>=sqlx::query_as("SELECT day::text,mode,COALESCE(sum(actual),0)::text,COALESCE(sum(reserved) FILTER(WHERE actual IS NULL),0)::text FROM ai_spending WHERE owner=$1 GROUP BY day,mode ORDER BY day,mode").bind(owner).fetch_all(&self.pool).await.map_err(storage)?;
        let days = rows
            .into_iter()
            .map(|(day, mode, spent_usd, reserved_usd)| {
                Ok(SpendingDay {
                    day,
                    mode: serde_json::from_value(serde_json::Value::String(mode))
                        .map_err(storage)?,
                    spent_usd,
                    reserved_usd,
                })
            })
            .collect::<Result<Vec<_>, AiError>>()?;
        Ok(AiSpending {
            daily_limit_usd: limit,
            today: today.to_string(),
            resets_at: resets,
            spent_usd: spent,
            reserved_usd: reserved,
            remaining_usd: remaining,
            days,
        })
    }
}
