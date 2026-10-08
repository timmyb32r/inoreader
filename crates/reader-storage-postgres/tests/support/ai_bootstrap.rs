use super::*;
use reader_ai::{AiError, AiStore, InterestStore, SpendMode, SpendReservation};
use reader_ingest::{
    DeliveryCommit, DeliveryTarget, LeasedWork, PollAction, PollCommit, PolledRecord, SourceRecord,
};

/// Historical-schema fixtures restore the old triggers before exercising upgrade.
pub async fn remove_schema(pool: &PgPool) {
    sqlx::raw_sql("DROP FUNCTION reader_ai_terms_superseded(UUID); DROP INDEX ai_definitions_article_prompt; DROP INDEX ai_terms_failures_article_prompt;").execute(pool).await.unwrap();
    sqlx::raw_sql("DROP FUNCTION reader_ai_deferred_until(BOOLEAN,TIMESTAMPTZ); DROP FUNCTION reader_ai_automatic_resume_at(TIMESTAMPTZ); DROP TABLE ai_automatic_schedule;").execute(pool).await.unwrap();
    sqlx::query("ALTER TABLE article_read_events DROP COLUMN IF EXISTS method")
        .execute(pool)
        .await
        .unwrap();
    sqlx::raw_sql("DROP TRIGGER ai_new_origin ON library_origins; DROP FUNCTION reader_ai_origin_candidate(); DROP FUNCTION reader_ai_finalize_subscription(TEXT,TEXT); DROP FUNCTION reader_ai_automatic_allowed(UUID,TEXT,TEXT); DROP INDEX ai_definitions_active_lease; DROP INDEX interest_scores_active_lease; DROP TABLE subscription_ai_selected,subscription_ai_initial_records,subscription_ai_bootstrap,ai_record_discovery,ai_bootstrap_policy; ALTER TABLE ai_chats DROP COLUMN automatic; ALTER TABLE ai_definitions DROP COLUMN automatic;")
        .execute(pool).await.unwrap();
    let old = include_str!("../../src/schema/ai_budget.sql");
    sqlx::raw_sql(&old[old.find("-- Candidate creation").unwrap()..])
        .execute(pool)
        .await
        .unwrap();
}

/// Attach a valid origin to an independently created article used by older AI
/// unit-of-storage fixtures. Real collection/delivery is exercised below.
pub async fn admit_fixture(
    pool: &PgPool,
    reader: &PostgresRepository,
    workspace: WorkspaceId,
    article: ArticleId,
) {
    let subscription = Subscription::new(
        SubscriptionId::new(),
        workspace,
        url::Url::parse(&format!("https://fixture.example/{}/feed", Uuid::new_v4())).unwrap(),
        "AI fixture".into(),
    );
    reader
        .save_subscription(None, subscription.clone())
        .await
        .unwrap();
    let sub = subscription.id().as_uuid().to_string();
    let source: String =
        sqlx::query_scalar("SELECT source_id FROM subscription_sources WHERE subscription_id=$1")
            .bind(&sub)
            .fetch_one(pool)
            .await
            .unwrap();
    let record = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO ai_record_discovery(record,source,source_order) VALUES($1,$2,0)")
        .bind(&record)
        .bind(source)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE subscription_ai_bootstrap SET cutoff=0,ready=true WHERE subscription=$1")
        .bind(&sub)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO library_origins(workspace_id,article_id,subscription_id,source_record_id) VALUES($1,$2,$3,$4)")
        .bind(workspace.as_uuid().to_string()).bind(article.as_uuid().to_string()).bind(sub).bind(record).execute(pool).await.unwrap();
}

