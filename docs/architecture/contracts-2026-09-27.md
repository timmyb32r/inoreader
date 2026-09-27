# Architecture contracts and hot-path ownership

This change implements the seven findings recorded in the task ledger. It keeps
one deployable modular monolith and PostgreSQL; it adds no services or frameworks.

## Recovery and construction

`ai.recovery_batch` is required, positive, and validated by `AiPolicy` before worker
startup. Each chat/definition/translation claim recovers at most that many expired
rows, ordered by expiry and ID with `FOR UPDATE SKIP LOCKED`. Bad rows are
quarantined without rewriting their original document. Uncertain paid attempts
become explicit failures requiring a deliberate retry, never an automatic charge.

`IngestLimits` has five private nonzero fields and a single fallible constructor.
It cannot publish a partially configured object or use unlimited payload defaults.
Scheduler intervals/concurrency are checked before connection and construction.
Adjacent `AiPolicy`, `InputLimits`, generation mode, cost rates and review snapshots
retain their existing owning validation boundaries. This is a bounded review of
these execution paths, not a claim that every DTO in the repository is a validated
execution object. Wire DTOs and editable drafts remain intentionally incomplete.

## Schema and content

`prepare-schema` initializes an empty database. `upgrade-schema` is an explicit
offline, one-way transaction from the preceding unversioned release; it is never
called by startup. `schema_releases` journals the release and application time.
All other database commands verify version and critical physical column contracts
before starting the server or workers. This is not a full DDL fingerprint.

Chunk bytes are native `BYTEA`. Publication batches inserts within PostgreSQL's
65,535 parameter protocol capacity, while configured ingestion limits govern
payload admission. A snapshot still reads manifest and ordered chunks in one SQL
statement and verifies count, identity, order and UTF-8. The offline Rust converter reads keyset batches, fills a checked BYTEA column,
then swaps columns and compacts the table inside the same transaction. It rejects malformed/noninteger/out-of-range JSON bytes and preserves empty chunks.
It changes the storage representation only, never the source byte sequence. A server probe of the earlier per-byte SQL approach took
332 ms for only 262 KiB, so it was replaced before any production schema change.

## Chat read and write models

`ai_chats.inputs` retains exact article, prompt and execution configuration.
`document` retains mutable history/progress; `public_view` is a validated derived
projection. Worker claims load the full aggregate once. Provider-call updates and
stream progress lock only progress/public projection and use the fenced claim's
inputs. Ordinary public reads never select `inputs`. Explicit operations such as
starting, retrying or cancelling still load and validate the full aggregate.

Draft publication validates the complete generation envelope and quotations before
updating the public projection. Partial verification text stays private; the
completed first draft stays visible until final verification completes.

`GET /api/ai/chats/{id}/changes?after=<revision>` checks ownership even when unchanged.
It returns an opaque decimal-string revision and `chat: null` if unchanged. The
client preserves the current view, avoiding repeated payload decoding/rendering.
Quarantined queue state takes precedence over any older public cache, including
unchanged polls, and cannot be revived by late progress/accounting writes.
New progress increments the revision in the same transaction as both projections.
The offline upgrade validates all existing AI history before creating projections;
any corrupt history aborts the whole transaction, preserving replayability.

## Frontend commands and modules

`useSubscriptionCommands` owns root pause/resume/refresh requests, pending state,
deduplication and account/workspace publication. Dialog closure does not cancel an
accepted write. Workspace switching drains writes; late results cannot update a
different account/workspace. Controls own visible feedback in reserved regions.
`ReaderSidebar` owns sidebar presentation/menu state; late menu responses cannot
close a subsequently opened different menu or show its errors there. Catalog and
detail editors retain their own existing save contracts.

API view models belong to `api/viewModels`; shared copying belongs to `ui/CopyButton`.
The frontend guard resolves TypeScript imports/re-exports/dynamic literal imports,
checks feature direction, and detects runtime cycles. Computed runtime module names
are outside this static graph. The Cargo guard includes renamed, workspace-inherited,
build and target-specific production dependencies; dev dependencies are test seams.

PostgreSQL ingestion is split into queue, poll, delivery, content and rules modules,
with transaction ownership unchanged. Composition separates startup, discovery,
seeding and HTTP instrumentation from CLI dispatch. No compatibility wrappers or
new connector abstractions were introduced.

## Evidence

The Docker regression suite exercises failed and successful native schema upgrades,
exact input/chunk preservation, corrupt recovery, queue fencing, authorization,
public unchanged polling, and progress without reloading immutable inputs.
A 262,144-byte fixture occupies 935,937 JSON bytes versus 262,144 binary bytes;
ten local reads measured about 263 ms versus 14 ms. Twenty progress writes with a
372,246-byte pinned input measured 23,712 WAL bytes versus 8,197,912 for the old
full-input write baseline. These are repeatable local fixtures, not production SLAs.
Browser regressions compare control rectangles through pending and failed commands
and assert immediate busy feedback and duplicate suppression.

## Production rollout — 2026-09-27

Deployed implementation commits `ef7870c` and `61af2f0` to 158.160.186.87.
Image: `sha256:229ecb0e14c976e039765c3d5fb14f963c9edfe77e43f5453bc10dd23ae9dcf4`.

The complete release gate passed: 218 Rust/acceptance tests, 157 frontend tests,
42 browser scenarios and 16 Python tests, plus formatting, Clippy, contracts,
architecture checks and build. A fresh 885,113,728-byte PostgreSQL backup was
restored and compared with the stopped application's original database. The
restored upgrade and production upgrade each preserved all original columns,
234,035 rows across 47 tables, and every content byte. New schema/projection
columns were validated separately. Backup and restore rehearsal database remain
on the server under the private operator rollout directory.

Native schema upgrade took 147.84 seconds on the restored copy and 154.63 seconds
on production; these exclude backup and exhaustive comparison time. Logical
chunk payload decreased from 9,706,205,232 to 2,766,911,157 bytes. Physical chunk
relation including indexes/TOAST decreased from 1,736,261,632 to 929,325,056 bytes
(46.5%). No article content was truncated or rewritten.

Production smoke passed readiness, authentication and workspace isolation,
bounded 50-article bootstrap, subscriptions, article/translation/definition/chat
reads, AI profile, glossary read, popup return navigation and latest-article links.
Three existing chats passed revision-aware polling; unchanged responses were
28 bytes. Chromium reported zero page errors. The temporary session was deleted;
this smoke did not make paid provider calls.

Post-start logs contained a Telegram history request failure. The release smoke
checks glossary reads, not successful live Telegram ingestion, and does not claim
that external dependency is healthy. Schema and application startup passed.
