use super::*;
use serde::Deserialize;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    revision: String,
    score: i32,
    arrival: String,
    article: String,
    seed: Option<Uuid>,
}

#[async_trait]
impl InterestStore for PostgresAiStore {
    async fn next_terms(
        &self,
        owners: &[Uuid],
        version: &str,
    ) -> Result<Option<(Uuid, Uuid, Uuid)>, AiError> {
        sqlx::query_as("SELECT p.owner,a.workspace_key::uuid,a.article_key::uuid FROM ai_profiles p JOIN workspaces w ON w.document::jsonb->>'owner'=p.owner::text JOIN articles a ON a.workspace_key=w.id JOIN LATERAL (SELECT m.id,m.document FROM library_origins o JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=w.id AND o.article_id=a.article_key ORDER BY (m.document::jsonb->>'fetched_at')::timestamptz DESC,m.id DESC LIMIT 1) m ON true WHERE (SELECT reader_ai_automatic_resume_at(statement_timestamp())) IS NULL AND p.owner=ANY($1) AND reader_ai_automatic_allowed(p.owner,w.id,a.article_key) AND NOT EXISTS(SELECT 1 FROM ai_definitions d WHERE d.owner=p.owner AND d.workspace::text=w.id AND d.article::text=a.article_key AND d.status<>'cancelled' AND (d.status='quarantined' OR (CASE WHEN d.status='quarantined' THEN NULL ELSE d.document::jsonb END)#>>'{job,promptVersion}'=$2)) AND NOT EXISTS(SELECT 1 FROM ai_terms_failures f WHERE f.owner=p.owner AND f.workspace::text=w.id AND f.article::text=a.article_key AND f.prompt_version=$2) ORDER BY a.is_read,a.arrival_order DESC,a.id DESC LIMIT 1")
        .bind(owners).bind(version).fetch_optional(&self.pool).await.map_err(|error| { log::error!("terms_candidate_query_failed sqlstate={:?} kind={:?}",error.as_database_error().and_then(|e|e.code()),std::mem::discriminant(&error)); storage(error) })
    }
    async fn reject_terms(
        &self,
        owner: Uuid,
        workspace: Uuid,
        article: Uuid,
        version: &str,
        reason: &str,
    ) -> Result<(), AiError> {
        owned_workspace(&self.pool, owner, workspace).await?;
        sqlx::query("INSERT INTO ai_terms_failures(owner,workspace,article,source_revision,prompt_version,reason) SELECT $1,$2,$3,m.id||'/'||(m.document::jsonb->>'source_revision')||'/'||(m.document::jsonb->>'refresh_id'),$4,$5 FROM library_origins o JOIN content_manifests m ON m.id=o.source_record_id WHERE o.workspace_id=$2::text AND o.article_id=$3::text ORDER BY (m.document::jsonb->>'fetched_at')::timestamptz DESC,m.id DESC LIMIT 1 ON CONFLICT DO NOTHING")
            .bind(owner).bind(workspace).bind(article).bind(version).bind(reason).execute(&self.pool).await.map_err(storage)?;
        Ok(())
    }
    async fn defer_terms(
        &self,
        c: &ClaimedDefinitions,
        reason: reader_ai::AiDeferral,
    ) -> Result<(), AiError> {
        owned_workspace(&self.pool, c.record.owner(), c.record.job().workspace_id).await?;
        let mut record = c.record.clone();
        record.defer()?;
        let changed = sqlx::query("UPDATE ai_definitions SET status='queued',document=$4,lease=NULL,lease_until=NULL,scheduled_at=reader_ai_deferred_until($5,clock_timestamp()) WHERE id=$1 AND owner=$2 AND lease=$3 AND status='generating' AND lease_until>now()")
        .bind(record.job().id).bind(record.owner()).bind(c.lease).bind(encode(&record)?).bind(reason==reader_ai::AiDeferral::PeakHours).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(AiError::Cancelled);
        }
        Ok(())
    }

    async fn retain_interest_input(
        &self,
        c: &InterestClaim,
        input: &InterestInput,
    ) -> Result<Option<InterestPrediction>, AiError> {
        owned_workspace(&self.pool, c.owner, c.workspace).await?;
        let body = encode(&input.body())?;
        let changed=sqlx::query("INSERT INTO interest_responses(id,owner,workspace,article,profile_revision,input) SELECT $1,$2,$3,$4,$5::text::bigint,$6 FROM interest_scores WHERE owner=$2 AND workspace=$3 AND article=$4 AND profile_revision=$5::text::bigint AND lease=$1 AND status='working'")
            .bind(c.lease).bind(c.owner).bind(c.workspace).bind(c.article).bind(&c.profile.revision).bind(&body).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(AiError::Conflict);
        }
        let cached: Option<(Vec<u8>, i32, bool)> = sqlx::query_as("SELECT response,response_status,interrupted FROM interest_responses WHERE owner=$1 AND workspace=$2 AND article=$3 AND profile_revision=$4::text::bigint AND input=$5 AND response IS NOT NULL AND response_status=200 AND interrupted=false ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(c.owner).bind(c.workspace).bind(c.article).bind(&c.profile.revision).bind(&body).fetch_optional(&self.pool).await.map_err(storage)?;
        if let Some((body, status, interrupted)) = cached {
            let reply = ProviderReply {
                body,
                status: status.try_into().map_err(storage)?,
                interrupted,
            };
            // Invalid paid replies are preserved for diagnosis, never reused.
            if let Ok(prediction) = reply.interest_result() {
                return Ok(Some(prediction));
            }
        }
        Ok(None)
    }
    async fn feed(
        &self,
        owner: Uuid,
        workspace: Uuid,
        cursor: Option<&str>,
        limit: u32,
        show_hidden: bool,
    ) -> Result<SmartFeed, AiError> {
        self.feed_order(
            owner,
            workspace,
            cursor,
            limit,
            FeedOrder::Smart(show_hidden),
        )
        .await
    }
    async fn random_feed(
        &self,
        owner: Uuid,
        workspace: Uuid,
        cursor: Option<&str>,
        limit: u32,
        seed: Uuid,
    ) -> Result<SmartFeed, AiError> {
        self.feed_order(owner, workspace, cursor, limit, FeedOrder::Random(seed))
            .await
    }
    async fn save_profile(
        &self,
        owner: Uuid,
        value: InterestProfile,
    ) -> Result<InterestProfile, AiError> {
        if value.prompt.trim().is_empty() || value.prompt.contains('\0') {
            return Err(AiError::Rejected);
        }
        let revision: i64 = value.revision.parse().map_err(|_| AiError::Rejected)?;
        if revision <= 0 {
            return Err(AiError::Rejected);
        }
        let changed=sqlx::query("UPDATE interest_profiles SET prompt=$2,revision=revision+1,updated_at=now() WHERE owner=$1 AND revision=$3 AND training_count=$4")
            .bind(owner).bind(value.prompt).bind(revision).bind(i64::try_from(value.training_count).map_err(|_|AiError::Rejected)?).execute(&self.pool).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(AiError::Conflict);
        }
        profile(&self.pool, owner).await?.ok_or(AiError::NotFound)
    }
    async fn claim_interest(
        &self,
        owners: &[Uuid],
        lease_seconds: u64,
    ) -> Result<Option<InterestClaim>, AiError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // A short scheduler lock admits one claim atomically; provider calls occur
        // after commit, never under this global lock or an article row lock.
        sqlx::query("SELECT pg_advisory_xact_lock(492871024)")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("UPDATE interest_scores SET status='failed',error='Interrupted request; retry explicitly',lease_until=now() WHERE status='working' AND lease_until<now() AND owner=ANY($1)").bind(owners).execute(&mut *tx).await.map_err(storage)?;
        let candidate:Option<(Uuid,String,String,String,String,String,i64,i64)>=sqlx::query_as("SELECT p.owner,a.workspace_key,a.article_key,COALESCE(a.document::jsonb#>>'{key,title}',''),COALESCE(a.document::jsonb#>>'{key,description}',''),p.prompt,p.revision,p.training_count FROM interest_profiles p JOIN workspaces w ON w.document::jsonb->>'owner'=p.owner::text JOIN articles a ON a.workspace_key=w.id LEFT JOIN interest_scores s ON s.owner=p.owner AND s.workspace=w.id::uuid AND s.article=a.article_key::uuid AND s.profile_revision=p.revision WHERE (SELECT reader_ai_automatic_resume_at(statement_timestamp())) IS NULL AND p.owner=ANY($1) AND reader_ai_automatic_allowed(p.owner,w.id,a.article_key) AND NOT EXISTS(SELECT 1 FROM interest_scores blocked WHERE blocked.owner=p.owner AND blocked.status='budget' AND blocked.lease_until>now()) AND NOT a.is_read AND reader_article_visible(a) AND (s.article IS NULL OR (s.dirty AND s.status!='working' AND s.status!='budget') OR (s.status='budget' AND s.lease_until<=now())) ORDER BY a.arrival_order DESC,a.id DESC LIMIT 1")
            .bind(owners).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some((owner, workspace, article, title, description, prompt, revision, count)) =
            candidate
        else {
            tx.commit().await.map_err(storage)?;
            return Ok(None);
        };
        let workspace = Uuid::parse_str(&workspace).map_err(storage)?;
        let article = Uuid::parse_str(&article).map_err(storage)?;
        let lease = Uuid::new_v4();
        sqlx::query("INSERT INTO interest_scores(owner,workspace,article,profile_revision,status,lease,lease_until) VALUES($1,$2,$3,$4,'working',$5,now()+$6*interval '1 second') ON CONFLICT(owner,workspace,article,profile_revision) DO UPDATE SET status='working',dirty=false,score=NULL,prediction=NULL,lease=EXCLUDED.lease,lease_until=EXCLUDED.lease_until,error=NULL")
            .bind(owner).bind(workspace).bind(article).bind(revision).bind(lease).bind(i64::try_from(lease_seconds).map_err(storage)?).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(Some(InterestClaim {
            owner,
            workspace,
            article,
            profile: InterestProfile {
                prompt,
                revision: revision.to_string(),
                training_count: count.try_into().map_err(storage)?,
            },
            lease,
            title,
            description,
        }))
    }
    async fn finish_interest(
        &self,
        c: &InterestClaim,
        result: Result<InterestPrediction, AiError>,
        reply: Option<&ProviderReply>,
    ) -> Result<(), AiError> {
        owned_workspace(&self.pool, c.owner, c.workspace).await?;
        let peak = matches!(&result, Err(AiError::AutomaticPaused));
        let (status, score, prediction, error, budget) = match result {
            Ok(v) => (
                "scored",
                Some(i16::from(v.score())),
                Some(encode(&v)?),
                None,
                false,
            ),
            Err(e) => (
                if matches!(e, AiError::Budget | AiError::AutomaticPaused) {
                    "budget"
                } else {
                    "failed"
                },
                None,
                None,
                Some(e.to_string()),
                matches!(e, AiError::Budget | AiError::AutomaticPaused),
            ),
        };
        let mut tx = self.pool.begin().await.map_err(storage)?;
        sqlx::query("INSERT INTO interest_responses(id,owner,workspace,article,profile_revision,input,response,response_status,interrupted) VALUES($1,$2,$3,$4,$5::text::bigint,$6,$7,$8,$9) ON CONFLICT(id) DO UPDATE SET response=EXCLUDED.response,response_status=EXCLUDED.response_status,interrupted=EXCLUDED.interrupted WHERE interest_responses.owner=EXCLUDED.owner AND interest_responses.workspace=EXCLUDED.workspace AND interest_responses.article=EXCLUDED.article AND interest_responses.profile_revision=EXCLUDED.profile_revision AND interest_responses.response IS NULL")
            .bind(c.lease).bind(c.owner).bind(c.workspace).bind(c.article).bind(&c.profile.revision).bind(encode(&c.profile)?).bind(reply.map(|r| &r.body)).bind(reply.map(|r|i32::from(r.status))).bind(reply.map(|r|r.interrupted)).execute(&mut *tx).await.map_err(storage)?;
        let changed=sqlx::query("UPDATE interest_scores SET status=CASE WHEN dirty AND NOT $10 THEN 'budget' ELSE $6 END,score=CASE WHEN dirty THEN NULL ELSE $7 END,prediction=CASE WHEN dirty THEN NULL ELSE $8 END,error=$9,lease_until=CASE WHEN $10 THEN reader_ai_deferred_until($11,clock_timestamp()) ELSE now() END WHERE owner=$1 AND workspace=$2 AND article=$3 AND profile_revision=$4::text::bigint AND lease=$5 AND status='working'")
            .bind(c.owner).bind(c.workspace).bind(c.article).bind(&c.profile.revision).bind(c.lease).bind(status).bind(score).bind(prediction).bind(error).bind(budget).bind(peak).execute(&mut *tx).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(AiError::Conflict);
        }
        tx.commit().await.map_err(storage)
    }
}
async fn profile(pool: &PgPool, owner: Uuid) -> Result<Option<InterestProfile>, AiError> {
    let row: Option<(String, i64, i64)> = sqlx::query_as(
        "SELECT prompt,revision,training_count FROM interest_profiles WHERE owner=$1",
    )
    .bind(owner)
    .fetch_optional(pool)
    .await
    .map_err(storage)?;
    row.map(|(prompt, revision, count)| {
        Ok(InterestProfile {
            prompt,
            revision: revision.to_string(),
            training_count: count.try_into().map_err(storage)?,
        })
    })
    .transpose()
}

