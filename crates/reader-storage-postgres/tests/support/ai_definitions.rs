use super::*;
use serde_json::json;

pub async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    other: Uuid,
    workspace: Uuid,
    article: Uuid,
) {
    let snapshot = ArticleSnapshot {
        title: "CDC".into(),
        source_url: "https://example.com/cdc".into(),
        safe_html: "<p>CDC captures changes.</p>".into(),
        text: "CDC captures changes.".into(),
        source_revision: "definitions-test".into(),
    };
    let policy = policy(owner);
    let input = DefinitionsInput::new(policy.config(), snapshot.clone()).unwrap();
    let make = || {
        DefinitionsRecord::new(
            owner,
            workspace,
            article,
            input.clone(),
            policy.cost_rates().clone(),
        )
        .unwrap()
    };
    let operation = Uuid::new_v4();
    let (a, b) = tokio::join!(
        store.create_definitions(make(), operation, false, false),
        store.create_definitions(make(), Uuid::new_v4(), false, false)
    );
    let job = a.unwrap();
    assert_eq!(job.id, b.unwrap().id);
    assert!(store.definitions(other, workspace, article).await.is_err());
    let claim = store.claim_definitions(60).await.unwrap().unwrap();
    assert!(store.claim_definitions(60).await.unwrap().is_none());
    let raw=json!({"choices":[{"finish_reason":"stop","message":{"content":json!({"entities":[{"name":"CDC","kind":"abbreviation","explanation":"Change Data Capture — получение изменений данных.","insufficientContext":false}]}).to_string()}}],"usage":{"prompt_tokens":10,"completion_tokens":8,"prompt_cache_hit_tokens":2,"prompt_cache_miss_tokens":8}}).to_string().into_bytes();
    let reply = ProviderReply {
        status: 200,
        body: raw.clone(),
        interrupted: false,
    };
    store
        .retain_reply(ReplyKind::Definitions, owner, job.id, &reply)
        .await
        .unwrap();
    let result = reply.definition_result(&snapshot).unwrap();
    store
        .finish_definitions(&claim, DefinitionState::Completed { result }, reply.usage())
        .await
        .unwrap();
    assert!(matches!(
        store
            .definitions(owner, workspace, article)
            .await
            .unwrap()
            .unwrap()
            .state,
        DefinitionState::Completed { .. }
    ));
    let retained: Vec<u8> =
        sqlx::query_scalar("SELECT raw_response FROM ai_definitions WHERE id=$1")
            .bind(job.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(retained, raw);
    assert_eq!(
        store
            .create_definitions(make(), operation, false, false)
            .await
            .unwrap()
            .id,
        job.id
    );
    // Automatic work is admitted once even when the source text changes.
    let mut updated = snapshot.clone();
    updated.source_revision = "refresh/new".into();
    updated.text.push_str(" Changed source content.");
    let auto = DefinitionsRecord::new(
        owner,
        workspace,
        article,
        DefinitionsInput::new(policy.config(), updated).unwrap(),
        policy.cost_rates().clone(),
    )
    .unwrap();
    assert_eq!(
        store
            .create_definitions(auto.clone(), Uuid::new_v4(), false, true)
            .await
            .unwrap()
            .id,
        job.id
    );
    // Simulate a duplicate left by the old scheduler. Cancel it while preserving
    // its exact snapshot and the completed result visible to the reader.
    sqlx::query("INSERT INTO ai_definitions(id,owner,workspace,article,status,document,input,automatic) VALUES($1,$2,$3,$4,'queued',$5,$6,true)")
        .bind(auto.job().id).bind(owner).bind(workspace).bind(article).bind(serde_json::to_string(&auto).unwrap()).bind(serde_json::to_string(auto.input()).unwrap()).execute(pool).await.unwrap();
    assert!(store.claim_definitions(60).await.unwrap().is_none());
    let cancelled: (String, String) =
        sqlx::query_as("SELECT status,input FROM ai_definitions WHERE id=$1")
            .bind(auto.job().id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(cancelled.0, "cancelled");
    assert_eq!(cancelled.1, serde_json::to_string(auto.input()).unwrap());
    assert_eq!(
        store
            .definitions(owner, workspace, article)
            .await
            .unwrap()
            .unwrap()
            .id,
        job.id
    );
    // A manual attempt arriving after an automatic lease closes the last paid
    // admission race; cancellation must not create a spending reservation.
    let race_article = Uuid::new_v4();
    let race = DefinitionsRecord::new(
        owner,
        workspace,
        race_article,
        input.clone(),
        policy.cost_rates().clone(),
    )
    .unwrap();
    store
        .create_definitions(race, Uuid::new_v4(), false, true)
        .await
        .unwrap();
    let auto_claim = store.claim_definitions(60).await.unwrap().unwrap();
    let mut changed = snapshot.clone();
    changed.text.push_str(" Manual revised content.");
    let manual = DefinitionsRecord::new(
        owner,
        workspace,
        race_article,
        DefinitionsInput::new(policy.config(), changed).unwrap(),
        policy.cost_rates().clone(),
    )
    .unwrap();
    store
        .create_definitions(manual, Uuid::new_v4(), true, false)
        .await
        .unwrap();
    let manual_claim = store.claim_definitions(60).await.unwrap().unwrap();
    store
        .finish_definitions(
            &manual_claim,
            DefinitionState::Failed {
                error: "manual fixture".into(),
            },
            None,
        )
        .await
        .unwrap();
    let reservation = SpendReservation::new("0.1".into(), "3".into(), SpendMode::Terms).unwrap();
    assert!(matches!(
        store.reserve(owner, auto_claim.lease, &reservation).await,
        Err(AiError::AutomaticDuplicate)
    ));
    let paid: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_spending WHERE id=$1")
        .bind(auto_claim.lease)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(paid, 0);
    store
        .finish_definitions(
            &auto_claim,
            DefinitionState::Cancelled {
                reason: AiError::AutomaticDuplicate.to_string(),
            },
            None,
        )
        .await
        .unwrap();
    let invalid=json!({"entities":[{"name":"Kafka","kind":"product", "explanation":"Не из статьи", "insufficientContext":false}]}).to_string();
    assert!(DefinitionResult::from_response(&snapshot, &invalid).is_err());
    let mut wire = serde_json::to_value(make()).unwrap();
    wire["job"]["model"] = json!("different");
    assert!(serde_json::from_value::<DefinitionsRecord>(wire).is_err());
    let mut bad_input = serde_json::to_value(&input).unwrap();
    bad_input["limits"]["max_input_bytes"] = json!(1);
    assert!(serde_json::from_value::<DefinitionsInput>(bad_input).is_err());
    assert!(store
        .create_definitions(make(), operation, true, false)
        .await
        .is_err());
    let refreshed = store
        .create_definitions(make(), Uuid::new_v4(), true, false)
        .await
        .unwrap();
    assert_ne!(refreshed.id, job.id);
    let refreshed_claim = store.claim_definitions(60).await.unwrap().unwrap();
    store
        .finish_definitions(
            &refreshed_claim,
            DefinitionState::Failed {
                error: "explicit attempt".into(),
            },
            None,
        )
        .await
        .unwrap();
    let mut next = snapshot.clone();
    next.source_revision = "updated-source".into();
    let identical = DefinitionsInput::new(policy.config(), next.clone()).unwrap();
    assert!(input.same_request(&identical).unwrap());
    let reused = store
        .create_definitions(
            DefinitionsRecord::new(
                owner,
                workspace,
                article,
                identical,
                policy.cost_rates().clone(),
            )
            .unwrap(),
            Uuid::new_v4(),
            false,
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        reused.id, job.id,
        "fetch revision must not reset the paid request cache"
    );
    next.text.push_str(" New content.");
    let record = DefinitionsRecord::new(
        owner,
        workspace,
        article,
        DefinitionsInput::new(policy.config(), next).unwrap(),
        policy.cost_rates().clone(),
    )
    .unwrap();
    let failed = store
        .create_definitions(record, Uuid::new_v4(), false, false)
        .await
        .unwrap();
    assert_ne!(failed.id, job.id);
    let claim = store.claim_definitions(60).await.unwrap().unwrap();
    sqlx::query("UPDATE ai_definitions SET lease_until=now()-interval '1 second' WHERE id=$1")
        .bind(failed.id)
        .execute(pool)
        .await
        .unwrap();
    store
        .retain_reply(
            ReplyKind::Definitions,
            owner,
            failed.id,
            &ProviderReply {
                status: 200,
                body: b"partial raw".to_vec(),
                interrupted: true,
            },
        )
        .await
        .unwrap();
    assert!(store
        .finish_definitions(
            &claim,
            DefinitionState::Failed {
                error: "invalid JSON".into()
            },
            None
        )
        .await
        .is_err());
    assert!(store.claim_definitions(60).await.unwrap().is_none());
    assert!(matches!(
        store
            .definitions(owner, workspace, article)
            .await
            .unwrap()
            .unwrap()
            .state,
        DefinitionState::Failed { .. }
    ));
    let retained: Vec<u8> =
        sqlx::query_scalar("SELECT raw_response FROM ai_definitions WHERE id=$1")
            .bind(failed.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(retained, b"partial raw");
    super::super::ai_bootstrap::admit_fixture(
        pool,
        &PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap(),
        WorkspaceId::from_uuid(workspace),
        ArticleId::from_uuid(article),
    )
    .await;
    // One invalid context cannot repeatedly occupy the front of the scheduler.
    let (o, w, a) = store
        .next_terms(&[owner], "scheduler-fixture")
        .await
        .unwrap()
        .expect("fixture must retain a full-text candidate");
    store
        .reject_terms(o, w, a, "scheduler-fixture", "context_unavailable")
        .await
        .unwrap();
    assert_ne!(
        store
            .next_terms(&[owner], "scheduler-fixture")
            .await
            .unwrap(),
        Some((o, w, a))
    );
    for expired in [false, true] {
        let bad = Uuid::new_v4();
        sqlx::query("INSERT INTO ai_definitions(id,owner,workspace,article,status,document,input,created_at,lease,lease_until) VALUES($1,$2,$3,$4,$5,'broken original','exact input',now()-interval '1 day',$6,$7)")
            .bind(bad).bind(owner).bind(workspace).bind(article).bind(if expired { "generating" } else { "queued" })
            .bind(expired.then(Uuid::new_v4)).bind(expired.then(|| Utc::now()-chrono::Duration::minutes(1)))
            .execute(pool).await.unwrap();
        assert!(store.claim_definitions(60).await.unwrap().is_none());
        let row: (String, String) =
            sqlx::query_as("SELECT status,document FROM ai_definitions WHERE id=$1")
                .bind(bad)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(row, ("quarantined".into(), "broken original".into()));
    }
    let fresh = store
        .create_definitions(make(), Uuid::new_v4(), true, false)
        .await
        .unwrap();
    let claim = store.claim_definitions(60).await.unwrap().unwrap();
    assert_eq!(claim.record.job().id, fresh.id);
    // Budget exhaustion is replay-safe: no provider request is made, and the
    // retained job becomes eligible at the next Moscow budget day.
    let deferred = fresh;
    store
        .defer_terms(&claim, reader_ai::AiDeferral::DailyBudget)
        .await
        .unwrap();
    assert!(store.claim_definitions(60).await.unwrap().is_none());
    let state: (String, bool) =
        sqlx::query_as("SELECT status,scheduled_at>now() FROM ai_definitions WHERE id=$1")
            .bind(deferred.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(state, ("queued".into(), true));
    assert!(store
        .defer_terms(&claim, reader_ai::AiDeferral::DailyBudget)
        .await
        .is_err());
    assert!(
        store
            .next_terms(&[owner], "after-quarantine-fixture")
            .await
            .unwrap()
            .is_none(),
        "quarantined input must never automatically issue another paid request"
    );
}
