# inoreader

A self-hosted, multi-account feed reader built as a modular Rust application with
a Preact interface. It supports isolated workspaces, RSS/Atom/JSON Feed sources,
exact article deduplication, an unread-only Feed and Read later, subscription rules, full-text
archival, OPML, and browser-backed Web feeds. Search is intentionally an inert
placeholder in v1.

The product contract is [SPEC.md](SPEC.md). The implementation is still under
active construction; source-code presence is not evidence that the real
PostgreSQL/Chromium acceptance gate has passed.

## Architecture

The release contains one `inoreader` binary. It serves the same-origin API and a
compile-time embedded `web/dist` build. PostgreSQL is the only storage implementation. Chromium is a sidecar reached through CDP for JavaScript pages;
it is not part of the application binary.

The important module boundaries are documented in
[docs/architecture.md](docs/architecture.md):

- `reader-core` owns valid domain values and transitions;
- `reader-application` coordinates use cases through narrow ports;
- `reader-storage-postgres` owns PostgreSQL persistence and transaction boundaries;
- `reader-ai` owns account-scoped article conversations and the DeepSeek integration;
- `reader-collectors` parses feeds and site-specific formats;
- `reader-web-runtime` owns outbound HTTP/CDP policy and instrumentation;
- server crates own HTTP contracts, routing, and embedded UI assets;
- `inoreader` is the only composition root.

## Prerequisites

- Rust 1.96.0 (pinned by `rust-toolchain.toml`)
- Node.js 22 and npm for rebuilding the UI
- `just` for repository commands
- Docker Compose for the packaged app/PostgreSQL/Chromium topology

## Build and development checks

Install frontend dependencies and create the UI artifact before a clean Rust
compile that includes `reader-server-ui`:

```sh
npm ci --prefix web
npm run build --prefix web
just test-affected-dry
just check-affected
```

`just check-affected` is the ordinary compile-only gate. It runs Cargo checking
and TypeScript checking; it does not run tests, Clippy, formatting, Docker,
bundling, or E2E. The explicit `just check-release` command is reserved for an
authorized release/merge run.

Build the deployable image with `docker build .`. The multi-stage Dockerfile
builds the frontend, embeds it into the Rust crate, and copies only the resulting
binary into the runtime image.

## Configuration and startup

Copy `config.example.yaml` to the untracked `config.yaml` and replace every
environment-specific value. All resource and safety limits are explicit; unknown
fields and unsupported content policy values are rejected. Compose references
untracked secrets for PostgreSQL and AI credential
encryption. Follow [docs/deepseek-operations.md](docs/deepseek-operations.md) for the
32-byte encryption key; never replace an existing key during deployment.

```sh
cargo run -p inoreader -- --config config.yaml check-config
cargo run -p inoreader -- --config config.yaml prepare-schema
docker compose up --build
```

Production Compose contains the app, Caddy, PostgreSQL and Chromium.
The PostgreSQL Docker acceptance test starts its own isolated, digest-pinned service.

## Source inventory and seed review

The source inventory losslessly contains the 42 active `personal_feed` recipes.
Historical observations are not passed off as raw response fixtures. PDF OCR rows
remain unresolved until visual review; URL values are never guessed.

```sh
ruby tools/validate_source_inventory.rb source-inventory/inventory.json \
  /path/to/personal_feed/config.yaml
python3 tools/validate_seed_manifest.py --inventory-only source-inventory/inventory.json
just seed-preview ACCOUNT_UUID WORKSPACE_UUID seed-manifest.json
just seed-validate seed-manifest.json
```

The generated manifest is account/workspace scoped and does not write the database until
the explicit `seed MANIFEST --apply` command. See
[source-inventory/README.md](source-inventory/README.md) for fixture provenance
and [docs/operations.md](docs/operations.md) for the atomic application contract.

## Operations

[docs/operations.md](docs/operations.md) covers production topology, health and
failure behavior, seed review, and PostgreSQL backup/restore procedures. Restore
must target a separate fresh database and is successful only after archived
content, primary row counts, content manifests, and pending job recovery have
been verified.

The optional DeepSeek chat uses a key saved by each user in their own profile.
Its infrastructure, encryption/backup requirements and prompt-approval gate are
documented in [docs/deepseek-operations.md](docs/deepseek-operations.md). Research
artifacts and blind author evaluation are separate from production readiness; no
claim of an accepted author-style prompt follows from passing software tests.

## Test layout

Rust unit tests live beside their component in sibling `tests.rs`/`tests/`
modules; cross-component tests belong in `tests/`. Frontend component tests use
Vitest and Testing Library, and browser flows use Playwright. CI's ordinary job
is compile-only plus static contract checks. The manual release job runs the
broader suite and retains Playwright diagnostics, but a complete A01-A30 result
also requires operator-provided real PostgreSQL and Chromium acceptance infrastructure.