pub async fn verify(pool: &PgPool) {
    let reader = std::sync::Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 100, 86400).unwrap(),
    );
    let store = PostgresIngestStore::new(
        pool.clone(),
        ChronoDuration::minutes(5),
        4,
        ChronoDuration::days(1),
    )
    .unwrap();
    let ai = reader_storage_postgres::PostgresAiStore::new(
        pool.clone(),
        reader.clone(),
        std::num::NonZeroU32::new(20).unwrap(),
    );
    assert!(matches!(
        ai.configure_initial_articles(0).await,
        Err(AiError::Configuration)
    ));
    ai.configure_initial_articles(10).await.unwrap();
    let owner = AccountId::new();
    let workspace = Workspace::new(WorkspaceId::new(), owner, "Initial AI cap".into());
    reader
        .save_workspace(None, workspace.clone())
        .await
        .unwrap();
    let sub = Subscription::new(
        SubscriptionId::new(),
        workspace.id(),
        url::Url::parse(&format!(
            "https://bootstrap.example/{}/feed",
            Uuid::new_v4()
        ))
        .unwrap(),
        "Initial".into(),
    );
    reader.save_subscription(None, sub.clone()).await.unwrap();
    let source: String =
        sqlx::query_scalar("SELECT source_id FROM subscription_sources WHERE subscription_id=$1")
            .bind(sub.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    let source = SourceId::from_uuid(Uuid::parse_str(&source).unwrap());
    let item = WorkItem::RefreshSource { source_id: source };
    let lease = LeasedWork {
        job_id: JobId::new(),
        item: item.clone(),
        token: LeaseToken::new(),
        deadline: Utc::now() + ChronoDuration::minutes(10),
        attempt: 0,
    };
    sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',0,$2,'https://bootstrap.example',0,$3,0,$4,$5)")
        .bind(lease.job_id.as_uuid().to_string()).bind(Utc::now().timestamp_millis()).bind(serde_json::to_string(&item).unwrap()).bind(lease.token.as_uuid().to_string()).bind(lease.deadline.timestamp_millis()).execute(pool).await.unwrap();
    let mut records: Vec<SourceRecord> = (0..100).map(|index| record(source, index)).collect();
    let mut duplicate = serde_json::to_value(&records[99]).unwrap();
    duplicate["id"] = serde_json::json!(Uuid::new_v4());
    duplicate["upstream_id"] = serde_json::json!("duplicate-latest");
    records.push(serde_json::from_value(duplicate).unwrap());
    let commit = |rows: &[SourceRecord], remainder: &[SourceRecord], incomplete: bool| PollCommit {
        source_id: source,
        source_revision: 0,
        records: rows
            .iter()
            .cloned()
            .map(|record| PolledRecord {
                record,
                action: PollAction::Deliver,
            })
            .collect(),
        remainder: remainder
            .iter()
            .cloned()
            .map(|record| PolledRecord {
                record,
                action: PollAction::Deliver,
            })
            .collect(),
        fetched_at: Utc::now(),
        final_url: url::Url::parse("https://bootstrap.example/feed").unwrap(),
        validators: Default::default(),
        incomplete,
        duration_ms: 1,
    };
    store
        .commit_poll(&lease, commit(&records[..30], &records[30..], true))
        .await
        .unwrap();
    let first = deliver(&store, &lease, &workspace, &sub, records[99].clone()).await;
    assert!(
        !allowed(pool, owner.as_uuid(), workspace.id(), first).await,
        "one delivered row must not spend before initial collection completes"
    );
    while let Some(batch) = store.pending_poll(source, 17).await.unwrap() {
        store.commit_poll(&lease, batch).await.unwrap();
    }
    deliver(&store, &lease, &workspace, &sub, records[100].clone()).await;
    let mut ids = vec![first];
    for record in records.iter().take(99).rev() {
        ids.push(deliver(&store, &lease, &workspace, &sub, record.clone()).await);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM articles WHERE workspace_key=$1")
        .bind(workspace.id().as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 100, "the entire archive is retained");
    let selected: Vec<String> = sqlx::query_scalar("SELECT r.document::jsonb->>'upstream_id' FROM subscription_ai_selected s JOIN source_records r ON r.id=s.record WHERE s.subscription=$1 ORDER BY 1")
        .bind(sub.id().as_uuid().to_string()).fetch_all(pool).await.unwrap();
    assert_eq!(
        selected,
        (90..100)
            .map(|n| format!("item-{n:03}"))
            .collect::<Vec<_>>()
    );
    let eligible: i64 = sqlx::query_scalar("SELECT count(*) FROM articles a WHERE a.workspace_key=$2 AND reader_ai_automatic_allowed($1,$2,a.article_key)").bind(owner.as_uuid()).bind(workspace.id().as_uuid().to_string()).fetch_one(pool).await.unwrap();
    assert_eq!(eligible, 10);
    assert!(
        !allowed(pool, Uuid::new_v4(), workspace.id(), ids[0]).await,
        "guessed article IDs never cross owners"
    );
    // Repeated poll, delayed delivery and restart do not refill the allowance.
    store
        .commit_poll(&lease, commit(&records, &[], false))
        .await
        .unwrap();
    deliver(&store, &lease, &workspace, &sub, records[1].clone()).await;
    assert_eq!(selected.len(), 10);
    let latest = record(source, 100);
    store
        .commit_poll(&lease, commit(std::slice::from_ref(&latest), &[], false))
        .await
        .unwrap();
    let live = deliver(&store, &lease, &workspace, &sub, latest).await;
    assert!(allowed(pool, owner.as_uuid(), workspace.id(), live).await);
    // Native candidates use the same admission function, including terms/ranking.
    sqlx::query("INSERT INTO ai_profiles(owner,encrypted_key,balance) VALUES($1,$2,$3)")
        .bind(owner.as_uuid())
        .bind(vec![1u8])
        .bind(
            serde_json::to_string(&reader_ai::Balance {
                available: true,
                balances: vec![],
                updated_at: Utc::now(),
            })
            .unwrap(),
        )
        .execute(pool)
        .await
        .unwrap();
    ai.enroll_summaries(&[owner.as_uuid()]).await.unwrap();
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_summary_queue WHERE owner=$1")
        .bind(owner.as_uuid())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(queued, 11);
    sqlx::query("INSERT INTO interest_profiles(owner,prompt,revision,training_count) VALUES($1,'Databases',1,0)").bind(owner.as_uuid()).execute(pool).await.unwrap();
    let claim = ai
        .claim_interest(&[owner.as_uuid()], 60)
        .await
        .unwrap()
        .unwrap();
    assert!(
        allowed(
            pool,
            owner.as_uuid(),
            workspace.id(),
            ArticleId::from_uuid(claim.article)
        )
        .await
    );
    // Revoking eligibility after the claim must stop the reservation itself.
    sqlx::query("UPDATE subscription_ai_bootstrap SET ready=false WHERE subscription=$1")
        .bind(sub.id().as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
    assert!(matches!(
        ai.reserve(
            owner.as_uuid(),
            claim.lease,
            &SpendReservation::new("0.1".into(), "3".into(), SpendMode::Ranking).unwrap()
        )
        .await,
        Err(AiError::AutomaticExcluded)
    ));
    ai.finish_interest(&claim, Err(AiError::AutomaticExcluded), None)
        .await
        .unwrap();
    sqlx::query("UPDATE subscription_ai_bootstrap SET ready=true WHERE subscription=$1")
        .bind(sub.id().as_uuid().to_string())
        .execute(pool)
        .await
        .unwrap();
    ai.configure_initial_articles(5).await.unwrap();
    let archived = *ids.last().unwrap();
    assert!(!allowed(pool, owner.as_uuid(), workspace.id(), archived).await);
    // A late fulltext must not admit an archived article to terms or summary.
    for (record, admitted) in [
        (&records[0], false),
        (
            &store
                .record_by_upstream(source, "item-100")
                .await
                .unwrap()
                .unwrap(),
            true,
        ),
    ] {
        let pointer = reader_ingest::ContentManifestPointer {
            reddit_flair: None,
            video: None,
            publication: None,
            record_id: record.id(),
            source_revision: 0,
            refresh_id: Uuid::new_v4(),
            raw_chunks: 0,
            safe_html_chunks: 0,
            fetched_at: Utc::now(),
            final_url: url::Url::parse("https://bootstrap.example/fulltext").unwrap(),
        };
        sqlx::query("INSERT INTO content_manifests(id,revision,document) VALUES($1,0,$2)")
            .bind(record.id().as_uuid().to_string())
            .bind(serde_json::to_string(&pointer).unwrap())
            .execute(pool)
            .await
            .unwrap();
        let terms = ai
            .next_terms(&[owner.as_uuid()], "bootstrap-terms")
            .await
            .unwrap();
        assert_eq!(terms.is_some(), admitted);
        if let Some((_, _, article)) = terms {
            assert_eq!(article, live.as_uuid());
        }
    }
    let chat_record = ai_tests::record(
        owner.as_uuid(),
        workspace.id().as_uuid(),
        archived.as_uuid(),
        Uuid::new_v4(),
    );
    let chat = ai
        .create_chat(
            chat_record.clone(),
            chat_record.operations[0].id,
            false,
            true,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
    assert!(ai.claim(60).await.unwrap().is_none());
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    let auto = ai.claim(60).await.unwrap().unwrap();
    assert_eq!(auto.record.view.id, chat.view.id);
    let reservation = SpendReservation::new("0.1".into(), "3".into(), SpendMode::Summary).unwrap();
    let model = reader_ai::CallModel::new(
        reader_ai::DeepSeekModel::Flash,
        chat_record.cost_rates.clone(),
    );
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
    assert!(matches!(
        ai.begin_call(
            &auto,
            reader_ai::GenerationPhase::Generating,
            &reservation,
            &model
        )
        .await,
        Err(AiError::AutomaticPaused)
    ));
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    assert!(matches!(
        ai.begin_call(
            &auto,
            reader_ai::GenerationPhase::Generating,
            &reservation,
            &model
        )
        .await,
        Err(AiError::AutomaticExcluded)
    ));
    ai.stop(owner.as_uuid(), chat.view.id).await.unwrap();
    let mut manual = ai_tests::record(
        owner.as_uuid(),
        workspace.id().as_uuid(),
        archived.as_uuid(),
        Uuid::new_v4(),
    );
    manual.operations[0].kind = reader_ai::OperationKind::Start { regenerate: true };
    ai.create_chat(manual.clone(), manual.operations[0].id, true, false)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
    let claim = ai.claim(60).await.unwrap().unwrap();
    let call = ai
        .begin_call(
            &claim,
            reader_ai::GenerationPhase::Generating,
            &reservation,
            &model,
        )
        .await
        .unwrap();
    ai.stop(owner.as_uuid(), claim.record.view.id)
        .await
        .unwrap();
    ai.settle(owner.as_uuid(), call, "0").await.unwrap();
    let policy = ai_tests::policy(owner.as_uuid());
    let input =
        reader_ai::DefinitionsInput::new(policy.config(), chat_record.snapshot.unwrap()).unwrap();
    let definition = reader_ai::DefinitionsRecord::new(
        owner.as_uuid(),
        workspace.id().as_uuid(),
        archived.as_uuid(),
        input.clone(),
        policy.cost_rates().clone(),
    )
    .unwrap();
    ai.create_definitions(definition, Uuid::new_v4(), false, true)
        .await
        .unwrap();
    assert!(ai.claim_definitions(60).await.unwrap().is_none());
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    let auto = ai.claim_definitions(60).await.unwrap().unwrap();
    let reservation = SpendReservation::new("0.1".into(), "3".into(), SpendMode::Terms).unwrap();
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
    assert!(matches!(
        ai.reserve(owner.as_uuid(), auto.lease, &reservation).await,
        Err(AiError::AutomaticPaused)
    ));
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    assert!(matches!(
        ai.reserve(owner.as_uuid(), auto.lease, &reservation).await,
        Err(AiError::AutomaticExcluded)
    ));
    ai.finish_definitions(
        &auto,
        reader_ai::DefinitionState::Failed {
            error: AiError::AutomaticExcluded.to_string(),
        },
        None,
    )
    .await
    .unwrap();
    let definition = reader_ai::DefinitionsRecord::new(
        owner.as_uuid(),
        workspace.id().as_uuid(),
        archived.as_uuid(),
        input,
        policy.cost_rates().clone(),
    )
    .unwrap();
    ai.create_definitions(definition, Uuid::new_v4(), true, false)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
    let manual = ai.claim_definitions(60).await.unwrap().unwrap();
    ai.reserve(owner.as_uuid(), manual.lease, &reservation)
        .await
        .unwrap();
    ai.settle(owner.as_uuid(), manual.lease, "0").await.unwrap();
    ai.finish_definitions(
        &manual,
        reader_ai::DefinitionState::Failed {
            error: "No provider called in admission test".into(),
        },
        None,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    // A second account subscribing to the cached public URL gets its own snapshot.
    let other = Workspace::new(
        WorkspaceId::new(),
        AccountId::new(),
        "Other subscriber".into(),
    );
    reader.save_workspace(None, other.clone()).await.unwrap();
    let other_sub = Subscription::new(
        SubscriptionId::new(),
        other.id(),
        sub.source_url().clone(),
        "Shared source".into(),
    );
    reader
        .save_subscription(None, other_sub.clone())
        .await
        .unwrap();
    store
        .record_source_success(&lease, source, Utc::now(), false, 1)
        .await
        .unwrap();
    let latest_record = store
        .record_by_upstream(source, "item-100")
        .await
        .unwrap()
        .unwrap();
    for record in records
        .iter()
        .skip(1)
        .rev()
        .chain(std::iter::once(&latest_record))
    {
        // record-100 must retain the exact stored identity, not a new fixture UUID.
        let current = store
            .record_by_upstream(source, record.upstream_id())
            .await
            .unwrap()
            .unwrap();
        deliver(&store, &lease, &other, &other_sub, current).await;
    }
    let other_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subscription_ai_selected WHERE subscription=$1")
            .bind(other_sub.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(other_count, 5);
    ai.configure_initial_articles(10).await.unwrap();
    let own_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subscription_ai_selected WHERE subscription=$1")
            .bind(sub.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        own_count, 10,
        "the second subscription cannot change the first"
    );
    assert!(!allowed(pool, other.owner().as_uuid(), workspace.id(), live).await);
    // Replacing the source preserves the old selection, starts a new initial
    // allowance and uses the exact URL reservation rather than an invalid NUL key.
    let replacement_url = url::Url::parse("https://bootstrap.example/replaced-feed").unwrap();
    let mut replacement = sub.clone();
    replacement
        .replace_source(
            replacement_url.clone(),
            replacement_url.to_string(),
            "Replacement".into(),
        )
        .unwrap();
    reader
        .replace_subscription_source(sub.revision(), replacement.clone())
        .await
        .unwrap();
    let states: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subscription_ai_bootstrap WHERE subscription=$1")
            .bind(sub.id().as_uuid().to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(states, 2);
    assert!(!allowed(pool, owner.as_uuid(), workspace.id(), live).await);
    let mut restored = replacement.clone();
    restored
        .replace_source(
            sub.source_url().clone(),
            sub.source_url_exact().to_owned(),
            sub.source_title().into(),
        )
        .unwrap();
    reader
        .replace_subscription_source(replacement.revision(), restored)
        .await
        .unwrap();
    assert!(allowed(pool, owner.as_uuid(), workspace.id(), live).await);
    for size in [0, 6] {
        let small = Subscription::new(
            SubscriptionId::new(),
            workspace.id(),
            url::Url::parse(&format!("https://bootstrap.example/small/{size}")).unwrap(),
            "Small".into(),
        );
        reader.save_subscription(None, small.clone()).await.unwrap();
        let source_text: String = sqlx::query_scalar(
            "SELECT source_id FROM subscription_sources WHERE subscription_id=$1",
        )
        .bind(small.id().as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
        let small_source = SourceId::from_uuid(Uuid::parse_str(&source_text).unwrap());
        let item = WorkItem::RefreshSource {
            source_id: small_source,
        };
        let small_lease = LeasedWork {
            job_id: JobId::new(),
            item: item.clone(),
            token: LeaseToken::new(),
            deadline: Utc::now() + ChronoDuration::minutes(10),
            attempt: 0,
        };
        sqlx::query("INSERT INTO ingest_jobs(id,status,run_at_ms,first_attempt_ms,origin_key,attempt,item,revision,lease_token,lease_deadline_ms) VALUES($1,'leased',0,$2,'https://bootstrap.example',0,$3,0,$4,$5)").bind(small_lease.job_id.as_uuid().to_string()).bind(Utc::now().timestamp_millis()).bind(serde_json::to_string(&item).unwrap()).bind(small_lease.token.as_uuid().to_string()).bind(small_lease.deadline.timestamp_millis()).execute(pool).await.unwrap();
        let records: Vec<SourceRecord> = (0..size)
            .map(|n| {
                let mut value = serde_json::to_value(record(small_source, n)).unwrap();
                value["published_at"] = serde_json::Value::Null;
                serde_json::from_value(value).unwrap()
            })
            .collect();
        store
            .commit_poll(
                &small_lease,
                PollCommit {
                    source_id: small_source,
                    source_revision: 0,
                    records: records
                        .iter()
                        .cloned()
                        .map(|record| PolledRecord {
                            record,
                            action: PollAction::Deliver,
                        })
                        .collect(),
                    remainder: vec![],
                    fetched_at: Utc::now(),
                    final_url: small.source_url().clone(),
                    validators: Default::default(),
                    incomplete: false,
                    duration_ms: 1,
                },
            )
            .await
            .unwrap();
        for record in records {
            deliver(&store, &small_lease, &workspace, &small, record).await;
        }
        let (ready,count):(bool,i64)=sqlx::query_as("SELECT b.ready,(SELECT count(*) FROM subscription_ai_selected s WHERE s.subscription=b.subscription AND s.source=b.source) FROM subscription_ai_bootstrap b WHERE b.subscription=$1").bind(small.id().as_uuid().to_string()).fetch_one(pool).await.unwrap();
        assert!(ready);
        assert_eq!(count, size as i64);
    }
}
fn record(source: SourceId, index: usize) -> SourceRecord {
    let date = chrono::DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z").unwrap()
        + ChronoDuration::hours(index as i64);
    SourceRecord::from_parsed(
        reader_core::SourceRecordId::new(),
        source,
        reader_collectors::ParsedRecord {
            upstream_id: format!("item-{index:03}"),
            original_url: format!("https://bootstrap.example/articles/{index}"),
            absolute_url: Some(
                url::Url::parse(&format!("https://bootstrap.example/articles/{index}")).unwrap(),
            ),
            title: format!("Article {index}"),
            description: None,
            categories: None,
            description_media_type: None,
            content_html: None,
            published_at: Some(date.with_timezone(&Utc).into()),
        },
    )
    .unwrap()
}
async fn deliver(
    store: &PostgresIngestStore,
    lease: &LeasedWork,
    workspace: &Workspace,
    sub: &Subscription,
    record: SourceRecord,
) -> ArticleId {
    match store
        .deliver(
            lease,
            DeliveryCommit {
                target: DeliveryTarget {
                    workspace_id: workspace.id(),
                    subscription_id: sub.id(),
                },
                record,
                proposed_article_id: ArticleId::new(),
                delivered_at: Utc::now(),
            },
        )
        .await
        .unwrap()
    {
        reader_ingest::DeliveryResult::Delivered(id)
        | reader_ingest::DeliveryResult::AlreadyDelivered(id) => id,
        other => panic!("unexpected {other:?}"),
    }
}
async fn allowed(pool: &PgPool, owner: Uuid, workspace: WorkspaceId, article: ArticleId) -> bool {
    sqlx::query_scalar("SELECT reader_ai_automatic_allowed($1,$2,$3)")
        .bind(owner)
        .bind(workspace.as_uuid().to_string())
        .bind(article.as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap()
}
