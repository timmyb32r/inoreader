# Third architecture audit: consistency and bounded work

## Authoritative records and history

`articles.document` is the sole article state/revision source. The dedup index
holds identity only; delivery locks the authoritative article before merging.
An upstream edit that replaces an article's last origin retains its article ID.
Actual merge/split follows the existing explicit state merge contract and records
workspace-scoped ancestry in `article_history_links`. History queries include that
ancestry, with account authorization, without rewriting original AI article IDs,
provider inputs, prompts or replies. Lineage is an association with the ancestor
article, not a copy of its conversations frozen at the regrouping timestamp.
New operation creation still requires the directly addressed current article.

Initial collection stores the entire received remainder in `poll_backlog` in the
same transaction as processed records and cache validators. Subsequent polls
consume configured bounded batches before making conditional HTTP requests.
Backlog identity, source and generation disagreement fails before commit; no
received records are silently discarded. Source polling is serialized, including
manual refresh and browser collection. The backlog holds a parsed source record,
its explicit action and original poll context; browser collection's own configured
page/action boundaries remain unchanged.

Content readers, AI input and rule evaluation use one SQL statement per snapshot
(manifest plus all ordered chunks), so READ COMMITTED cannot mix a previous
manifest with newly cleaned-up chunks. Count, ordinal, record identity and UTF-8
are checked before rendering. A deterministic PostgreSQL test pauses manifest
reading, publishes/reclaims the generation in another transaction, and verifies
that the paused reader still gets the exact old complete version.

## Boundaries and representations

Raw source descriptions and their declared media type are persisted separately.
Known HTML/XHTML gets an explicitly derived sanitized body and plain-text preview.
Plain text and unknown historical format remain literal text. No heuristic changes
old source identity or rewrites source bytes.

The neutral web recipe lives in `reader-core`. `PreparedWebFeed` pairs an immutable
validated execution recipe with the exact authored UI draft. Construction validates
configured action/page limits before preview or storage. Storage no longer parses
an HTTP JSON document using different limits. Operational recipe deserialization
checks intrinsic invariants too. External DNS/address checks remain at the outbound
request boundary. Core now uses native selector/regex parsers: this modest compile
dependency is intentional to keep validation authoritative and independent of HTTP
and concrete collectors, rather than introducing a second parser or adapter layer.

Generated `WireTypes` maps schema names to TypeScript responses. Transport infers
its result from its decoder contract; callers cannot supply an unrelated generic
response type. Raw/UI drafts remain distinct from validated execution state.

## Work ownership and lifecycle

Latest articles owns its bounded subscription query rather than slicing whichever
reader page happens to be visible. The recent-article region reserves its height
while loading. Full-text refresh is a controller-owned write, participates in the
navigation write barrier, deduplicates activation and restarts stopped polling.
Its initiating button gives immediate feedback without changing its hit target.

Terms retains an unacknowledged POST operation ID across network errors and panel
reopening. An acknowledged regeneration receives a new explicit ID. Corrupt queued
or expired chat/definition jobs are quarantined without altering original payloads;
a bad row no longer prevents healthy jobs from progressing. No uncertain paid
request is silently repeated.

`termination_signal` and the task supervisor are shared process lifecycle owners.
Shutdown stops admission, then drains accepted operations. Application grace must
exceed AI lease budgets (which already cover generation plus verification), HTTP,
server and browser work. Compose's stop budget must exceed application grace.
Production example: 700 seconds application grace and 720 seconds container stop.
These are maximum budgets, not a fixed shutdown delay. A subprocess acceptance
test sends real SIGTERM during a controlled first provider call and verifies exactly
two completed calls, durable final output and clean exit.

Origin admission uses transaction advisory locks on an origin fingerprint and
rechecks actual active leases before claiming. Hash collisions only serialize
unrelated origins, never merge their identities. Eligibility is filtered in SQL,
so 140 blocked jobs do not conceal work on another origin. Expired leases cannot
be renewed after capacity has been released.

## Memory and write efficiency

AI JSON streaming scans new bytes once and parses only newly completed segments;
final validation still rejects malformed envelopes and unverified quotations.
Chat storage separates immutable pinned input from mutable progress. Progress
retains the existing PostgreSQL TOAST input datum instead of rewriting the article
and prompts. Only resolving an absent snapshot to a pinned snapshot is permitted;
other input changes fail explicitly.

Measured real PostgreSQL fixture: 372,246 bytes of pinned input, 20 progress writes;
new WAL about 6.7–9.5 KB versus 8,197,992 bytes for full-document writes. One run
measured 0.5176 s for new store updates versus 0.7091 s for the old write-only SQL
baseline. The baseline excludes old read/deserialization cost. These are local
fixture measurements, not a production latency guarantee; the automated regression
requires at least 5x WAL reduction and exact input preservation.

Icon refresh owns keyset pagination, per-page grouping, bounded fetch admission,
and immediate persistence of each completed result before admitting a replacement.
Shutdown drains accepted requests. Failed URLs advance for the current pass and
are retried on the next scheduled pass. Limits reuse the explicit validated ingest
batch and icon-worker configuration. The composition-level module retains direct
PostgreSQL writes because icon scheduling is application maintenance, not a reusable
collector contract; introducing another general repository hierarchy adds no current
consumer value. A controlled slow/fast test proves bounds and persistence ordering.

## Deployment and rollback contract

Fresh schema and explicit offline upgrade are separate. Stop admission and drain
before running `tools/upgrade_article_index.sql` and
`tools/upgrade_ai_chat_inputs.sql`, after a verified backup/restore. Upgrades fail
closed for inconsistent identity or malformed/incomplete historical AI input.
Do not automatically repair, discard or choose winners. The regression reconstructs
all original JSON values after splitting inputs and checks failure rollback.

An old application image cannot run against the new schema. Rollback requires the
matching verified old database backup, old configuration and old image together;
retain any new production data separately before considering rollback. Do not
silently restore over user activity created after rollout.

## Final verification

`just check-release` passed on the final implementation: 215 Rust tests, 152 frontend
tests, 39 mocked browser scenarios, real-backend browser acceptance, PostgreSQL
backup/restore, actual Chromium/CDP and 14 Python tests. Production backup restored
and both upgrades rehearsed successfully before rollout. The real-site smoke
verified authenticated/unauthenticated and workspace boundaries, pagination,
subscription dialogs/latest articles and return navigation without JS errors.
All operational evidence and deployment identity are recorded in the
[task ledger](../tasks/architecture-consistency-2026-09-27.md).
