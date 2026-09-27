# Architecture hardening: second audit

Authoritative autopilot obligation ledger. Scope: all eight recommendations in
the second architecture audit, implementation, regression coverage and authorized
deployment to 158.160.186.87. Preserve user data and existing UI semantics.

| Obligation | Status | Acceptance evidence |
| --- | --- | --- |
| 1. Reader read/write concurrency | verified | 10 controller regression tests passed; Page loads cannot discard pending writes; reordered success/failure tests, immediate pending feedback and stable controls. |
| 2. Durable AI replies | verified | Persist received attempt bytes before parsing/accounting, independently of publication lease; late reply and failure tests. |
| 3. Bounded bulk marking | verified | Application-owned scenario, bounded filtered storage selection, atomic conflict handling, isolation tests. |
| 4. Endpoint response contracts | verified | Explicit per-method decoders, unsupported schema rejection, mutation response regression. |
| 5. Valid execution requests | verified | Closed article scope, validated page size, rejected construction and caller coverage. |
| 6. Responsibility boundaries | verified | Reader composition split by responsibility; HTTP bulk handler delegates application rules. |
| 7. Real AI vertical acceptance | verified | Browser, server, worker, PostgreSQL and controlled provider exercised together with failures. |
| 8. Performance observations | verified | Latency distributions and queue/pool observations, documented budgets and checks. |
| 9. Release and deploy | verified | Full release gate passed; backup restored separately; final image healthy; authenticated API and Chromium smoke passed. |

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
- `just check-release`: passed on final implementation: 207 Rust tests across 21 suites, 149 frontend tests, 37 mocked browser scenarios, 14 Python tests.
- PostgreSQL acceptance includes backup/restore, bulk limit/conflict/isolation checks, immutable late AI reply capture and three real-backend browser scenarios (reader persistence, translation failure/retry/persistence, HTTP latency/request budgets).
- Full gate initially exposed incomplete Web Feed test data. Fixture was corrected to the real `FeedPreviewResponse`; targeted and complete reruns passed.
- No production schema change, no paid provider requests, no data transformation.
- `git diff --check` and final source review passed. Rollout verification is recorded below.

- Final controller review also fixed polling after refreshing a page with the same selected article. A regression proves polling restarts and the previous response cannot replace the fresh article.

## Production rollout evidence

- Implementation commits: `fc28f27`, `e6d17b2`.
- Deployed image: `sha256:337ac514ccbb4465b1141aa26f40a34adfa30428efa9f9591549c5f4de970b51`; container healthy.
- Backup: `.inoreader-state/deployments/hardening-20260927/before.dump` on the server, 880,802,111 bytes. Restored successfully into a temporary database: 9,795 articles, 211 subscriptions, 1 account. Verification database removed; previous image retained for rollback.
- Authenticated production smoke passed at 2026-09-27 16:14 UTC: bounded 50-article bootstrap, subscriptions, article detail, AI histories/profile and glossary. Unauthorized bootstrap returned 401; foreign workspace returned 404. All responses had request correlation IDs. Temporary test session removed.
- Chromium: reader opened, subscriptions dialog opened and returned on Escape, zero page errors. No paid AI requests or article mutations.
- Production instrumentation sample: 1,997 pool acquisitions and 5,965 SQL statements; histogram p95 upper bounds 0.512 ms and 4.096 ms respectively. This short smoke/startup window is diagnostic evidence, not a representative production SLO measurement.
- Docker combined output included one plain startup telemetry line; it was explicitly separated before JSON analysis, with the original retained. Local evidence: `.inoreader-state/deployments/hardening-20260927/`.
- Visual inspection also observed literal HTML in an excerpt-only article's summary. This content-rendering issue is outside the eight architecture findings and remains unresolved; passing browser smoke does not imply all article content renders correctly.
