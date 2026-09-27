# Architecture consistency: third audit implementation

Authoritative autopilot ledger. User authorized all ten audit recommendations,
implementation, verification and deployment under standing server authorization.
Preserve source bytes, user state, associations and explicit retry semantics.

| Obligation | Status | Acceptance criteria |
| --- | --- | --- |
| 1. Authoritative article state | verified | Dedup lookup holds identity only; redelivery/concurrent attachment preserves read/later/protection and monotonic revision. |
| 2. Stable article identity | verified | Upstream edits retain identity/history; genuine regrouping preserves all user associations transactionally. |
| 3. Durable initial backfill | verified | Received remainder survives HTTP 304/restart; completion reflects persisted progress. |
| 4. Atomic content snapshot | verified | Reader/AI share consistent pointer/chunk read and completeness validation during publication/cleanup. |
| 5. Frontend query ownership | verified | Subscription latest articles independent of current page; fulltext retry restarts observation with immediate stable feedback. |
| 6. Content representations | verified | Preserve original format/bytes; explicit text preview and safe rendered body; RSS/Atom/JSON coverage. |
| 7. Paid operation lifecycle | verified | Terms retains uncertain operation identity; corrupt jobs quarantined without dropping originals; deployment drain agrees with admitted provider work. |
| 8. Atomic origin admission | verified | Parallel claims honor origin capacity; blocked origins cannot starve another origin; expiry releases capacity. |
| 9. Typed module contracts | verified | Response type derived from decoder; web recipe validated once through shared typed boundary and persisted without HTTP draft leakage. |
| 10. Bounded efficient work | verified | Incremental AI parsing; measure chat write amplification and improve appropriately; icons persisted with bounded input/results; cohesive ownership. |
| 11. Release and deploy | in progress | Meaningful regressions, full release gate, backup/restoration, authorized rollout, API/browser smoke, committed evidence. |

Execution order: storage consistency (1–4,8), frontend/content/contracts (5–6,9),
AI/runtime/efficiency (7,10), integrated release (11). Independent read-only checks
may run concurrently; no subagents requested for this invocation.

No obligations cancelled. Audit findings are static paths until regressions prove
them. Each implementation slice records tests and trade-offs before verification.

Storage checkpoint: real PostgreSQL acceptance passed (16.48s), including
concurrent state preservation, stable ID/history on revision and merge, durable
backfill restart, parallel origin admission and a healthy origin behind >128
blocked jobs. Ingest unit suite: 26 passed. Frontend checkpoint: 150 passed,
typecheck passed. Full release/deployment remains pending. Follow-up tests for
corruption, snapshot publication races and UI geometry are still required.

Explicit deployment SQL replaces runtime compatibility migration for the dedup
index. Original descriptions remain unchanged; newly collected records retain
the declared media type. Existing absent media types are unknown, never guessed.

Implementation checkpoint: obligations 1–10 have their regression evidence.
Real PostgreSQL covers authoritative state locks, ID retention, merge/split lineage,
backlog restart, admission/expiry, quarantined raw payloads, deterministic content
publication/cleanup, exact upgrade reconstruction and rollback, and real SIGTERM
across both AI phases. Unit tests cover prepared recipe construction/deserialization,
RSS/Atom/JSON format fidelity, derived HTML/text presentation, incremental streaming,
and bounded icon admission/persistence/drain. Frontend: 152 tests plus the three
subscription browser scenarios (independent query, stable loading region, immediate
retry feedback, accessible name, fixed geometry and duplicate protection).

Full release is being rerun on the final tree after fixes discovered by that gate.
Candidate image builds and check-config passed on the authorized server; production
remains unchanged pending full gate and backup/restore rehearsal. Production preflight:
9,799 articles, 211 subscriptions, 1 account, zero dedup mismatches, zero incomplete
AI input records and no active paid requests. Detailed contracts/trade-offs are in
`docs/architecture/consistency-2026-09-27.md`.

Final release gate passed: `just check-release` (Rust fmt/Clippy/tests, generated contracts, frontend build/format, Docker PostgreSQL backup/restore, real-backend browser acceptance, real Chromium/CDP, 152 frontend tests, 39 browser scenarios, crate boundaries and 14 Python tests). Production rollout and final smoke remain in progress.