/// Random order uses PostgreSQL's native non-cryptographic hash for bounded SQL
/// keyset pagination, without loading a workspace into application memory. Seed
/// and profile revision fence the cursor; a new seed is an explicit reshuffle.
enum FeedOrder {
    Smart(bool),
    Random(Uuid),
}
impl PostgresAiStore {
    async fn feed_order(
        &self,
        owner: Uuid,
        workspace: Uuid,
        cursor: Option<&str>,
        limit: u32,
        order: FeedOrder,
    ) -> Result<SmartFeed, AiError> {
        let (show_hidden, seed) = match order {
            FeedOrder::Smart(show_hidden) => (show_hidden, None),
            FeedOrder::Random(seed) => (false, Some(seed)),
        };
        owned_workspace(&self.pool, owner, workspace).await?;
        if limit == 0 {
            return Err(AiError::Rejected);
        }
        let profile = profile(&self.pool, owner).await?;
        let revision = profile.as_ref().map(|p| p.revision.as_str()).unwrap_or("0");
        let position: Option<Cursor> = cursor
            .map(|v| serde_json::from_str(v).map_err(|_| AiError::Rejected))
            .transpose()?;
        if position.as_ref().is_some_and(|p| {
            p.seed != seed
                || p.revision != revision
                || !(0..=10).contains(&p.score)
                || p.arrival.strip_prefix('-').unwrap_or(&p.arrival).is_empty()
                || !p
                    .arrival
                    .strip_prefix('-')
                    .unwrap_or(&p.arrival)
                    .bytes()
                    .all(|b| b.is_ascii_digit())
                || Uuid::parse_str(&p.article).is_err()
        }) {
            return Err(AiError::Conflict);
        }
        let (total,scored,failed):(i64,i64,i64)=sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE s.status='scored' AND NOT s.dirty),count(*) FILTER(WHERE s.status='failed') FROM articles a LEFT JOIN interest_scores s ON s.owner=$1 AND s.workspace=$2 AND s.article=a.article_key::uuid AND s.profile_revision=$3::text::bigint WHERE a.workspace_key=$2::text AND NOT a.is_read AND reader_article_visible(a)")
            .bind(owner).bind(workspace).bind(revision).fetch_one(&self.pool).await.map_err(storage)?;
        let rows:Vec<FeedRow>=sqlx::query_as("SELECT a.article_key,COALESCE(a.document::jsonb#>>'{key,title}',''),COALESCE(a.document::jsonb#>>'{key,description}',''),CASE WHEN s.dirty THEN NULL ELSE s.prediction END,s.error,CASE WHEN $9::text IS NOT NULL OR s.dirty THEN 0 ELSE COALESCE(s.score,0) END,(CASE WHEN $9::text IS NULL THEN a.arrival_order ELSE hashtextextended(a.article_key,hashtextextended($9,0))::numeric END)::text FROM articles a LEFT JOIN interest_scores s ON s.owner=$1 AND s.workspace=$2 AND s.article=a.article_key::uuid AND s.profile_revision=$3::text::bigint WHERE a.workspace_key=$2::text AND NOT a.is_read AND reader_article_visible(a) AND ($8 OR s.dirty OR s.score IS DISTINCT FROM 1) AND ($4::integer IS NULL OR ((CASE WHEN $9::text IS NOT NULL OR s.dirty THEN 0 ELSE COALESCE(s.score,0) END),(CASE WHEN $9::text IS NULL THEN a.arrival_order ELSE hashtextextended(a.article_key,hashtextextended($9,0))::numeric END),a.article_key)<($4,$5::numeric,$6)) ORDER BY (CASE WHEN $9::text IS NOT NULL OR s.dirty THEN 0 ELSE COALESCE(s.score,0) END) DESC,(CASE WHEN $9::text IS NULL THEN a.arrival_order ELSE hashtextextended(a.article_key,hashtextextended($9,0))::numeric END) DESC,a.article_key DESC LIMIT $7")
            .bind(owner).bind(workspace).bind(revision).bind(position.as_ref().map(|p|p.score)).bind(position.as_ref().map(|p|p.arrival.as_str())).bind(position.as_ref().map(|p|p.article.as_str())).bind(i64::from(limit)+1).bind(show_hidden).bind(seed.map(|value|value.to_string())).fetch_all(&self.pool).await.map_err(storage)?;
        let extra = rows.len() > limit as usize;
        let mut articles = Vec::new();
        let mut next_cursor = None;
        for (id, title, description, prediction, error, score, arrival) in
            rows.into_iter().take(limit as usize)
        {
            if extra {
                next_cursor = Some(encode(&Cursor {
                    revision: revision.into(),
                    score,
                    arrival,
                    article: id.clone(),
                    seed,
                })?);
            }
            articles.push(SmartArticle {
                id: Uuid::parse_str(&id).map_err(storage)?,
                title,
                excerpt: reader_ai::article_plain_text(&description),
                prediction: prediction.as_deref().map(decode).transpose()?,
                error,
            });
        }
        Ok(SmartFeed {
            profile,
            articles,
            total: total.try_into().map_err(storage)?,
            scored: scored.try_into().map_err(storage)?,
            failed: failed.try_into().map_err(storage)?,
            next_cursor,
        })
    }
}

type FeedRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i32,
    String,
);
