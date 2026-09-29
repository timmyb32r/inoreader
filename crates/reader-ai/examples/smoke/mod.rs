use async_trait::async_trait;
use reader_ai::{
    AiError, AiProvider, ArticleSnapshot, CompletedGeneration, CostRates, DeepSeekProvider,
    GenerationInput, GenerationProgress, InputLimits, ReviewSnapshot, Usage,
};
use reader_web_runtime::{
    ExternalRequestCompletion, ExternalRequestObserver, OutboundHttpClient, OutboundLimits,
    OutboundPolicy, RawOutboundLimits, ReqwestPinnedTransport, TokioDnsResolver,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    Generating,
    Verifying,
}

struct Arguments {
    input: PathBuf,
    stage: Stage,
    draft_run: Option<PathBuf>,
    execution: Option<Execution>,
}
struct Execution {
    reservation: Uuid,
    key_file: PathBuf,
    output_dir: PathBuf,
}
impl Arguments {
    fn parse(args: Vec<String>) -> Result<Self, &'static str> {
        let mut values = std::collections::BTreeMap::new();
        let mut args = args.into_iter();
        while let Some(name) = args.next() {
            if !matches!(
                name.as_str(),
                "--input"
                    | "--stage"
                    | "--draft-run"
                    | "--execute-reserved"
                    | "--key-file"
                    | "--output-dir"
            ) || values
                .insert(name, args.next().ok_or("missing argument value")?)
                .is_some()
            {
                return Err("unknown or duplicate argument");
            }
        }
        let input = values
            .remove("--input")
            .ok_or("--input is required")?
            .into();
        let stage = match values.remove("--stage").as_deref() {
            Some("generating") => Stage::Generating,
            Some("verifying") => Stage::Verifying,
            _ => return Err("--stage must be generating or verifying"),
        };
        let draft_run = values.remove("--draft-run").map(PathBuf::from);
        if (stage == Stage::Verifying) != draft_run.is_some() {
            return Err("only verifying requires --draft-run");
        }
        let execution = if let Some(value) = values.remove("--execute-reserved") {
            Some(Execution {
                reservation: Uuid::parse_str(&value).map_err(|_| "invalid reservation UUID")?,
                key_file: values
                    .remove("--key-file")
                    .ok_or("execution requires --key-file")?
                    .into(),
                output_dir: values
                    .remove("--output-dir")
                    .ok_or("execution requires --output-dir")?
                    .into(),
            })
        } else {
            if values.contains_key("--key-file") || values.contains_key("--output-dir") {
                return Err("key/output arguments require explicit --execute-reserved");
            }
            None
        };
        Ok(Self {
            input,
            stage,
            draft_run,
            execution,
        })
    }
}

/// The JSON is a frozen research record, not production deployment config. Both
/// system prompts include transport exactly once; no file resolution or defaults.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    article: ArticleSnapshot,

    generation: ReviewSnapshot,

    review: ReviewSnapshot,

    limits: InputLimits,

    network: Network,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Network {
    connect_timeout_ms: u64,

    request_deadline_ms: u64,

    max_redirect_hops: usize,

    max_response_body_bytes: usize,

    cancellation_poll_ms: u64,
}
impl Snapshot {
    fn settings(&self, stage: Stage) -> &ReviewSnapshot {
        match stage {
            Stage::Generating => &self.generation,
            Stage::Verifying => &self.review,
        }
    }
    fn input(&self, stage: Stage, draft: Option<String>) -> Result<GenerationInput, &'static str> {
        if self.article.text.trim().is_empty() || (stage == Stage::Verifying) != draft.is_some() {
            return Err("empty article or stage/draft mismatch");
        }
        let settings = self.settings(stage);
        if settings.prompt_version.is_empty() {
            return Err("prompt version is required");
        }
        let input = GenerationInput {
            model: settings.model.clone(),
            generation_mode: settings.generation_mode,
            system: settings.system_prompt.clone(),
            article: self.article.clone(),
            messages: vec![],
            max_output_tokens: settings.max_output_tokens,
            max_response_bytes: self.limits.max_response_bytes,
            context_tokens: self.limits.context_tokens,
            framing_tokens_per_message: self.limits.framing_tokens_per_message,
            framing_tokens_base: self.limits.framing_tokens_base,
            max_input_bytes: self.limits.max_input_bytes,
            review_draft: draft,
        };
        input
            .validate_context()
            .map_err(|_| "invalid or oversized complete request")?;
        Ok(input)
    }
    fn network(&self) -> Result<OutboundLimits, &'static str> {
        if self.network.cancellation_poll_ms == 0 {
            return Err("cancellation poll must be nonzero");
        }
        OutboundLimits::try_from(RawOutboundLimits {
            connect_timeout_ms: self.network.connect_timeout_ms,
            request_deadline_ms: self.network.request_deadline_ms,
            max_redirect_hops: self.network.max_redirect_hops,
            max_response_body_bytes: self.network.max_response_body_bytes,
        })
        .map_err(|_| "invalid outbound limits")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultRecord {
    reservation_id: Uuid,
    stage: Stage,
    // Success means production parser observed a valid stop + DONE + usage and
    // validated the complete envelope. Raw failure reasons are intentionally absent.
    finish: String,
    error: Option<String>,
    envelope: Option<String>,
    content: String,
    usage: Option<Usage>,
    elapsed_ms: u128,
    first_publication_ms: Option<u128>,
    publications: usize,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, &'static str> {
    let bytes = fs::read(path).map_err(|_| "input file unavailable")?;
    serde_json::from_slice(&bytes).map_err(|_| "invalid input JSON")
}

