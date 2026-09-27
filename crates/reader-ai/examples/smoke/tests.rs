use super::*;
use reader_ai::{GenerationMode, ReasoningEffort};
use std::os::unix::fs::PermissionsExt;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn snapshot() -> Snapshot {
    let settings = ReviewSnapshot {
        system_prompt: "Exact fixture prompt\nTransport fixture".into(),
        prompt_version: "smoke-fixture-v1".into(),
        model: "deepseek-v4-pro".into(),
        generation_mode: GenerationMode::Thinking {
            effort: ReasoningEffort::Low,
        },
        max_output_tokens: 100,
        cost_rates: CostRates::new("1.32".into(), "0.044".into(), "3.96".into()).unwrap(),
    };
    Snapshot {
        article: ArticleSnapshot {
            title: "Оригинальный заголовок".into(),
            source_url: "https://example.com/article".into(),
            safe_html: "<p>A\r\nB\n</p>".into(),
            text: "A\r\nB\n".into(),
            source_revision: "test1".into(),
        },
        generation: settings.clone(),
        review: settings,
        limits: InputLimits {
            context_tokens: 10000,
            framing_tokens_per_message: 64,
            framing_tokens_base: 64,
            max_input_bytes: 10000,
            max_response_bytes: 10000,
        },
        network: Network {
            connect_timeout_ms: 1000,
            request_deadline_ms: 10000,
            max_redirect_hops: 3,
            max_response_body_bytes: 10000,
            cancellation_poll_ms: 100,
        },
    }
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("reader-smoke-test-{}", Uuid::new_v4()));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn execution_requires_explicit_reservation_key_and_new_output_path() {
    let parsed = Arguments::parse(args(&["--input", "a.json", "--stage", "generating"])).unwrap();
    assert!(parsed.execution.is_none());
    for raw in [
        vec![
            "--input",
            "a",
            "--stage",
            "generating",
            "--key-file",
            "secret",
        ],
        vec![
            "--input",
            "a",
            "--stage",
            "generating",
            "--output-dir",
            "dir",
        ],
        vec![
            "--input",
            "a",
            "--stage",
            "generating",
            "--execute-reserved",
            "not-uuid",
        ],
        vec!["--input", "a", "--stage", "verifying"],
        vec![
            "--input",
            "a",
            "--stage",
            "generating",
            "--draft-run",
            "old",
        ],
        vec!["--input", "a", "--input", "b", "--stage", "generating"],
        vec![
            "--input",
            "a",
            "--stage",
            "generating",
            "--unknown",
            "value",
        ],
    ] {
        assert!(Arguments::parse(args(&raw)).is_err());
    }
    let id = Uuid::new_v4().to_string();
    assert!(Arguments::parse(args(&[
        "--input",
        "a",
        "--stage",
        "generating",
        "--execute-reserved",
        &id,
        "--key-file",
        "key"
    ]))
    .is_err());
    let parsed = Arguments::parse(args(&[
        "--input",
        "a",
        "--stage",
        "generating",
        "--execute-reserved",
        &id,
        "--key-file",
        "key",
        "--output-dir",
        "new",
    ]))
    .unwrap();
    assert_eq!(parsed.execution.unwrap().reservation.to_string(), id);
}

#[tokio::test]
async fn local_validation_has_no_key_no_network_and_no_output_side_effects() {
    let dir = Directory::new();
    let path = dir.0.join("input.json");
    let snap = snapshot();
    save_json(&path, &snap).unwrap();
    run(args(&[
        "--input",
        path.to_str().unwrap(),
        "--stage",
        "generating",
    ]))
    .await
    .unwrap();
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    let mut invalid = snapshot();
    invalid.limits.context_tokens = 101;
    assert!(invalid.input(Stage::Generating, None).is_err());
    invalid.network.cancellation_poll_ms = 0;
    assert!(invalid.network().is_err());
}

#[test]
fn review_requires_completed_exact_snapshot_and_preserves_envelope_bytes() {
    let dir = Directory::new();
    let snap = snapshot();
    save_json(&dir.0.join("snapshot.json"), &snap).unwrap();
    let envelope = "{\"segments\":[{\"kind\":\"quote\",\"content\":\"A\\r\\nB\\n\"}]}\n";
    let result = ResultRecord {
        reservation_id: Uuid::new_v4(),
        stage: Stage::Generating,
        finish: "validated_stop".into(),
        error: None,
        envelope: Some(envelope.into()),
        content: "> A\r\n> B\n> ".into(),
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            prompt_cache_hit_tokens: 0,
            prompt_cache_miss_tokens: 10,
            estimated_cost_usd: Some("0.000033000".into()),
        }),
        elapsed_ms: 10,
        first_publication_ms: Some(5),
        publications: 1,
    };
    save_json(&dir.0.join("result.json"), &result).unwrap();
    assert_eq!(verified_draft(&snap, &dir.0).unwrap(), envelope);
    let messages = snap
        .input(Stage::Verifying, Some(envelope.into()))
        .unwrap()
        .messages_json()
        .unwrap();
    let payload: Value = serde_json::from_str(
        messages[1]["content"]
            .as_str()
            .unwrap()
            .split_once('\n')
            .unwrap()
            .1,
    )
    .unwrap();
    assert_eq!(payload["article_snapshot"]["text"], snap.article.text);
    assert_eq!(
        payload["draft_summary"]["segments"][0]["content"],
        snap.article.text
    );
    let mut changed = snapshot();
    changed.article.text.push('!');
    assert!(verified_draft(&changed, &dir.0).is_err());
    changed = snapshot();
    changed.review.prompt_version.push('2');
    assert!(verified_draft(&changed, &dir.0).is_err());
}

#[test]
fn outputs_are_private_and_existing_evidence_is_never_overwritten() {
    let dir = Directory::new();
    assert_eq!(
        fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let path = dir.0.join("record.json");
    save_json(&path, &json!({"status":"before"})).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(save_json(&path, &json!({"status":"after"})).is_err());
    assert_eq!(read_json::<Value>(&path).unwrap()["status"], "before");
}
