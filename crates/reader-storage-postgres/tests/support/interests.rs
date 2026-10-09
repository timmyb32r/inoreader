use super::*;
use reader_ai::{AiError, InterestPrediction, InterestProfile, InterestStore};
use std::sync::Arc;
pub async fn verify(pool: &PgPool) {
    let reader = Arc::new(
        PostgresRepository::new(pool.clone(), ReasonPolicy::new(256).unwrap(), 20, 86400).unwrap(),
    );
    let store = reader_storage_postgres::PostgresAiStore::new(
        pool.clone(),
        reader.clone(),
        std::num::NonZeroU32::new(20).unwrap(),
    );
    let mut owners = Vec::new();
    for index in 0..2 {
        let account = AccountRecord {
            id: AccountId::new(),
            username: Uuid::new_v4().to_string(),
            password_hash: "fixture".into(),
            admin: false,
            auth_revision: 0,
            revision: 0,
        };
        let workspace =
            Workspace::new(WorkspaceId::new(), account.id, format!("Interests {index}"));
        reader
            .create_account_and_workspace(account.clone(), workspace.clone())
            .await
            .unwrap();
        owners.push((account.id.as_uuid(), workspace.id()));
    }
    let profile = InterestProfile {
        prompt: "Prefer database research; dislike marketing".into(),
        revision: "1".into(),
        training_count: 2,
    };
    sqlx::query(
        "INSERT INTO interest_profiles(owner,prompt,revision,training_count) VALUES($1,$2,1,2)",
    )
    .bind(owners[0].0)
    .bind(&profile.prompt)
    .execute(pool)
    .await
    .unwrap();
    let mut ids = Vec::new();
    for n in 0..3 {
        let article = Article {
            id: ArticleId::new(),
            key: DedupKey {
                location: url::Url::parse(&format!("https://example.test/shared-interests/{n}"))
                    .unwrap()
                    .into(),
                title: format!("Article {n}"),
                description: Some("Original content".into()),
            },
            state: ArticleState::default(),
            first_arrived_at: Utc::now() + ChronoDuration::seconds(n),
            origins: vec![],
            revision: 0,
        };
        ids.push(article.id);
        for (_, workspace) in &owners {
            reader
                .save_article(*workspace, None, article.clone())
                .await
                .unwrap();
        }
    }
    for id in &ids {
        super::ai_bootstrap::admit_fixture(pool, &reader, owners[0].1, *id).await;
    }
    assert!(matches!(
        store
            .feed(owners[1].0, owners[0].1.as_uuid(), None, 10, false)
            .await,
        Err(AiError::NotFound)
    ));
    let own = store
        .feed(owners[0].0, owners[0].1.as_uuid(), None, 10, false)
        .await
        .unwrap();
    assert_eq!(own.total, 3);
    assert_eq!(own.scored, 0);
    let other = store
        .feed(owners[1].0, owners[1].1.as_uuid(), None, 10, false)
        .await
        .unwrap();
    assert!(other.profile.is_none());
    assert!(other.articles.iter().all(|a| a.prediction.is_none()));
    assert!(store
        .claim_interest(&[owners[1].0], 60)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
    assert!(store
        .claim_interest(&[owners[0].0], 60)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .claim_interest(&[owners[1].0], 60)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
        .execute(pool)
        .await
        .unwrap();
    for expected in [2, 1, 0] {
        let claim = store
            .claim_interest(&[owners[0].0], 60)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claim.article, ids[expected].as_uuid());
        if expected == 2 {
            sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=true,calendar_valid_through='2000-01-01'").execute(pool).await.unwrap();
            let reservation = reader_ai::SpendReservation::new(
                "0.1".into(),
                "3".into(),
                reader_ai::SpendMode::Ranking,
            )
            .unwrap();
            assert!(matches!(
                reader_ai::AiStore::reserve(&store, claim.owner, claim.lease, &reservation).await,
                Err(AiError::AutomaticPaused)
            ));
            let reserved: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_spending WHERE id=$1")
                .bind(claim.lease)
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(reserved, 0);
            sqlx::query("UPDATE ai_automatic_schedule SET pause_peak_hours=false")
                .execute(pool)
                .await
                .unwrap();
        }
        let prediction:InterestPrediction=serde_json::from_value(serde_json::json!({"score":if expected==0 {9}else if expected==1 {1}else{2},"reason":"Test evidence","confidence":"high"})).unwrap();
        let input = reader_ai::InterestInput::new(
            super::ai_tests::policy(claim.owner).config(),
            &claim.profile.prompt,
            &claim.title,
            "Unchanged classifier text",
        )
        .unwrap();
        assert!(store
            .retain_interest_input(&claim, &input)
            .await
            .unwrap()
            .is_none());
        let reply = reader_ai::ProviderReply { status: 200, interrupted: false, body: serde_json::to_vec(&serde_json::json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::to_string(&prediction).unwrap()}}]})).unwrap() };
        store
            .finish_interest(&claim, Ok(prediction), Some(&reply))
            .await
            .unwrap();
        assert!(matches!(
            store
                .finish_interest(&claim, Err(AiError::Provider), None)
                .await,
            Err(AiError::Storage) | Err(AiError::Conflict)
        ));
    }
    // Source metadata invalidates a score, but an identical complete classifier
    // input reuses the paid result before any new budget reservation.
    sqlx::query("UPDATE interest_scores SET dirty=true WHERE owner=$1 AND article=$2")
        .bind(owners[0].0)
        .bind(ids[2].as_uuid())
        .execute(pool)
        .await
        .unwrap();
    let claim = store
        .claim_interest(&[owners[0].0], 60)
        .await
        .unwrap()
        .unwrap();
    let config = super::ai_tests::policy(claim.owner);
    let input = reader_ai::InterestInput::new(
        config.config(),
        &claim.profile.prompt,
        &claim.title,
        "Unchanged classifier text",
    )
    .unwrap();
    let cached = store
        .retain_interest_input(&claim, &input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cached.score(), 2);
    let mut foreign = reader_ai::InterestClaim {
        owner: claim.owner,
        workspace: claim.workspace,
        article: claim.article,
        profile: claim.profile.clone(),
        lease: claim.lease,
        title: claim.title.clone(),
        description: claim.description.clone(),
    };
    foreign.owner = owners[1].0;
    assert!(matches!(
        store.retain_interest_input(&foreign, &input).await,
        Err(AiError::NotFound)
    ));
    let mut revised = reader_ai::InterestClaim {
        owner: claim.owner,
        workspace: claim.workspace,
        article: claim.article,
        profile: claim.profile.clone(),
        lease: claim.lease,
        title: claim.title.clone(),
        description: claim.description.clone(),
    };
    revised.profile.revision = "2".into();
    assert!(matches!(
        store.retain_interest_input(&revised, &input).await,
        Err(AiError::Conflict)
    ));
    store
        .finish_interest(&claim, Ok(cached), None)
        .await
        .unwrap();
    sqlx::query("UPDATE interest_scores SET dirty=true WHERE owner=$1 AND article=$2")
        .bind(owners[0].0)
        .bind(ids[2].as_uuid())
        .execute(pool)
        .await
        .unwrap();
    let claim = store
        .claim_interest(&[owners[0].0], 60)
        .await
        .unwrap()
        .unwrap();
    let changed = reader_ai::InterestInput::new(
        config.config(),
        &claim.profile.prompt,
        &claim.title,
        "Changed classifier text",
    )
    .unwrap();
    assert!(store
        .retain_interest_input(&claim, &changed)
        .await
        .unwrap()
        .is_none());
    store
        .finish_interest(&claim, Err(AiError::Provider), None)
        .await
        .unwrap();
    sqlx::query("UPDATE interest_scores SET status='scored',score=2,prediction=$3 WHERE owner=$1 AND article=$2")
        .bind(owners[0].0).bind(ids[2].as_uuid()).bind(serde_json::json!({"score":2,"reason":"Test evidence","confidence":"high"}).to_string()).execute(pool).await.unwrap();
    let visible = store
        .feed(owners[0].0, owners[0].1.as_uuid(), None, 10, false)
        .await
        .unwrap();
    assert!(!visible.articles.iter().any(|a| a.id == ids[1].as_uuid()));
    let hidden = store
        .feed(owners[0].0, owners[0].1.as_uuid(), None, 10, true)
        .await
        .unwrap();
    assert!(hidden.articles.iter().any(|a| a.id == ids[1].as_uuid()));
    let seed = Uuid::new_v4();
    let random = store
        .random_feed(owners[0].0, owners[0].1.as_uuid(), None, 10, seed)
        .await
        .unwrap();
    let again = store
        .random_feed(owners[0].0, owners[0].1.as_uuid(), None, 10, seed)
        .await
        .unwrap();
    assert_eq!(
        random
            .articles
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        again
            .articles
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(random.articles.len(), 2);
    assert!(!random
        .articles
        .iter()
        .any(|item| item.id == ids[1].as_uuid()));
    let random_first = store
        .random_feed(owners[0].0, owners[0].1.as_uuid(), None, 1, seed)
        .await
        .unwrap();
    let random_next = store
        .random_feed(
            owners[0].0,
            owners[0].1.as_uuid(),
            random_first.next_cursor.as_deref(),
            1,
            seed,
        )
        .await
        .unwrap();
    assert_ne!(random_first.articles[0].id, random_next.articles[0].id);
    assert!(random_next.next_cursor.is_none());
    assert!(matches!(
        store
            .random_feed(
                owners[0].0,
                owners[0].1.as_uuid(),
                random_first.next_cursor.as_deref(),
                1,
                Uuid::new_v4()
            )
            .await,
        Err(AiError::Conflict)
    ));
    assert!(matches!(
        store
            .random_feed(owners[1].0, owners[0].1.as_uuid(), None, 10, seed)
            .await,
        Err(AiError::NotFound)
    ));
    let random_other = store
        .random_feed(owners[1].0, owners[1].1.as_uuid(), None, 10, seed)
        .await
        .unwrap();
    assert_eq!(
        random_other.articles.len(),
        3,
        "shared article IDs never hide another owner's independent records"
    );
    assert!(random_other.profile.is_none());
    assert!(
        random_other
            .articles
            .iter()
            .all(|item| item.prediction.is_none() && item.error.is_none()),
        "another owner's scores and errors must not leak through reused article IDs"
    );
    let first = store
        .feed(owners[0].0, owners[0].1.as_uuid(), None, 1, false)
        .await
        .unwrap();
    assert_eq!(first.articles[0].id, ids[0].as_uuid());
    assert_eq!(first.scored, 3);
    let next = store
        .feed(
            owners[0].0,
            owners[0].1.as_uuid(),
            first.next_cursor.as_deref(),
            1,
            false,
        )
        .await
        .unwrap();
    assert_ne!(next.articles[0].id, ids[0].as_uuid());
    assert!(store
        .save_profile(owners[1].0, profile.clone())
        .await
        .is_err());
    let updated = store
        .save_profile(
            owners[0].0,
            InterestProfile {
                prompt: "Changed preferences".into(),
                ..profile.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.revision, "2");
    assert_eq!(updated.training_count, 2);
    assert!(store.save_profile(owners[0].0, profile).await.is_err());
    assert!(matches!(
        store
            .feed(
                owners[0].0,
                owners[0].1.as_uuid(),
                first.next_cursor.as_deref(),
                1,
                false,
            )
            .await,
        Err(AiError::Conflict)
    ));
    let reranked = store
        .feed(owners[0].0, owners[0].1.as_uuid(), None, 10, false)
        .await
        .unwrap();
    assert_eq!(reranked.scored, 0);
    assert!(reranked.articles.iter().all(|a| a.prediction.is_none()));
    let claim = store
        .claim_interest(&[owners[0].0], 60)
        .await
        .unwrap()
        .unwrap();
    store
        .finish_interest(&claim, Err(AiError::Budget), None)
        .await
        .unwrap();
    assert!(store
        .claim_interest(&[owners[0].0], 60)
        .await
        .unwrap()
        .is_none());
    let other = store
        .feed(owners[1].0, owners[1].1.as_uuid(), None, 10, false)
        .await
        .unwrap();
    assert_eq!(other.total, 3);
    assert_eq!(other.scored, 0);
    // Display rules are explicit, workspace-scoped and preserve every source row.
    for url in [
        "https://www.snowflake.com/content/snowflake-site/global/fr/blog/story",
        "https://www.snowflake.com/es/blog/story",
        "https://example.test/duplicate",
    ] {
        let copies = if url.contains("duplicate") { 2 } else { 1 };
        for _ in 0..copies {
            let article = Article {
                id: ArticleId::new(),
                key: DedupKey {
                    location: url::Url::parse(url).unwrap().into(),
                    title: "Same authored title".into(),
                    description: None,
                },
                state: ArticleState::default(),
                first_arrived_at: Utc::now(),
                origins: vec![],
                revision: 0,
            };
            for (_, workspace) in &owners {
                reader
                    .save_article(*workspace, None, article.clone())
                    .await
                    .unwrap();
            }
        }
    }
    sqlx::query("INSERT INTO reading_preferences(workspace,skip_duplicates,hide_snowflake_spanish,hide_snowflake_french) VALUES($1,true,true,true)")
        .bind(owners[0].1.as_uuid().to_string()).execute(pool).await.unwrap();
    let count = |workspace: WorkspaceId| {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM articles a WHERE workspace_key=$1 AND reader_article_visible(a)",
        )
        .bind(workspace.as_uuid().to_string())
    };
    assert_eq!(count(owners[0].1).fetch_one(pool).await.unwrap(), 4);
    assert_eq!(count(owners[1].1).fetch_one(pool).await.unwrap(), 7);
    let retained: i64 = sqlx::query_scalar("SELECT count(*) FROM articles WHERE workspace_key=$1")
        .bind(owners[0].1.as_uuid().to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(retained, 7);
    assert!(matches!(
        store
            .reject_terms(
                owners[1].0,
                owners[0].1.as_uuid(),
                ids[0].as_uuid(),
                "fixture",
                "context"
            )
            .await,
        Err(AiError::NotFound)
    ));
}
