use chrono::{Duration as ChronoDuration, Utc};
use reader_application::{ReaderRepository, RepositoryError};
use reader_core::{AccountId, ReasonPolicy, Subscription, SubscriptionId, Workspace, WorkspaceId};
use reader_ingest::{IngestStore, WorkItem};
use reader_storage_ydb::{
    prepare_schema, ProductionYdbTransport, YdbClientLimits, YdbIngestStore, YdbRepository,
};
use std::{
    process::{Command, Output},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use url::Url;
use uuid::Uuid;

// ydbplatform/local-ydb:25.4.1, pinned by registry manifest digest.
const YDB_IMAGE: &str =
    "ydbplatform/local-ydb@sha256:55fdd320ee0064b9e8c628cb766d481f9be6a2c021ec8235ea8d2280fc937aaf";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(180);

struct YdbContainer {
    name: String,
}

impl YdbContainer {
    fn start() -> Self {
        require_docker();
        let name = format!("inoreader-ydb-acceptance-{}", Uuid::new_v4());
        let output = command(
            "docker",
            &[
                "run",
                "--detach",
                "--name",
                &name,
                "--hostname",
                "localhost",
                "--platform",
                "linux/amd64",
                "--security-opt",
                "no-new-privileges:true",
                "--publish",
                "127.0.0.1::2136",
                "--env",
                "YDB_USE_IN_MEMORY_PDISKS=true",
                "--env",
                "YDB_DEFAULT_LOG_LEVEL=NOTICE",
                "--env",
                "GRPC_PORT=2136",
                "--env",
                "MON_PORT=8765",
                YDB_IMAGE,
            ],
        );
        assert_success("start pinned YDB container", &output);
        Self { name }
    }

    fn connection_string(&self) -> String {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            let health = command(
                "docker",
                &[
                    "inspect",
                    "--format",
                    "{{if .State.Health}}{{.State.Health.Status}}{{end}}",
                    &self.name,
                ],
            );
            let output = command("docker", &["port", &self.name, "2136/tcp"]);
            if health.status.success() && output.status.success() {
                if let Some(connection) = healthy_connection(
                    &String::from_utf8_lossy(&health.stdout),
                    &String::from_utf8_lossy(&output.stdout),
                ) {
                    return connection;
                }
            }
            let diagnostics = self.diagnostics();
            let running = command(
                "docker",
                &["inspect", "--format", "{{.State.Running}}", &self.name],
            );
            assert!(
                running.status.success()
                    && String::from_utf8_lossy(&running.stdout).trim() == "true",
                "YDB container exited during startup: {diagnostics}"
            );
            assert!(
                !diagnostics.contains("Segmentation fault"),
                "YDB server process crashed during startup: {diagnostics}"
            );
            assert!(
                Instant::now() < deadline,
                "YDB container did not become healthy with a published gRPC port within {STARTUP_TIMEOUT:?}: {}",
                diagnostics
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    fn diagnostics(&self) -> String {
        let output = command("docker", &["logs", &self.name]);
        format!(
            "status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

// The pinned image's healthcheck executes a real YDB query. A process-name check
// is both weaker and incorrect under QEMU, where Docker reports the emulator.
// The repository additionally validates its own SDK connection before use.
fn healthy_connection(health: &str, mapping: &str) -> Option<String> {
    if health.trim() != "healthy" {
        return None;
    }
    let address = mapping.trim().parse::<std::net::SocketAddr>().ok()?;
    if address.ip() != std::net::Ipv4Addr::LOCALHOST || address.port() == 0 {
        return None;
    }
    // Discovery advertises localhost:2136 inside the container. This fixture
    // must keep the randomly published host endpoint instead of switching to it.
    Some(format!("grpc://{address}/local?use_discovery=false"))
}

#[test]
fn published_port_is_not_readiness_until_the_image_healthcheck_succeeds() {
    for health in ["", "starting", "unhealthy"] {
        assert!(healthy_connection(health, "127.0.0.1:32100\n").is_none());
    }
    for mapping in ["", "0.0.0.0:32100", "127.0.0.1:0", "127.0.0.1:not-a-port"] {
        assert!(healthy_connection("healthy", mapping).is_none());
    }
    assert_eq!(
        healthy_connection("healthy\n", "127.0.0.1:32100\n"),
        Some("grpc://127.0.0.1:32100/local?use_discovery=false".into())
    );
}

impl Drop for YdbContainer {
    fn drop(&mut self) {
        let output = command("docker", &["rm", "--force", &self.name]);
        if !output.status.success() {
            eprintln!(
                "failed to remove YDB acceptance container {}: {}",
                self.name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn real_ydb_preserves_repository_transactions_and_durable_leases() {
    let container = YdbContainer::start();
    let connection = container.connection_string();
    // The official local image exposes an anonymous test database. This is scoped
    // to this integration-test process and is the credential mode expected by the SDK.
    std::env::set_var("YDB_ANONYMOUS_CREDENTIALS", "1");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("create Tokio runtime");
    let result = runtime.block_on(run_acceptance(&connection));
    if let Err(error) = result {
        panic!(
            "YDB Docker acceptance failed: {error}\ncontainer diagnostics: {}",
            container.diagnostics()
        );
    }
}

async fn run_acceptance(connection: &str) -> Result<(), String> {
    let transport = connect_when_ready(connection).await?;
    prepare_schema(transport.as_ref())
        .await
        .map_err(|error| format!("prepare schema: {error}"))?;
    // Re-running schema preparation proves the version marker and DDL are idempotent.
    prepare_schema(transport.as_ref())
        .await
        .map_err(|error| format!("prepare schema a second time: {error}"))?;

    let reason_policy = ReasonPolicy::new(4096).map_err(|error| error.to_string())?;
    let repository = YdbRepository::new(transport.clone(), reason_policy, 100)
        .map_err(|error| error.to_string())?;
    repository
        .readiness()
        .await
        .map_err(|error| format!("repository readiness: {error}"))?;

    let owner = AccountId::new();
    let workspace_id = WorkspaceId::new();
    let mut workspace = Workspace::new(workspace_id, owner, "Acceptance library".into());
    repository
        .save_workspace(None, workspace.clone())
        .await
        .map_err(|error| format!("create workspace: {error}"))?;
    assert_eq!(
        repository
            .workspace(workspace_id)
            .await
            .map_err(|error| error.to_string())?
            .name(),
        "Acceptance library"
    );

    workspace.rename("Renamed library".into());
    repository
        .save_workspace(Some(0), workspace.clone())
        .await
        .map_err(|error| format!("CAS workspace update: {error}"))?;
    let stale = repository.save_workspace(Some(0), workspace).await;
    if !matches!(stale, Err(RepositoryError::Conflict)) {
        return Err(format!("stale CAS must conflict, got {stale:?}"));
    }

    let subscription_id = SubscriptionId::new();
    let subscription = Subscription::new(
        subscription_id,
        workspace_id,
        Url::parse("https://acceptance.example/feed.xml").map_err(|error| error.to_string())?,
        "Acceptance feed".into(),
    );
    repository
        .save_subscription(None, subscription.clone())
        .await
        .map_err(|error| format!("atomically provision subscription/source/job: {error}"))?;
    let listed = repository
        .subscriptions_by_workspace(workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    if listed != vec![subscription] {
        return Err("workspace-scoped subscription lookup returned the wrong rows".into());
    }

    let store = YdbIngestStore::new(
        transport,
        ChronoDuration::minutes(30),
        1,
        ChronoDuration::days(1),
    )
    .map_err(|error| error.to_string())?;
    let now = Utc::now();
    let lease = store
        .claim("acceptance-worker", now, now + ChronoDuration::seconds(30))
        .await
        .map_err(|error| format!("claim durable job: {error}"))?
        .ok_or_else(|| "provisioning did not leave a durable source job".to_owned())?;
    if !matches!(lease.item, WorkItem::PollSource { .. }) {
        return Err(format!("unexpected provisioned job: {:?}", lease.item));
    }
    store
        .renew(lease.job_id, lease.token, now + ChronoDuration::seconds(60))
        .await
        .map_err(|error| format!("renew fenced lease: {error}"))?;
    store
        .complete(lease.job_id, lease.token)
        .await
        .map_err(|error| format!("complete fenced lease: {error}"))?;
    if store
        .claim(
            "second-worker",
            now + ChronoDuration::seconds(1),
            now + ChronoDuration::seconds(31),
        )
        .await
        .map_err(|error| format!("check completed job visibility: {error}"))?
        .is_some()
    {
        return Err("completed job was claimable again".into());
    }
    Ok(())
}

async fn connect_when_ready(connection: &str) -> Result<Arc<ProductionYdbTransport>, String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut last_error = "YDB did not accept a connection".to_owned();
    while Instant::now() < deadline {
        let limits = YdbClientLimits::new(Duration::from_secs(5), 8, 2, Duration::from_millis(50))?;
        let attempt = tokio::time::timeout(
            Duration::from_secs(10),
            ProductionYdbTransport::connect_from_environment(connection, limits),
        )
        .await;
        match attempt {
            Err(_) => last_error = "YDB SDK connection attempt timed out".into(),
            Ok(Err(error)) => last_error = error,
            Ok(Ok(transport)) => {
                match tokio::time::timeout(Duration::from_secs(10), transport.health()).await {
                    Ok(Ok(())) => return Ok(Arc::new(transport)),
                    Ok(Err(error)) => last_error = error,
                    Err(_) => last_error = "YDB SDK health query timed out".into(),
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(format!(
        "YDB was not healthy within {STARTUP_TIMEOUT:?}: {last_error}"
    ))
}

fn require_docker() {
    let output = command("docker", &["version", "--format", "{{.Server.Version}}"]);
    assert_success(
        "connect to Docker daemon; Docker is mandatory for YDB acceptance",
        &output,
    );
}

fn command(program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to execute {program}: {error}"))
}

fn assert_success(operation: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{operation} failed (status {}): stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
