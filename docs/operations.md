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
`0600`. Keep configuration and secrets outside version
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
All values must be positive before a connection is attempted.
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
configured external origin and must not expose Chromium or PostgreSQL ports.

## Backup and restore

PostgreSQL is the production source of truth. Back up the database persisted in the named
`postgres-data` volume with `pg_dump --format=custom` from the pinned PostgreSQL
image, write to a new operator-owned path, and keep the password in the Docker
secret. A valid backup is not just a successful command: restore it into a fresh
PostgreSQL database, run `prepare-schema`, compare every table count, and run the
authentication, library, content, queue, and cross-user isolation smoke tests
before accepting it.

A custom-format backup can be created from the running PostgreSQL container:

```sh
umask 077
set -o noclobber
docker compose exec -T postgres pg_dump -U inoreader -d inoreader -Fc > NEW_BACKUP_PATH.dump
```

Check the command's exit status and the archive with `pg_restore --list`. Keep
incomplete output after a failed dump separate from accepted backups. Restore
only into a newly created, separate empty database using `pg_restore
--exit-on-error --single-transaction`; do not use `--clean` against the running
database. Preserve the AI credential master key separately. Restoring the DB
without its matching master key cannot recover encrypted API or bot credentials.

The release gate includes the digest-pinned PostgreSQL Docker acceptance test.
It creates the complete schema, checks idempotency and constraints, exercises
lease theft fencing, and proves that two users may share a public fetch source
without sharing workspace, subscription, activity, note, or article state. It also dumps and restores the entire database, comparing every field in every
application table, restoring schema/indexes and resuming a saved job. Missing Docker is a hard failure, never a skipped test.

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

PostgreSQL unavailability makes readiness fail and never selects local or in-memory
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

## Consistency schema upgrade (2026-09-27)

This release requires the explicit offline SQL steps documented in
[consistency architecture](architecture/consistency-2026-09-27.md#deployment-and-rollback-contract).
The normal startup path does not migrate obsolete duplicate article snapshots or
split old AI documents implicitly. Preserve a verified old database/image/config
set before upgrading. Configure `server.graceful_shutdown_seconds: 700` and
`CONTAINER_STOP_GRACE_SECONDS=720` (Compose default); startup rejects insufficient
budgets for admitted two-stage AI work. Larger configured AI leases require larger
application and container budgets too.
