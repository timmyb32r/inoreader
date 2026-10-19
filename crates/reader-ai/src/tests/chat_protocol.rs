use super::*;

fn discussion() -> GenerationInput {
    let mut input = input();
    for (role, content, status) in [
        (
            MessageRole::Assistant,
            "Previous answer: 中文\n\"quoted\"",
            MessageStatus::Complete,
        ),
        (MessageRole::User, "Explain this", MessageStatus::Complete),
        (
            MessageRole::Assistant,
            "unfinished",
            MessageStatus::Interrupted,
        ),
    ] {
        input.messages.push(ChatMessage {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            status,
            created_at: chrono::Utc::now(),
            phase: None,
            purpose: None,
        });
    }
    input
}

#[test]
fn discussion_history_preserves_text_inside_json_and_supplies_bounded_prefix() {
    let input = discussion();
    let messages = input.messages_json().unwrap();
    let previous: serde_json::Value =
        serde_json::from_str(messages[2]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        previous["segments"][0]["content"],
        input.messages[0].content
    );
    assert_eq!(messages[3]["content"], "Explain this");
    assert_eq!(
        messages.len(),
        5,
        "unfinished replies stay out of model history"
    );
    assert_eq!(
        messages[4],
        serde_json::json!({"role":"assistant","content":"{\"segments\":[","prefix":true})
    );
    assert!(super::input().discussion_prefix().is_none());
    let mut review = discussion();
    review.review_draft = Some("{\"segments\":[]}".into());
    assert!(review.discussion_prefix().is_none());
    assert_eq!(review.messages_json().unwrap().len(), 2);
}

#[tokio::test]
async fn discussion_continuation_reconstructs_json_stream_and_verifies_quotes() {
    for (content, valid) in [
        ("{\"kind\":\"text\",\"content\":\"Привет\"},{\"kind\":\"quote\",\"content\":\"Source quote\"}]}", true),
        ("{\"kind\":\"quote\",\"content\":\"fabricated quote\"}]}", false),
        ("     ", false),
    ] {
        let response = format!("{}data: [DONE]\n\n", event(serde_json::json!({"choices":[{"delta":{"content":content},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":6,"prompt_cache_hit_tokens":2,"prompt_cache_miss_tokens":10}})));
        let (provider, requests) = provider(response, StatusCode::OK);
        let mut progress = Progress { visible: vec![] };
        let output = provider.generate("test-key", discussion(), &mut progress).await;
        assert_eq!(output.is_ok(), valid);
        if valid {
            let output = output.unwrap();
            assert_eq!(output.content, "Привет\n\n> Source quote");
            assert!(serde_json::from_str::<serde_json::Value>(&output.envelope).is_ok());
        }
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1, "never automatically repeat uncertain paid calls");
        assert_eq!(requests[0].url.path(), "/beta/chat/completions");
        let body: serde_json::Value = serde_json::from_slice(requests[0].body.as_ref().unwrap()).unwrap();
        assert!(body.get("response_format").is_none(), "discussion bypasses the provider's empty-output JSON mode");
    }
}
