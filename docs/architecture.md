# Architecture

The application is a modular monolith. `reader-core` owns valid domain values and
state transitions. `reader-application` coordinates use cases through ports.
Collectors and the browser/HTTP runtime obtain untrusted external data. The PostgreSQL adapter owns persistence details. Server crates translate HTTP DTOs and embed the
Preact build. The `inoreader` binary is the only composition root.

The [publication-date contract](publication-dates.md) separates publisher dates
from immutable workspace arrival times, preserves date precision, and describes
page evidence, historical enrichment and read-model projection.

Dependencies point toward domain contracts. Core never imports HTTP, PostgreSQL,
CDP, or server code. Application code never embeds SQL. Adapters may depend on core and
application ports, but never on sibling adapters. The source-level boundary guard
in `scripts/check_crate_boundaries.py` makes the initial direction discoverable;
Cargo type checking remains the authoritative compiler boundary.

`reader-ingest` is the durable background-work boundary. It depends on feed
parsers and the shared web runtime, but neither on PostgreSQL nor the HTTP server. A
worker claims a single fenced job and performs network work outside database
transactions. The PostgreSQL adapter implements the semantic `IngestStore` operations:
poll commit plus fan-out outbox, exact-key delivery plus origin attachment, and
chunk publication plus manifest switch. Every operation checks the current lease
token. Retry may repeat any operation; source identity, delivery origin and
manifest revision are therefore idempotency keys rather than process memory.

Feed polling stops when a source has no active delivery targets. Fulltext work
already created for a delivered record remains independent and completes after a
subscription pause or workspace archive. Browser collection reports an explicit
degraded capability when CDP is unavailable; RSS/Atom/JSON polling and the reader
remain usable. No adapter falls back to an in-memory queue or database.

The PostgreSQL adapter uses these durable logical keys (escaped tuple components, never
concatenated ambiguous user text): `source_id`; `(source_id, upstream_id)` for
source-record idempotency; `(workspace_id, exact_location, title,
description_presence, description)` for a library candidate; `(workspace_id,
article_id, subscription_id, source_record_id)` for origins; `(record_id,
refresh_id, representation, ordinal)` for chunks; and `record_id` for the current
manifest. URL-less `exact_location` is `(source_id, upstream_id)`. Job identity,
lease token, run time and fan-out cursor are stored as separate typed columns so
claim/renew/complete can use indexed predicates without parsing JSON.

`commit_poll`, `deliver`, and `publish_content` are native database transactions.
PostgreSQL locks and checks the job row in the transaction that writes its results. A general sequence of `compare_and_swap` calls is not an
implementation of these operations. Feed validators are committed with a
successful poll and sent as `If-None-Match`/`If-Modified-Since`; HTTP 304 updates
no records and creates no fan-out work.

Raw input, drafts, DTOs, and validated operational objects are distinct types.
Fallible constructors own intrinsic validation. External state is revalidated at
the last safe boundary before persistence, delivery, commit, or acknowledgement.

## Build and runtime boundaries

`web/dist` is a generated, reviewable build input. `reader-server-ui/build.rs`
copies every asset into Cargo's output directory and generates a static table;
the runtime never reads the repository, `web/`, or `node_modules`. HTML uses a
revalidation cache policy while content-addressed assets are immutable. Missing
UI output fails compilation instead of silently shipping an API-only binary.

Production Compose contains Caddy, the application, PostgreSQL and Chromium.
PostgreSQL uses a durable volume on the private database network. The app also
joins a public-egress network and an internal browser-control network; Chromium
joins only browser-control. CDP is private and the browser container has no direct
Internet route. No service receives `docker.sock`. PostgreSQL acceptance tests start an isolated digest-pinned container.

## Article conversations

`reader-ai` owns validated AI policies, provider-independent conversation records,
the repository port, encrypted credentials, DeepSeek protocol handling and durable
generation orchestration. Its HTTP requests use `reader-web-runtime`; it does not
depend on the server or a concrete database. PostgreSQL implements its repository
port. `reader-server` authenticates and scopes every operation to its account, and
the composition root wires optional, explicitly allowlisted access.

An explicit Summarize action pins the complete article text and prompt/model
version. Article opening never initiates a paid request. The conversation retains
that immutable snapshot across follow-up turns and explicit regenerated versions.
Leases and operation IDs prevent concurrent duplicate submissions; uncertain
provider outcomes require a deliberate retry. The server verifies each complete
quotation segment before publishing it. This is an exact-quotation guarantee,
not a guarantee of factual correctness for arbitrary model prose. Completed
Flash previews are exposed while Pro verification runs; `into_public_view`
projects the retained draft instead of partial verification output. Non-complete
status identifies the unchecked preview; final acceptance replaces it without
discarding the draft or prior versions.

The Preact `ai/` feature owns profile controls, safe Markdown and the floating chat.
The reader supplies article identity; `api/ai.ts` owns its wire contract. Chat state
is account scoped, controls show pending feedback immediately, and dragging or
streamed messages do not move the reader's controls. See
[deepseek-operations.md](deepseek-operations.md) for key management, startup gates,
limits, interrupted requests and the distinction between infrastructure readiness
and human approval of the author-style prompt.

## Durable operations

Seed import is modeled as an account-scoped import batch with stable per-source
idempotency keys. Inventory validation and manifest review happen before the
application creates subscriptions. Unresolved OCR rows cannot be selected. The
backend must atomically persist each application result so interruption resumes
without duplicate subscriptions; a local file alone is never proof of apply.

Backup and restore cover the entire selected database because archive content,
library ownership, jobs, leases, outbox records and conversations form one durable
contract. PostgreSQL backups must include the AI tables; the credential-encryption
key is backed up separately and must never be regenerated over an existing key.
Restore targets a separate database and preserves primary rows exactly. Derived
counters may be rebuilt only after primary verification. The application never
falls back to files or memory when its database is unavailable.

The [third audit implementation](architecture/consistency-2026-09-27.md) records
current article identity/history, durable backfill, atomic content snapshots,
typed recipe/transport boundaries, graceful AI drain and bounded maintenance work.

See [current execution/storage contracts](architecture/contracts-2026-09-27.md) for bounded recovery, schema versions, binary chunks, public AI projections, command ownership and graph guards.