fn verified_draft(snapshot: &Snapshot, directory: &Path) -> Result<String, &'static str> {
    let prior: Snapshot = read_json(&directory.join("snapshot.json"))?;
    if serde_json::to_value(&prior).map_err(|_| "snapshot serialization failed")?
        != serde_json::to_value(snapshot).map_err(|_| "snapshot serialization failed")?
    {
        return Err("draft and verification snapshots differ");
    }
    let result: ResultRecord = read_json(&directory.join("result.json"))?;
    if result.stage != Stage::Generating
        || result.finish != "validated_stop"
        || result.error.is_some()
    {
        return Err("draft did not complete successfully");
    }
    let envelope = result.envelope.ok_or("draft envelope missing")?;
    CompletedGeneration::new(
        envelope.clone(),
        result.usage.ok_or("draft usage missing")?,
        &snapshot.article.text,
        snapshot.limits.max_response_bytes,
    )
    .map_err(|_| "draft envelope failed production validation")?;
    Ok(envelope)
}

fn private_file(path: &Path) -> Result<File, &'static str> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "private output could not be created; nothing overwritten")
}
fn save_json(path: &Path, value: &impl Serialize) -> Result<(), &'static str> {
    let mut file = private_file(path)?;
    serde_json::to_writer(&mut file, value).map_err(|_| "private output write failed")?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|_| "private output sync failed")
}

#[derive(Clone)]
struct Events {
    file: Arc<Mutex<File>>,
    failed: Arc<AtomicBool>,
}
impl Events {
    fn record(&self, event: Value) -> Result<(), AiError> {
        let result = (|| {
            let mut file = self.file.lock().map_err(|_| AiError::Storage)?;
            serde_json::to_writer(&mut *file, &event).map_err(|_| AiError::Storage)?;
            file.write_all(b"\n")
                .and_then(|()| file.sync_all())
                .map_err(|_| AiError::Storage)
        })();
        if result.is_err() {
            self.failed.store(true, Ordering::Relaxed);
        }
        result
    }
}
impl ExternalRequestObserver for Events {
    fn completed(&self, value: ExternalRequestCompletion) {
        // Shared-boundary classification only: no URL, body, key or SDK error.
        let event = json!({"event":"external_request", "system":value.system,
            "operation":value.operation,"outcome":format!("{:?}",value.outcome),
            "elapsed_ms":value.elapsed.as_millis()});
        let _ = tokio::task::block_in_place(|| self.record(event));
    }
}

struct Progress {
    events: Events,
    cancelled: Arc<AtomicBool>,
    started: Instant,
    rates: CostRates,
    content: String,
    usage: Option<Usage>,
    first_publication_ms: Option<u128>,
    publications: usize,
}
#[async_trait]
impl GenerationProgress for Progress {
    async fn check_active(&mut self) -> Result<(), AiError> {
        if self.events.failed.load(Ordering::Relaxed) {
            return Err(AiError::Storage);
        }
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(AiError::Cancelled);
        }
        Ok(())
    }
    async fn publish(&mut self, content: &str) -> Result<(), AiError> {
        let elapsed = self.started.elapsed().as_millis();
        self.first_publication_ms.get_or_insert(elapsed);
        self.publications += 1;
        self.content = content.to_owned();
        let events = self.events.clone();
        let event = json!({"event":"publication","elapsed_ms":elapsed,"content":content});
        tokio::task::spawn_blocking(move || events.record(event))
            .await
            .map_err(|_| AiError::Storage)?
    }
    async fn usage(&mut self, usage: &Usage) -> Result<(), AiError> {
        let mut usage = usage.clone();
        usage.estimated_cost_usd = Some(self.rates.cost(&usage)?);
        self.usage = Some(usage.clone());
        let events = self.events.clone();
        let event =
            json!({"event":"usage","elapsed_ms":self.started.elapsed().as_millis(),"usage":usage});
        tokio::task::spawn_blocking(move || events.record(event))
            .await
            .map_err(|_| AiError::Storage)?
    }
}

