use super::*;

const TITLE: &str = "Original\u{00a0}title";
const CHANGED: &str = "**Original title**\n\nText";
const EXACT: &str = "**Original\u{00a0}title**\n\nText";

pub(super) async fn verify(store: Arc<PostgresAiStore>, owner: Uuid, ws: Uuid, article: Uuid) {
    // The draft's NBSP loss must reach the checker unchanged. A correct final
    // title is accepted, and later chat turns do not need a heading.
    let (service, provider, id) = fixture_title(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text(CHANGED),
            Step::Text(EXACT),
            Step::Text("Reply without heading"),
        ],
        TITLE,
    )
    .await;
    service.work_once().await.unwrap();
    let done = service.chat(owner, id).await.unwrap();
    assert_eq!(done.status, ChatStatus::Completed);
    assert_eq!(done.messages[0].content, EXACT);
    let draft: serde_json::Value = serde_json::from_str(
        provider.inputs.lock().unwrap()[1]
            .review_draft
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(draft["segments"][0]["content"], CHANGED);
    service
        .message(owner, id, Uuid::new_v4(), "Explain".into())
        .await
        .unwrap();
    service.work_once().await.unwrap();
    let followup = service.chat(owner, id).await.unwrap();
    assert_eq!(followup.status, ChatStatus::Completed);
    assert_eq!(
        followup.messages.last().unwrap().content,
        "Reply without heading"
    );
    assert_eq!(provider.inputs.lock().unwrap().len(), 3);

    // A final NBSP→space change is a clear terminal failure, with both request
    // bills retained; retry consumes the same draft and checks the exact title.
    let (service, provider, id) = fixture_title(
        store.clone(),
        owner,
        ws,
        article,
        vec![Step::Text(CHANGED), Step::Text(CHANGED), Step::Text(EXACT)],
        TITLE,
    )
    .await;
    service.work_once().await.unwrap();
    let failed = service.chat(owner, id).await.unwrap();
    assert_eq!(failed.status, ChatStatus::Failed);
    assert_eq!(failed.messages[0].status, MessageStatus::Failed);
    assert_eq!(
        failed.error.as_deref(),
        Some(AiError::OriginalTitleChanged.to_string().as_str())
    );
    assert_eq!(failed.provider_calls.len(), 2);
    assert_eq!(failed.provider_calls[1].status, CallStatus::Failed);
    assert!(failed.provider_calls.iter().all(|c| c.usage.is_some()));
    assert!(!service.work_once().await.unwrap());
    service.retry(owner, id, Uuid::new_v4()).await.unwrap();
    service.work_once().await.unwrap();
    let done = service.chat(owner, id).await.unwrap();
    assert_eq!(done.status, ChatStatus::Completed);
    assert_eq!(done.messages[1].content, EXACT);
    {
        let inputs = provider.inputs.lock().unwrap();
        assert_eq!(
            inputs.len(),
            3,
            "retry did not regenerate the charged draft"
        );
        assert_eq!(inputs[1].review_draft, inputs[2].review_draft);
    }

    // A caller bypassing the worker is still rejected at the repository's
    // terminal transition, after a completed paid review is recorded.
    let (_, _, id) = fixture_title(store.clone(), owner, ws, article, vec![], TITLE).await;
    let claim = store.claim(5).await.unwrap().unwrap();
    assert_eq!(claim.record.view.id, id);
    let generation = store
        .begin_call(&claim, GenerationPhase::Generating)
        .await
        .unwrap();
    let mut bill = usage();
    bill.estimated_cost_usd = Some(claim.record.cost_rates.cost(&bill).unwrap());
    store
        .update_call(&claim, generation, CallUpdate::Usage(bill.clone()))
        .await
        .unwrap();
    store
        .update_call(
            &claim,
            generation,
            CallUpdate::Finished {
                status: CallStatus::Completed,
                draft: Some(
                    serde_json::json!({"segments":[{"kind":"text","content":CHANGED}]}).to_string(),
                ),
            },
        )
        .await
        .unwrap();
    let review = store
        .begin_call(&claim, GenerationPhase::Verifying)
        .await
        .unwrap();
    store
        .update_call(&claim, review, CallUpdate::Usage(bill))
        .await
        .unwrap();
    store
        .update_call(
            &claim,
            review,
            CallUpdate::Finished {
                status: CallStatus::Completed,
                draft: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .update_claim(&claim, ChatStatus::Completed, Some(CHANGED), None, None)
            .await,
        Err(AiError::OriginalTitleChanged)
    ));
    assert_eq!(
        store.chat(owner, id).await.unwrap().view.status,
        ChatStatus::Verifying
    );
    store
        .update_claim(&claim, ChatStatus::Completed, Some(EXACT), None, None)
        .await
        .unwrap();
    assert_eq!(
        store.chat(owner, id).await.unwrap().view.messages[0].content,
        EXACT
    );
}
