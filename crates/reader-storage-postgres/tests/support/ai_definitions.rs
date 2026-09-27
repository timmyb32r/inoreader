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
        store.create_definitions(make(), operation, false),
        store.create_definitions(make(), Uuid::new_v4(), false)
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
            .create_definitions(make(), operation, false)
            .await
            .unwrap()
            .id,
        job.id
    );
    let invalid=json!({"entities":[{"name":"Kafka","kind":"product", "explanation":"Не из статьи", "insufficientContext":false}]}).to_string();
    assert!(DefinitionResult::from_response(&snapshot, &invalid).is_err());
    let mut wire = serde_json::to_value(make()).unwrap();
    wire["job"]["model"] = json!("different");
    assert!(serde_json::from_value::<DefinitionsRecord>(wire).is_err());
    let mut bad_input = serde_json::to_value(&input).unwrap();
    bad_input["limits"]["max_input_bytes"] = json!(1);
    assert!(serde_json::from_value::<DefinitionsInput>(bad_input).is_err());
    assert!(store
        .create_definitions(make(), operation, true)
        .await
        .is_err());
    let refreshed = store
        .create_definitions(make(), Uuid::new_v4(), true)
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
    let mut next = snapshot;
    next.source_revision = "updated-source".into();
    let record = DefinitionsRecord::new(
        owner,
        workspace,
        article,
        DefinitionsInput::new(policy.config(), next).unwrap(),
        policy.cost_rates().clone(),
    )
    .unwrap();
    let failed = store
        .create_definitions(record, Uuid::new_v4(), false)
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
}
