# Architecture hardening: second audit

Authoritative autopilot obligation ledger. Scope: all eight recommendations in
the second architecture audit, implementation, regression coverage and authorized
deployment to 158.160.186.87. Preserve user data and existing UI semantics.

| Obligation | Status | Acceptance evidence |
| --- | --- | --- |
| 1. Reader read/write concurrency | verified | 9 controller regression tests passed; Page loads cannot discard pending writes; reordered success/failure tests, immediate pending feedback and stable controls. |
| 2. Durable AI replies | verified | Persist received attempt bytes before parsing/accounting, independently of publication lease; late reply and failure tests. |
| 3. Bounded bulk marking | verified | Application-owned scenario, bounded filtered storage selection, atomic conflict handling, isolation tests. |
| 4. Endpoint response contracts | verified | Explicit per-method decoders, unsupported schema rejection, mutation response regression. |
| 5. Valid execution requests | verified | Closed article scope, validated page size, rejected construction and caller coverage. |
| 6. Responsibility boundaries | verified | Reader composition split by responsibility; HTTP bulk handler delegates application rules. |
| 7. Real AI vertical acceptance | verified | Browser, server, worker, PostgreSQL and controlled provider exercised together with failures. |
| 8. Performance observations | verified | Latency distributions and queue/pool observations, documented budgets and checks. |
| 9. Release and deploy | in progress | Full release gate, data-preserving rollout, deployed smoke; exact evidence and limitations recorded. |

## Execution order

1. Reader concurrency and regression slice.
2. Durable AI attempt capture and worker integration checks.
3. Valid query contracts and bounded application bulk mutation.
4. Endpoint decoding and frontend composition boundaries.
5. AI vertical acceptance and performance observations.
6. Full release checks, review, backup-aware deployment and smoke.

No items cancelled. No blockers currently known. No paid AI calls required for
deterministic verification.

## Local verification

- `just check-affected`: passed (Rust 6.51s plus frontend typecheck).
- `just check-release`: passed on final implementation: 207 Rust tests across 21 suites, 148 frontend tests, 37 mocked browser scenarios, 14 Python tests.
- PostgreSQL acceptance includes backup/restore, bulk limit/conflict/isolation checks, immutable late AI reply capture and three real-backend browser scenarios (reader persistence, translation failure/retry/persistence, HTTP latency/request budgets).
- Full gate initially exposed incomplete Web Feed test data. Fixture was corrected to the real `FeedPreviewResponse`; targeted and complete reruns passed.
- No production schema change, no paid provider requests, no data transformation.
- `git diff --check` and final source review passed. Rollout remains in progress.
