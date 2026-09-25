# Architecture

The application is a modular monolith. `reader-core` owns valid domain values and
state transitions. `reader-application` coordinates use cases through ports.
Collectors and the browser/HTTP runtime obtain untrusted external data. The YDB
adapter owns persistence details. Server crates translate HTTP DTOs and embed the
Preact build. The `inoreader` binary is the only composition root.

Dependencies point toward domain contracts. Core never imports HTTP, YDB, CDP, or
server code. Application code never embeds YQL. Adapters may depend on core and
application ports, but never on sibling adapters. The source-level boundary guard
in `scripts/check_crate_boundaries.py` makes the initial direction discoverable;
Cargo type checking remains the authoritative compiler boundary.

`reader-ingest` is the durable background-work boundary. It depends on feed
parsers and the shared web runtime, but neither on YDB nor the HTTP server. A
worker claims a single fenced job and performs network work outside database
transactions. The YDB adapter implements the semantic `IngestStore` operations:
poll commit plus fan-out outbox, exact-key delivery plus origin attachment, and
chunk publication plus manifest switch. Every operation checks the current lease
token. Retry may repeat any operation; source identity, delivery origin and
manifest revision are therefore idempotency keys rather than process memory.

Feed polling stops when a source has no active delivery targets. Fulltext work
already created for a delivered record remains independent and completes after a
subscription pause or workspace archive. Browser collection reports an explicit
degraded capability when CDP is unavailable; RSS/Atom/JSON polling and the reader
remain usable. No adapter falls back to an in-memory queue or database.

The YDB adapter uses these durable logical keys (escaped tuple components, never
concatenated ambiguous user text): `source_id`; `(source_id, upstream_id)` for
source-record idempotency; `(workspace_id, exact_location, title,
description_presence, description)` for a library candidate; `(workspace_id,
article_id, subscription_id, source_record_id)` for origins; `(record_id,
refresh_id, representation, ordinal)` for chunks; and `record_id` for the current
manifest. URL-less `exact_location` is `(source_id, upstream_id)`. Job identity,
lease token, run time and fan-out cursor are stored as separate typed columns so
claim/renew/complete can use indexed predicates without parsing JSON.

`commit_poll`, `deliver`, and `publish_content` are native YDB transactions. They
read and verify the typed lease token inside the same serializable transaction as
their writes. A general sequence of `compare_and_swap` calls is not an
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

Production has two Compose services: the application and Chromium. Managed YDB
is outside Compose. The optional local overlay adds the official local YDB
implementation for integration work without introducing another storage adapter.
The app joins a public-egress network and an internal browser-control network;
Chromium joins only browser-control. CDP is private and the browser container has
no direct Internet route. No service receives `docker.sock`.

## Durable operations

Seed import is modeled as an account-scoped import batch with stable per-source
idempotency keys. Inventory validation and manifest review happen before the
application creates subscriptions. Unresolved OCR rows cannot be selected. The
backend must atomically persist each application result so interruption resumes
without duplicate subscriptions; a local file alone is never proof of apply.

Backup and restore operate on the entire YDB database because archive content,
library ownership, jobs, leases, and outbox records form one durable contract.
Restore targets a separate database and preserves primary rows exactly. Derived
counters may be rebuilt only after primary verification. The application never
falls back to files or memory when YDB is unavailable.
