use super::*;

pub(super) async fn verify(
    pool: &PgPool,
    store: Arc<PostgresAiStore>,
    owner: Uuid,
    ws: Uuid,
    article: Uuid,
) {
    let title = "Original\u{00a0}title";
    let (service, provider, id) = fixture_title(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("Draft without heading"),
            Step::Review(r#"{"verdict":"unchanged"}"#),
            Step::Text("Chat reply"),
        ],
        title,
    )
    .await;
    service.work_once().await.unwrap();
    let done = service.chat(owner, id).await.unwrap();
    assert_eq!(done.status, ChatStatus::Completed);
    assert_eq!(
        done.messages[0].content,
        format!("**{title}**\n\nDraft without heading")
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
        "Chat reply"
    );
    assert_eq!(provider.inputs.lock().unwrap().len(), 3);

    let invalid = r#"{"verdict":"corrections","changes":[{"segment":1,"before":"absent","after":"replacement","reason":"wrong anchor"}]}"#;
    let (service, provider, id) = fixture_title(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("Kept draft"),
            Step::Review(invalid),
            Step::Review(r#"{"verdict":"unchanged"}"#),
        ],
        title,
    )
    .await;
    service.work_once().await.unwrap();
    let failed = service.chat(owner, id).await.unwrap();
    assert_eq!(failed.status, ChatStatus::Failed);
    assert_eq!(
        failed.messages[0].content,
        format!("**{title}**\n\nKept draft")
    );
    assert_eq!(failed.provider_calls.len(), 2);
    assert!(failed.provider_calls.iter().all(|c| c.usage.is_some()));
    assert_eq!(failed.provider_calls[1].status, CallStatus::Failed);
    for _ in 0..3 {
        assert!(!service.work_once().await.unwrap());
    }
    // Same operation key is idempotent; distinct concurrent keys are fenced too.
    let op = Uuid::new_v4();
    let (a, b) = tokio::join!(service.retry(owner, id, op), service.retry(owner, id, op));
    assert_eq!(a.unwrap().messages.len(), 2);
    assert_eq!(b.unwrap().messages.len(), 2);
    service.work_once().await.unwrap();
    let done = service.chat(owner, id).await.unwrap();
    assert_eq!(done.status, ChatStatus::Completed);
    assert_eq!(done.messages[1].content, failed.messages[0].content);
    {
        let inputs = provider.inputs.lock().unwrap();
        assert_eq!(inputs.len(), 3, "only the review is repeated");
        assert_eq!(inputs[1].review_draft, inputs[2].review_draft);
    }
    let retained: String = sqlx::query_scalar("SELECT content FROM ai_call_responses WHERE id=$1")
        .bind(failed.provider_calls[1].id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        retained, invalid,
        "paid rejected response is retained exactly"
    );
    storage_boundary(store.clone(), owner, ws, article).await;
    let (service, provider, id) = fixture_title(
        store.clone(),
        owner,
        ws,
        article,
        vec![
            Step::Text("Kept on restart"),
            Step::Error(AiError::Provider),
        ],
        title,
    )
    .await;
    service.work_once().await.unwrap();
    sqlx::query("UPDATE ai_chats SET scheduled_at=now()-interval '1 day' WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    let mut config = policy(owner).config().clone();
    config.automatic_summaries = true;
    let restarted = AiService::new(
        store.clone(),
        provider.clone(),
        CredentialCipher::new(&[5; 32]).unwrap(),
        AiPolicy::new(config, "style".into(), "review".into()).unwrap(),
    );
    restarted.schedule_once().await.unwrap();
    assert_eq!(
        restarted.chat(owner, id).await.unwrap().status,
        ChatStatus::Interrupted,
        "scheduler must never revive a failed paid check, even after its delay"
    );
    assert_eq!(provider.inputs.lock().unwrap().len(), 2);
}

async fn storage_boundary(store: Arc<PostgresAiStore>, owner: Uuid, ws: Uuid, article: Uuid) {
    let (_, _, id) = fixture_title(store.clone(), owner, ws, article, vec![], "Title").await;
    let claim = store.claim(30).await.unwrap().unwrap();
    assert_eq!(claim.record.view.id, id);
    let selection = reader_ai::CallModel::new(
        reader_ai::DeepSeekModel::Flash,
        claim.record.cost_rates.clone(),
    );
    let draft = r#"{"segments":[{"kind":"text","content":"**Title**"},{"kind":"text","content":"Retained body"}]}"#;
    for (phase, mode) in [
        (GenerationPhase::Generating, reader_ai::SpendMode::Summary),
        (
            GenerationPhase::Verifying,
            reader_ai::SpendMode::Verification,
        ),
    ] {
        let call = store
            .begin_call(
                &claim,
                phase,
                &reader_ai::SpendReservation::new("0.01".into(), "3".into(), mode).unwrap(),
                &selection,
            )
            .await
            .unwrap();
        let mut bill = usage();
        bill.estimated_cost_usd = Some(claim.record.cost_rates.cost(&bill).unwrap());
        store
            .update_call(&claim, call, CallUpdate::Usage(bill))
            .await
            .unwrap();
        if phase == GenerationPhase::Verifying {
            store
                .update_call(
                    &claim,
                    call,
                    CallUpdate::Response {
                        content: r#"{"verdict":"unchanged"}"#.into(),
                        system: "checked prompt".into(),
                    },
                )
                .await
                .unwrap();
            assert!(matches!(
                store
                    .update_call(
                        &claim,
                        call,
                        CallUpdate::Response {
                            content: "different paid response".into(),
                            system: "checked prompt".into()
                        }
                    )
                    .await,
                Err(AiError::Conflict)
            ));
        }
        store
            .update_call(
                &claim,
                call,
                CallUpdate::Finished {
                    status: CallStatus::Completed,
                    draft: (phase == GenerationPhase::Generating).then(|| draft.into()),
                },
            )
            .await
            .unwrap();
    }
    assert!(matches!(
        store
            .update_claim(
                &claim,
                ChatStatus::Completed,
                Some("Forged final result"),
                None,
                None
            )
            .await,
        Err(AiError::Review)
    ));
    store
        .update_claim(
            &claim,
            ChatStatus::Completed,
            Some("**Title**\n\nRetained body"),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        store.chat(owner, id).await.unwrap().view.status,
        ChatStatus::Completed
    );
}
