# Operations

## Production topology

The release artifact is one Linux `inoreader` binary containing the compiled
Preact application. Runtime dependencies are the PostgreSQL and Chromium
containers in `compose.yaml`. PostgreSQL uses a persistent volume and is reachable
only through the internal database network. The app joins the database,
public-egress, and internal browser-control networks. Chromium joins only browser-control, which is
declared `internal: true`, so it has no direct public or host-network route. The
CDP port is available only on that network. Browser HTTP(S) is fulfilled through
the app's shared outbound policy; a context-level black-hole proxy is a second
fail-closed barrier. Neither container receives the Docker socket or a host
directory containing browser state.

Copy `config.example.yaml` to `config.yaml`, set the public origin, and create a
random single-line PostgreSQL password in `secrets/postgres-password` with mode
`0600`. The YDB endpoint and `secrets/ydb-key.json` are required only while the
one-way migration is available. Keep configuration and secrets outside version
control. `check-config` validates the PostgreSQL password-file reference without
connecting to the database:

```sh
docker compose run --rm app --config /etc/inoreader/config.yaml check-config
docker compose run --rm app --config /etc/inoreader/config.yaml prepare-schema
docker compose up -d
```

## Incremental container builds

The production Dockerfile keeps Cargo registry, git, target, rustup, and npm
caches in named BuildKit cache mounts. A source edit invalidates the relevant
image layer but reuses compiled dependencies and unchanged workspace crates.
Run the normal command; no host Rust toolchain or bind-mounted build directory
is required:

```sh
docker compose build app
docker compose up -d --force-recreate app
```

Do not use `docker builder prune` during routine deployment because it deletes
these caches and makes the next build cold. `.dockerignore` keeps local build
artifacts, repository history, runtime configuration, and credentials out of
the build context.

The PostgreSQL pool size and acquire deadline are explicit validated settings.
The YDB request, concurrency, and retry settings apply only to migration-source
reads. All values must be positive before a connection is attempted.
Authentication exposes the Argon2id memory, time, and parallelism costs; invalid
combinations fail startup validation and the chosen costs are embedded in every
new password hash.

`observability.log_format` selects text or newline-delimited JSON completion
records for external requests. `external_request_metrics` is mandatory in v1
and must remain `true`; completion logging remains enabled independently.

TLS terminates at the Caddy reverse proxy in `compose.yaml`. Set
`EXTERNAL_HOST` in `.env` to the same DNS name as `server.external_origin`.
Caddy obtains and renews its certificate; ports 80 and 443 must be reachable
from the Internet. The proxy must preserve the
configured external origin and must not expose Chromium or YDB ports.

`compose.local.yaml` starts only the real local YDB implementation for an
isolated developer machine and persists it in a named volume. Start the app and
Chromium separately with a local config, so the production secret definition is
never weakened for convenience. Before a release gate, pin the YDB image to the
exact version approved for the target Serverless service; `latest` is not release
evidence. A host-run app uses endpoint `grpc://127.0.0.1:2136`, database path
`/local`, and an explicitly selected anonymous local credential provider.

## Backup and restore

User archives have no TTL. Backups therefore use the provider-supported YDB
export mechanism and include the complete database path. `tools/ydb_backup.sh`
is a strict wrapper around `ydb tools dump`: it refuses an existing destination
and writes its manifest only after the dump exits successfully. Credentials stay
in the YDB CLI environment and are never copied to the backup.

```sh
tools/ydb_backup.sh \
  --endpoint grpcs://example.net:2135 \
  --database /region/project/database \
  --output /secure/backups/inoreader-2026-09-25
```

Restore is a separate command and requires a different, explicitly named target
database. `tools/ydb_restore.sh` checks the backup manifest, requires the operator
to type the exact `ENDPOINT|DATABASE` target identity, and invokes `ydb tools restore`.
Automation may pass the same exact identity through `--confirm-target`; a database
path alone is insufficient because independent YDB clusters may use the same path. It never drops,
truncates, or prepares the target schema. Restore into a fresh database, run the
application's schema/version check, then compare primary row counts and sampled
content manifests before changing traffic. Jobs and outbox rows are primary
data: restore them as-is so expired leases resume through normal fencing logic.
Derived counters may be rebuilt only after primary verification.

```sh
tools/ydb_restore.sh \
  --endpoint grpcs://example.net:2135 \
  --database /region/project/restored-database \
  --input /secure/backups/inoreader-2026-09-25
```

The release gate runs `tools/test_ydb_backup_restore.sh` against two independent,
ephemeral containers using a digest-pinned YDB 25.4.1 image
(`sha256:55fdd320ee0064b9e8c628cb766d481f9be6a2c021ec8235ea8d2280fc937aaf`).
It populates representative library, fulltext, rule, state, and expired
lease rows; invokes the production backup and restore wrappers; verifies primary
rows; proves the expired job is eligible for leasing; and rebuilds the derived
subscription count from restored origins. Missing Docker or image access is a
hard failure rather than a skipped test. A deployment backup is verified only
after the same procedure succeeds against its separately provisioned target.

The `reader-storage-ydb` integration target independently starts the same pinned
image on a random loopback port and exercises the production Rust SDK, schema
preparation, optimistic transactions, atomic subscription provisioning, and
fenced durable leases. It is part of `cargo test --workspace`, owns its container,
and reports startup logs on failure. The official local YDB image is currently
`linux/amd64`; Apple Silicon runners therefore need Docker Desktop Rosetta or an
x86_64 CI runner. An incompatible runtime fails the gate explicitly.

## Initial source seed

`source-inventory/inventory.json` preserves all 42 legacy source definitions.
PDF OCR candidates remain untrusted until visual review. Generate a draft for
one explicit owner and workspace:

```sh
just seed-preview ACCOUNT_UUID WORKSPACE_UUID seed-manifest.json
just seed-validate seed-manifest.json
```

The draft records an idempotency key per source and contains no password or
infrastructure secret. All 42 sources map to the shared standard-feed collector,
the validated generic HTML recipe, or one of seven built-in publisher adapters.
Validation rejects unresolved, disabled, duplicate, or cross-owner entries
selected for application. Review it, then apply it with the same binary:

```sh
inoreader --config /etc/inoreader/config.yaml seed seed-manifest.json --apply
```

Application independently parses every selected recipe into validated runtime
types and rejects unknown fields, adapters, selectors, patterns, URLs and limits.
It verifies the manifest account owns the target workspace and commits
all selected subscriptions, source recipes, and initial jobs in one transaction.
Stable manifest keys make retrying the same manifest idempotent; a conflicting
reuse of a key fails without partial changes.

## Failure behavior

YDB unavailability makes readiness fail and never selects local or in-memory
storage. Chromium unavailability degrades browser-backed jobs while ordinary
feed ingestion and reading saved articles continue. `archive_budget_bytes` is a
diagnostic telemetry threshold in v1; it never deletes content, truncates input,
or rejects records. Configuration and schema errors stop startup before workers
begin.

## Chromium acceptance

The release gate runs `tools/run_chromium_acceptance.sh`. It starts the pinned
the digest-pinned `chromedp/headless-shell` 151.0.7922.109 image on the
loopback-only CDP port,
waits for `/json/version`, runs the real CDP collector
acceptance crate, and removes the container through an exit trap. The suite
checks JavaScript actions, preview geometry, repeated scheduled collection,
SSRF interception, and degraded/recovered probes. Missing Docker, an unavailable
image, or a failed health check is a release failure and is never converted to a
skip.
