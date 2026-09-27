# Architecture improvement ledger

Scope: implement all nine recommendations accepted by the user on 2026-09-27.
Preserve exact source data, tenant isolation, operation identity, durable writes,
and stable interaction geometry. Keep Rust/PostgreSQL/Preact modular monolith.

| # | Obligation | Acceptance criteria | Status |
|---|---|---|---|
| 1 | Frontend ownership and stale-response protection | Reader/session/workspace concerns extracted; request scope protects pagination, mutations and counters; delayed-response and account-switch regressions | verified locally |
| 2 | Background lifecycle | SIGINT/SIGTERM; supervised AI/glossary/icons/ingest; stop admission and await active work; bounded shutdown; fair AI work classes; tests | verified locally |
| 3 | API contract source of truth | Generated TS contract from server definitions; closed state enums; runtime decoding for critical responses; CI drift checks | verified locally |
| 4 | Backend boundaries | Feature-owned routes and repositories; narrow repository ports; use-case-owned article mutations and checked ownership; regression coverage | verified locally |
| 5 | PostgreSQL read model | Indexed lossless read projection for Feed/subscriptions; exact source preserved; 10k/100k/1m reproducible query measurements; consistency/restore tests | verified locally |
| 6 | AI attempt lifecycle | Frozen prompt/model/parameters/validator identity for translation and consistent attempt provenance; raw-response retention; replay validation without paid calls; cache identity and failure-isolation tests | verified locally |
| 7 | Shared UI primitives | FloatingPanel/AsyncButton/StatusRegion shared mechanics; canonical checked theme tokens; formatting; layout/pending/dedup regression tests | verified locally |
| 8 | Real application acceptance | Browser + real Rust API + PostgreSQL in release gate; persisted mutation, tenant isolation, delayed navigation, queue/idempotency coverage with deterministic provider | verified locally |
| 9 | Correlated diagnostics | Request/operation/job correlation through API, SQL and provider; queue/provider/validation durations; bounded metric labels and redaction tests | verified locally |

Final gates: affected compilation, complete release gate, separate final diff review,
backup/read-model preservation checks, deploy to authorized 158.160.186.87,
production smoke checks and clean commits. No item is complete just because a
file was split or a mock-only test passed.

## Verification evidence

- Complete `just check-release` passed: 205 Rust tests across 21 suites,
  142 frontend tests, 37 browser scenarios, 9 Python tests. Real Rust/PostgreSQL
  browser acceptance, backup/restore and real Chromium extraction were included.
- `just check-affected` passed (Rust 7.79s plus frontend typecheck).
- Dependency guard: 13 crates, acyclic; no internal dependencies in wire contracts.
- A final SQL review removed a parameterized CASE from the partial-index predicate
  so PostgreSQL generic prepared plans can retain index eligibility. Full gate
  rerun passed for that final change and the subsequent tenant-corruption guard.
- Synthetic 10k / 100k / 1m query benchmark completed with old and new indexes,
  warmup and median of three runs. At 1m rows: page 5.350 → 2.076 ms;
  unread count 6535.725 → 702.365 ms. These are query microbenchmarks, not
  end-to-end production latency or write-throughput measurements.
- Final review corrected CI to build embedded UI before acceptance and reject
  generated contract drift. Shared source formatting is intentional.

## Rollout status

- Production backup completed (840 MiB, archive catalog verified) at private server path
  `.inoreader-state/deployments/architecture-20260927/before.dump`.
- Automatic approval review rejected the source transfer twice despite the
  previous standing authorization. Explicit fresh confirmation was requested;
  do not bypass the restriction or claim deployment until it is lifted.
- Final code review completed; `git diff --check` passed.
- Still required: confirmed source transfer, container build/rollout, production
  smoke, and commits. Deployment remains blocked on approval, not on local tests.
