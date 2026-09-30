use super::*;
use reader_ai::{DeepSeekModel, ModelPreferences};

pub(super) async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    ws: Uuid,
    article: Uuid,
) {
    let off = ModelPreferences {
        summary: DeepSeekModel::Flash,
        verification: None,
    };
    // An already queued summary follows the new choice. No phantom checker,
    // charge or reservation is created; subsequent conversation remains usable.
    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![Step::Text("Unchecked draft"), Step::Text("Follow-up")],
    )
    .await;
    service.save_models(owner, off).await.unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_spending WHERE owner=$1 AND mode='verification'",
    )
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap();
    service.work_once().await.unwrap();
    let done = service.chat(owner, id).await.unwrap();
    assert_eq!(done.status, ChatStatus::Completed);
    assert_eq!(done.messages[0].content, "**Title**\n\nUnchecked draft");
    assert_eq!(done.messages[0].status, MessageStatus::Complete);
    assert_eq!(done.messages[0].phase, Some(GenerationPhase::Generating));
    assert_eq!(done.provider_calls.len(), 1);
    assert_eq!(provider.inputs.lock().unwrap().len(), 1);
    assert!(provider.inputs.lock().unwrap()[0].review_draft.is_none());
    let after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_spending WHERE owner=$1 AND mode='verification'",
    )
    .bind(owner)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        store.model_preferences(Uuid::new_v4()).await.unwrap(),
        ModelPreferences::default()
    );
    service
        .message(owner, id, Uuid::new_v4(), "Explain".into())
        .await
        .unwrap();
    service.work_once().await.unwrap();
    assert_eq!(
        service
            .chat(owner, id)
            .await
            .unwrap()
            .messages
            .last()
            .unwrap()
            .content,
        "Follow-up"
    );
    assert_eq!(provider.inputs.lock().unwrap().len(), 2);
    assert!(provider.inputs.lock().unwrap()[1]
        .messages
        .iter()
        .any(|m| m.content == "**Title**\n\nUnchecked draft"));

    // Switching off while generation runs saves that same draft, without check.
    service
        .save_models(owner, ModelPreferences::default())
        .await
        .unwrap();
    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![Step::ChangeModels(off, "Already paid")],
    )
    .await;
    service.work_once().await.unwrap();
    assert_eq!(
        service.chat(owner, id).await.unwrap().status,
        ChatStatus::Completed
    );
    assert_eq!(provider.inputs.lock().unwrap().len(), 1);

    // A saved draft carried by a retry publishes without repeating either call.
    service
        .save_models(owner, ModelPreferences::default())
        .await
        .unwrap();
    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("Retained draft"),
            Step::Error(AiError::RateLimit),
        ],
    )
    .await;
    service.work_once().await.unwrap();
    let failed = service.chat(owner, id).await.unwrap();
    service.retry(owner, id, Uuid::new_v4()).await.unwrap();
    service.save_models(owner, off).await.unwrap();
    service.work_once().await.unwrap();
    let done = service.chat(owner, id).await.unwrap();
    assert_eq!(done.status, ChatStatus::Completed);
    assert_eq!(
        done.messages.last().unwrap().content,
        failed.messages[0].content
    );
    assert_eq!(
        done.messages.last().unwrap().phase,
        Some(GenerationPhase::Generating)
    );
    assert_eq!(done.provider_calls.len(), failed.provider_calls.len());
    assert_eq!(provider.inputs.lock().unwrap().len(), 2);

    // An opt-out is no bypass for missing generation, source validation or leases.
    let (_, _, id) = fixture(store.clone(), owner, ws, article, vec![]).await;
    let claim = store.claim(120).await.unwrap().unwrap();
    assert!(matches!(
        store.finish_unchecked(&claim).await,
        Err(AiError::Conflict)
    ));
    store.stop(owner, id).await.unwrap();
    assert!(matches!(
        store.finish_unchecked(&claim).await,
        Err(AiError::Cancelled)
    ));

    // Explicitly re-enabling restores two stages for future jobs.
    service
        .save_models(owner, ModelPreferences::default())
        .await
        .unwrap();
    let (service, provider, id) = fixture(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("New draft"),
            Step::Review(r#"{"verdict":"unchanged"}"#),
        ],
    )
    .await;
    service.work_once().await.unwrap();
    assert_eq!(
        service.chat(owner, id).await.unwrap().messages[0].phase,
        Some(GenerationPhase::Verifying)
    );
    assert_eq!(provider.inputs.lock().unwrap().len(), 2);
}
