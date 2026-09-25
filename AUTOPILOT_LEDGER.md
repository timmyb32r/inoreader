# Autopilot obligation ledger

This is the authoritative implementation ledger for the v1 specification. A row
may become `verified` only with fresh evidence. Docker checks fail explicitly
when the host cannot execute a required pinned image.

| Obligation | Status | Evidence / remaining work |
|---|---|---|
| F01 — modular Rust/Preact workspace, dependency guards, shared contracts | implemented_compile_verified | Final workspace/all-target and TypeScript compile passed; parsed ten-crate dependency policy is acyclic |
| F02 — validated configuration, one embedded-UI binary, Docker Compose with Chromium | implemented_compile_verified | Strict config, embedded UI crate and hardened Compose compile; clean release artifact remains V03 |
| F03 — YDB-only storage port and concrete adapter; durable schema/jobs/outbox/leases | implemented_compile_verified | Official SDK, schema v5, fenced transactions, distributed origin limits, durable retries, isolated subscription-source mapping and source health compile; live YDB remains V03 |
| F04 — centralized safe outbound HTTP/CDP boundary and observability | implemented_compile_verified | Per-hop HTTP validation, CDP interception, configured text/JSON completion logs and request limits compile |
| F05 — source inventory and regression corpus for every known source | incomplete_external_evidence | 42/42 identities, exact configurations and evidence contracts validate; compiled runtime paths cover 2 standard feeds, 33 HTML recipes and 7 built-in adapters. Historical raw bodies remain 0/42 and reviewed PDF values 3/214 |
| F06 — dense unit/component/integration/browser test suites and CI entry points | verified_partial_external | Rust unit/integration, tool validators and real Chromium CDP acceptance run locally; real YDB remains blocked by the official amd64-only image on this arm64 daemon |
| A01 — one release app binary embeds and serves UI/API | implemented_compile_verified | UI embedding and one composition binary compile; isolated release smoke remains V03 |
| A02 — invalid config/secret/schema fails before workers without mutation | implemented_compile_verified | Strict config/secret/schema readiness is before bind/workers; runtime proof remains V03 |
| A03 — invite-only auth, password/reset/session semantics, field contracts | implemented_compile_verified | Configured Argon2id, opaque tokens, revocation and typed fields compile; behavioral execution remains V02/V03 |
| A04 — account/workspace authorization isolation across every boundary | implemented_compile_verified | Scoped API, preview, rule progress, seed and storage paths compile with isolation tests authored |
| A05 — state/rules isolation between workspaces sharing a source | implemented_compile_verified | Workspace-local state/rules and shared-source fan-out compile; regression tests authored |
| A06 — initial-depth visibility; RSS/Atom/JSON ingest; exact dedup semantics | implemented_compile_verified | Feed parsers, exact key semantics and durable incomplete/continuation state compile |
| A07 — open/read semantics; independent saved/later; bounded mark-all | implemented_compile_verified | Domain/API/UI transitions and boundary-pinned bulk mutation compile |
| A08 — durable automatic fulltext and explicit refresh/retry behavior | implemented_compile_verified | Automatic/manual durable jobs, visible failure and bounded retry compile |
| A09 — indefinite archive; text remains after age/unsubscribe/source loss | implemented_compile_verified | No TTL, reversible unsubscribe, preserved origins/content and latest saved content presentation compile; source-loss runtime proof remains V03 |
| A10 — atomic latest-successful content replacement and safe cleanup | implemented_compile_verified | Generational chunks, fenced manifest switch and resumable cleanup compile |
| A11 — literal rules, duplicate scope, actions/reasons/manual overrides | implemented_compile_verified | Matching, provenance, preview shared scope and UI compile |
| A12 — versioned rule lifecycle, cancellation, bulk apply/restart semantics | implemented_compile_verified | Pinned durable jobs, operation progress/cancel reason and guarded UI polling compile |
| A13 — Web feed visual builder for static/JS fixtures and scheduled parity | verified | Fixture Playwright and real Docker Chromium/CDP acceptance cover the builder, JS actions, geometry, and preview/scheduled parity |
| A14 — Web-feed viewport/overlay/selectors/actions/snapshot safety | implemented_compile_verified | CSS/XPath, validated multi-page navigation, actions, viewport and one-shot scoped stale-token protection compile |
| A15 — browser navigation/subresource isolation from private/metadata networks | verified | Real Docker Chromium acceptance proves metadata/private requests cannot bypass the shared SSRF boundary |
| A16 — untrusted article/preview content cannot execute in app origin | implemented_compile_verified | Sanitized text rendering, PNG-only preview and CSP compile; real browser proof remains V03 |
| A17 — crash/lease/outbox/fan-out correctness and fencing | implemented_compile_verified | Native fenced transitions, persisted retry age/origin limits and deterministic jobs compile |
| A18 — Chromium degraded mode; YDB fail-closed; durable recovery | verified_partial_external | Live Chromium degradation and recovery pass; YDB recovery coverage is implemented but cannot execute on this arm64 host |
| A19 — inert accessible Search / Coming later placeholder only | verified | UI contains disabled accessible placeholders, no search field; backend search route removed; component/browser specs exist |
| A20 — inventory accounts for PDF 214 and all 42 source IDs without loss | verified | Row-aware extraction accounts for 214/214 logical rows (215 fragments with one explicit page 2/3 merge); 42/42 config IDs validate; all OCR uncertainty remains explicit |
| A21 — seed idempotency, recipe mapping, no cross-account seed leakage | implemented_compile_verified | Atomic owner-scoped idempotent apply compiles for all 42 sources: 2 shared feeds, 33 isolated editable HTML recipes and 7 built-in adapters |
| A22 — OPML preview/errors/flattening/export/reimport | implemented_compile_verified | DTD/entity rejection, flatten warnings, atomic apply/export and stale-preview invalidation compile |
| A23 — responsive accessible English UI, themes, immediate feedback, no layout shift | verified | 37 component tests and 13 Playwright scenarios pass, including mobile navigation, pending deduplication and stable control geometry |
| A24 — backup/restore preserves primary data and resumes jobs | implemented_external_runtime_blocked | Mandatory two-container, digest-pinned YDB acceptance invokes the production dump/restore wrappers and verifies primary state, fulltext, rules, manifests and resumable leases. The local arm64 daemon cannot execute the official amd64-only YDB server under QEMU; amd64 CI runs it without a skip |
| A25 — split/merge state matrix, provenance, counters and concurrency | implemented_compile_verified | Merge/split state semantics, earliest arrival and provenance migration compile with matrix tests authored |
| A26 — required subscription pause reason/history/cursor/concurrency/UI | implemented_compile_verified | Validated reason/history, delivery gating and atomic resume + bounded catch-up compile; unsubscribe/restore preserves archived content |
| A27 — required workspace archive reason and selective restore | implemented_compile_verified | Validated aggregate restore and selective atomic refresh/backfill compile with concurrent delivery gating |
| A28 — per-source coverage matrix and trustworthy fixture provenance | incomplete_external_evidence | Machine-checked inventory and evidence contracts cover 42/42 config IDs; runtime mappings cover 2 feeds + 33 HTML + 7 adapters; PDF geometry accounts for 214/214. Raw bodies remain 0/42, tests are not executable in this run, and exact PDF values are visually reviewed 3/214 |
| A29 — component and real-browser UI suites with diagnostic artifacts | verified | 37 component tests, 13 fixture Playwright scenarios and 3 live Docker Chromium/CDP tests pass with screenshots/traces retained on failure |
| A30 — enforced modular boundaries and adapter reuse | implemented_compile_verified | Parsed Cargo graph guard passed for all ten crates and workspace compile passed; isolated/integration tests are authored but cannot be counted verified until the permitted release run |
| V01 — ordinary compile-only `just check-affected` gate | verified | Fresh final-tree run passed: workspace all-target Cargo check plus `tsc --noEmit` |
| V02 — focused regression tests needed for implementation confidence | verified | Rust tests excluding the amd64-only YDB container target, YDB adapter unit tests, six tool tests and Chromium Docker acceptance passed locally |
| V03 — full release/E2E/YDB/Chromium gate | blocked_host_architecture | Release gate and CI include all mandatory checks without silent skips. Chromium passes locally; both real-YDB tests fail explicitly because the official pinned image is linux/amd64 and ydbd exits 139 under QEMU on this arm64 Docker daemon |

## Dependency order

1. Establish workspace, validated domain contracts, frontend shell, inventory.
2. Connect storage/server/collectors and UI into a complete local vertical slice.
3. Add durable jobs, secure outbound/browser execution, import/export and seed flows.
4. Reconcile every A01–A30 criterion with tests and source fixtures.
5. Run the permitted compile gate, focused diagnostics, real smoke tests, and the
   full external gate where the required environment is available.