pub async fn run(args: Vec<String>) -> Result<(), &'static str> {
    let args = Arguments::parse(args)?;
    let snapshot: Snapshot = read_json(&args.input)?;
    // Check both frozen stages before paying for the first. The complete actual
    // review draft is checked again when present; no content is truncated.
    snapshot.input(Stage::Generating, None)?;
    snapshot.input(Stage::Verifying, Some("{\"segments\":[]}".into()))?;
    let draft = args
        .draft_run
        .as_ref()
        .map(|p| verified_draft(&snapshot, p))
        .transpose()?;
    let input = snapshot.input(args.stage, draft)?;
    let network = snapshot.network()?;
    let Some(execution) = args.execution else {
        println!("validated locally; no key read, no network request");
        return Ok(());
    };
    // A fresh directory prevents accidental reuse/overwrite of a prior paid run.
    DirBuilder::new()
        .mode(0o700)
        .create(&execution.output_dir)
        .map_err(|_| "output directory must be new and have an existing parent")?;
    save_json(&execution.output_dir.join("snapshot.json"), &snapshot)?;
    save_json(
        &execution.output_dir.join("request.json"),
        &json!({"reservation_id":execution.reservation,
        "stage":args.stage,"messages":input.messages_json().map_err(|_| "request framing failed")?,
        "started_at":chrono::Utc::now()}),
    )?;
    let events = Events {
        file: Arc::new(Mutex::new(private_file(
            &execution.output_dir.join("events.jsonl"),
        )?)),
        failed: Arc::new(AtomicBool::new(false)),
    };
    let key = fs::read_to_string(&execution.key_file).map_err(|_| "key file unavailable")?;
    // Explicit credential-file syntax: one token with an optional LF/CRLF.
    let key = key
        .strip_suffix("\r\n")
        .or_else(|| key.strip_suffix('\n'))
        .unwrap_or(&key);
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return Err("key file must contain one token with at most one trailing newline");
    }
    let http = OutboundHttpClient::new(
        OutboundPolicy::for_plain_http_hosts([], network),
        TokioDnsResolver,
        ReqwestPinnedTransport,
        events.clone(),
    );
    let provider = DeepSeekProvider::new(
        http,
        Duration::from_millis(snapshot.network.cancellation_poll_ms),
    )
    .map_err(|_| "invalid provider configuration")?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal_flag = cancelled.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_flag.store(true, Ordering::Relaxed);
        }
    });
    let mut progress = Progress {
        events,
        cancelled,
        started: Instant::now(),
        rates: snapshot.settings(args.stage).cost_rates.clone(),
        content: String::new(),
        usage: None,
        first_publication_ms: None,
        publications: 0,
    };
    let result = provider.generate(key, input, &mut progress).await;
    signal.abort();
    let (finish, error, envelope) = match result {
        Ok(completed) => {
            progress.content = completed.content().to_owned();
            let mut usage = completed.usage().clone();
            usage.estimated_cost_usd = Some(
                progress
                    .rates
                    .cost(&usage)
                    .map_err(|_| "cost calculation failed; retain reservation")?,
            );
            progress.usage = Some(usage);
            let error: Option<String> = None;
            (
                if error.is_some() {
                    "not_completed"
                } else {
                    "validated_stop"
                },
                error,
                Some(completed.envelope().to_owned()),
            )
        }
        Err(error) => ("not_completed", Some(error.to_string()), None),
    };
    let result = ResultRecord {
        reservation_id: execution.reservation,
        stage: args.stage,
        finish: finish.into(),
        error,
        envelope,
        content: progress.content,
        usage: progress.usage,
        elapsed_ms: progress.started.elapsed().as_millis(),
        first_publication_ms: progress.first_publication_ms,
        publications: progress.publications,
    };
    save_json(&execution.output_dir.join("result.json"), &result)?;
    if progress.events.failed.load(Ordering::Relaxed) {
        return Err(
            "private event recording failed; inspect known usage and retain uncertain reservation",
        );
    }
    if result.error.is_some() {
        return Err(
            "provider call did not complete; inspect private result and retain unknown usage",
        );
    }
    println!("one provider call completed; result saved privately; settle reserved ledger entry");
    Ok(())
}
