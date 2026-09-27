# DeepSeek integration verification

Verification date: 2026-09-27 (Europe/Moscow). Tests use mocked DeepSeek
responses; no provider key or paid model endpoint is used by these gates.

## Final gate status

**`just check-release` passed on the final implementation in 95.571 s**, including
the tariff snapshot, prompt-approval and HTTP 402/429 regressions, validated
standard/thinking mode and repaired browser/configuration fixtures.
`just check-affected` passed immediately before it in **1.917 s**. No release
check was skipped. The complete gate includes formatting, Clippy, all Rust tests,
real PostgreSQL/YDB/Chromium containers, frontend unit/browser tests, real YDB
backup/restore, and the repository's architecture/operational checks.

Latest completed checks:

| Command/check | Result | Duration |
| --- | --- | --- |
| `just check-affected` on final implementation | passed | 1.917 s |
| `just check-release` on the same implementation | passed | 95.571 s |
| Workspace formatting and all-target/all-feature Clippy | passed | included in full gate |
| Workspace Rust tests, including pinned PostgreSQL and YDB | 193 passed | YDB 20.16 s; PostgreSQL 5.91 s |
| `bash tools/run_chromium_acceptance.sh` | 3 passed | tests 2.46 s |
| `cd web && npm test` | 120 passed in 18 files | included in full gate |
| `cd web && npm run test:e2e` | 28 passed | 7.0 s |
| `./tools/test_ydb_backup_restore.sh` | passed in final gate and separately | separate run 42.641 s |
| `python3 scripts/check_crate_boundaries.py` | 12 crates; acyclic | 0.027 s |
| `ruby tools/validate_source_inventory.rb source-inventory/inventory.json` | passed; 42/42 IDs | 0.105 s |
| `python3 tools/check_operational_assets.py` | passed | 0.023 s |
| `python3 -m unittest discover -s tools/tests -p 'test_operational_assets.py'` | 3 passed | 0.090 s |

Full final log: `/tmp/inoreader-ai-final-20260927-release.log`; timing result:
`/tmp/inoreader-ai-final-20260927-results.json`. These local logs are verification
artifacts, not deployment assets. No server deployment or real-key import was
performed by this verification task.

## First-pass findings and repairs

- `just check-affected` passed in **4.677 s**: Cargo check **3.115 s**,
  TypeScript **1.562 s** (`target/affected-tests-timings.json`).
- Initial `just check-release` failed after **213.471 s** in the existing YDB
  acceptance fixture. Formatting, workspace Clippy and **162 Rust tests**,
  including the real PostgreSQL/AI suite (**5.71 s**), had passed.
- YDB was healthy and its gRPC port was published, but the fixture required
  Docker's process name to equal `ydbd`, which is unreliable with QEMU. Readiness
  now requires the pinned image's real-query healthcheck and a valid loopback
  port. A regression rejects a published port while health is starting/unhealthy.
  The fixture also now disables SDK discovery explicitly: a direct fixture
  `discovery list` returned container-internal `localhost:2136`, which would
  replace the random host port. Production discovery configuration is unchanged.
- Remaining shared HTTP tests passed: **24 tests**, command **2.381 s**.
- Initial Chromium Docker acceptance failed **1 of 3 tests** (**15.935 s**).
  Discovery failure escaped before updating capability/error classification.
  `health_check` now marks unavailable and returns `browser_probe_failed`;
  collection connection discovery marks unavailable and returns
  `browser_degraded`. The existing stopped-container recovery test exercises both.
- The operational checker previously selected PostgreSQL's network block as
  the application's block. It now scopes service blocks explicitly. Three new
  tests cover that regression, Chromium isolation, and the AI prompt/key mounts
  plus secret exclusion from the Docker build context.
- The second full attempt took **142.244 s** and reached Playwright, where
  14 old scenarios failed and 14 passed. Their fixture returned obsolete
  bootstrap/article-list shapes, omitted the account ID, and used old navigation
  and auth selectors. The UI owner updated the fixtures and current product
  scenarios, removed superseded response adapters, and retained functional
  assertions. All 28 scenarios pass in the final complete release gate.
- `docker compose` is unavailable in this host's Docker CLI; the installed
  equivalent `EXTERNAL_HOST=inoreader.duckdns.org docker-compose -f compose.yaml
  config --quiet` passed (**0.242 s**), without starting deployment services.

## New-feature coverage

The real pinned PostgreSQL test covers account isolation for direct lookup,
lists and mutations; exact operation-ID replay; concurrent initial opens;
immutable source/prompt/model snapshots; interrupted leases; explicit retry;
stop/key removal preserving history; old-key balance races; and detection of
missing full-text chunks. Provider tests cover encrypted key ownership,
context preflight before transport, no ambiguous POST retries, incremental
SSE/usage decoding, cancellation during headers, and exact source quotes
including CRLF and terminal newline preservation. Session/CSRF checks and a
provider-key rejection returning 422 rather than Reader's session-expiry 401
are covered in the server suite.

Generation-mode coverage rejects invalid/non-finite temperatures and contradictory
or unknown tagged parameters during construction and JSON/YAML deserialization.
Mocked provider tests cover all three thinking efforts, omission of ignored
temperature fields, exclusion of reasoning deltas from published output, and
preservation of the complete billed token count. Real PostgreSQL tests verify
that old-chat retries retain standard mode after a deployment switches to
thinking, and new thinking chats retain their mode on a standard-configured
worker. The current example configuration is experimental Pro/high at 8192 output
tokens and remains unapproved, with an empty enabled-account allowlist.

The prompt approval gate remains independent from software verification.
Passing tests does not approve the author's style or enable paid generation.

## Integration audit and rate-snapshot correction — 2026-09-27

Read-only integration review covered the approved specification, app composition,
provider/worker, PostgreSQL ownership queries, profile API, widget/hooks and their
existing tests. No corpus text or research response body was opened. The app
composes the AI service only for an explicit account allowlist; generation checks
approval at both enqueue and worker boundaries. Reopening an existing chat is a
read operation. The provider uses the shared streaming outbound boundary, and
SQLx completion timing applies to the AI store's existing pool. The UI starts
generation only through explicit actions and retains operation IDs after an
ambiguous acknowledgement.

Current local configuration is **experimental Pro/high, 8192 output tokens**,
with `prompt_approved: false` and `enabled_accounts: []`. The previous release
run at 97.502 seconds tested standard/0.3; the latest 95.571-second gate includes
the changed experimental configuration. The inspected candidate02
manifest is explicitly unapproved. Root confirmed no deployment, production
master-key creation or API-key import into a Reader profile has occurred; this
agent has done none of those actions. This is a task-history statement, not an
independent remote-server inspection.

The audit found a cost-estimation defect: a saved old-model conversation used
the worker's new-model tariffs after reconfiguration. It is now corrected with a
required immutable `cost_rates` snapshot in every persisted chat. Construction
and deserialization reject missing/invalid rates; exact decimal strings are
retained. There is no legacy reader, migration or fallback to current prices in
this unshipped schema. New versions capture current rates; old turns and retries
use saved rates. Estimates intentionally describe those saved tariffs, rather
than asserting the provider's future prices.

Verification for that correction:

- `cargo test -p reader-ai -p reader-storage-postgres --all-targets --all-features`:
  13 AI unit tests and 7 storage unit tests passed in **9.533 s** including compile;
  the mandatory Docker test then failed explicitly because the sandbox denied
  the Colima socket. No test was skipped or reclassified as passing.
- With access to the local Docker daemon,
  `cargo test -p reader-storage-postgres --test postgres_docker_acceptance --all-features`
  passed in **6.084 s** (test itself **5.85 s**). The real database verifies an old
  Flash retry under a Pro-configured worker and a new Pro conversation under a
  Flash-configured worker, including both models and cached-input/output tariffs.
  Logs: `/tmp/inoreader-ai-pricing-tests.log` and
  `/tmp/inoreader-ai-pricing-postgres.log`.
- The final real-PostgreSQL suite additionally proves that an unapproved prompt
  blocks new summaries, messages, retries and already-queued work despite a
  configured key; existing chats remain readable, and provider-call count never
  increases. Mocked HTTP 402/429 tests prove typed balance/rate-limit errors, one
  transport attempt and no publication of the upstream error body.
- The first full rerun stopped after **12.634 s** on a stale test that replaced
  `temperature: 0.3` in the now-thinking example. The test now supplies explicit
  mode YAML independently of the example's current selection and still checks
  non-finite/negative temperatures and contradictory mode parameters. Final
  `just check-affected` (**1.917 s**) and `just check-release` (**95.571 s**) pass
  after this repair; first-attempt logs are retained as `.attempt1.log` files.

Remaining delivery prerequisites are concrete:

1. Finish the frozen-candidate factual/quote review and the author's 20-case
   acceptance for that exact candidate. An earlier candidate's score does not
   approve a revised prompt. Keep paid generation disabled until both gates pass.
2. Install the accepted `style.md`, retain the exact compiled transport instruction,
   and record the matching version/model/mode/output limit and verified rates in
   the manifest and deployment config. Do not point `prompt_path` to the already
   combined `system.md`, which would append the transport twice.
3. Create the 32-byte master key once, make it readable by UID 10001, and back it
   up separately from a complete PostgreSQL backup. Compose mounts this secret
   unconditionally, so the file is needed before a Compose deployment even while
   the allowlist is empty. Preserve it across every later deployment. Configure
   only the intended account, and save its provider key through the authenticated
   profile flow; there is no startup import from `~/.deepseek`.
4. The final implementation now passes affected and full release gates. Recheck
   if accepted deployment assets require further code/configuration changes.
   Exercise real streaming with the accepted settings: reserve each research
   smoke call under the same $10 ledger and retain any unknown charge.
5. Deploy to the authorized host, verify health/profile and one complete
   summary/follow-up/quote flow, reopen it without a new paid call, and check
   persistence after reconnect. Confirm progress, latency, usage and errors in
   redacted logs. A real provider/deployed smoke is still pending; mocked tests
   do not replace it.

No model-training pipeline, dynamic retrieval or new database design is needed
to finish the approved first version. Future quality improvements remain separate
from the acceptance, secret/configuration and deployment prerequisites above.

## Feedback-driven research checkpoint

After the final software gate, only prompts/research artifacts and reports changed.
Candidate03 style+transport composition and frozen copies were compared byte-for-byte;
all20 article snapshots match candidate02 exactly. Python research tests passed:
26 collector/source/review tests and30 partition/budget/evaluation tests. A real
Chromium visit to `http://127.0.0.1:8765/next/` traversed all20 new items, verified
exact full raw texts and source URLs against the manifest, stable Next-button
bounds, and no page errors. The temporary browser context did not touch the
author's browser drafts. Screenshot: `/tmp/inoreader-author-review-next.png`.
The local HTTP endpoint returned200; original review and its feedback export
were retained with their own evaluation ID.

All40 paired model outputs passed transport checks, but candidate03 has detected
factual/terminological errors in4cases; it is not approved or deployed. Detailed
evidence: `docs/research/summary-report.md` and candidate03 evaluation result.
No production key import or deployment took place at this checkpoint.

## Two-stage summary implementation checkpoint

Initial summaries and explicit regeneration now run a private draft request and
a separate factual-review request over the same complete article snapshot.
Follow-up chat still uses one request. Both prompts, models, modes, output
bounds, input limits and tariffs are frozen with each conversation. A completed
draft is saved internally; failed verification can be retried explicitly without
paying for another draft. Neither restarting a worker nor an expired lease
automatically repeats an uncertain provider request.

The per-request ledger is stored atomically inside the account-owned chat
document, replacing the unshipped standalone `ai_usage` table. Usage is persisted
as soon as the provider reports it, before rejecting malformed, incomplete or
invalid-quote output. Known costs survive failure, cancellation and later retries;
missing usage remains unknown. The storage boundary validates reported cache
counts and exact estimates against the frozen stage tariffs. Late usage may
update its original call after cancellation or expiry, but cannot revive a job,
replace a newer assistant message or publish an unreviewed draft.

Scoped verification on the final backend tree:

- `cargo test -p reader-ai -p inoreader --lib --bins`: **16 AI + 20 application
  tests passed**; compilation **3.08 s**, test runners **0.03 s**. Log:
  `/tmp/inoreader-ai-two-stage-unit.log`. Coverage includes exact full UTF-8/CRLF
  source-plus-draft payloads, context guards before transport, preserved usage on
  rejected output, and invalid/missing review configuration at startup.
- `cargo test -p reader-storage-postgres --test postgres_docker_acceptance`:
  **passed in 12.011 s**, including compile; real PostgreSQL test **6.62 s**. Log:
  `/tmp/inoreader-ai-two-stage-postgres.log`. It exercises second-stage credential,
  rate-limit and network errors; idempotent concurrent retry; pinned reviewer
  settings and tariffs after reconfiguration; one-call follow-ups; quote rejection;
  stop/key removal between stages and stop during review; crash/lease expiry and
  explicit review-only retry; late billing without stale publication; raw-draft
  and completion-bypass guards; forged owner and forged-cost rejection. Existing
  account isolation and unapproved-prompt/no-provider-call checks still run.
- `cargo clippy -p reader-ai -p reader-storage-postgres -p inoreader
  --all-targets --all-features -- -D warnings`: **passed in 5.660 s**. Log:
  `/tmp/inoreader-ai-two-stage-clippy.log`. `cargo fmt --all` and
  `git diff --check` completed cleanly.

The initial sandboxed PostgreSQL run failed explicitly because Colima's socket
was unavailable to the sandbox; the subsequent run used the authorized local
Docker daemon. One intermediate schema assertion was updated from 38 to 37
tables after deleting `ai_usage`; no production table is being migrated or
deleted because this module has not been shipped. One unit fixture's context
bound was corrected so it tests an oversized request under intrinsically valid
configuration, instead of now-invalid output/context limits.

Independent integration review is in
`docs/research/two-stage-integration-review.md`. It found no additional backend
P0/P1/P2; the UI's premature Send action before a completed summary was fixed
with focused unit and browser regressions. The UI keeps summary content hidden
until successful verification and shows draft/review progress separately.

This checkpoint is **not a release or prompt acceptance**. The parent agent will
run the final affected/full release gates after the complete shared tree and
accepted prompt assets are frozen. Existing earlier full-gate evidence predates
this two-stage change. Production remains disabled/unapproved; no provider key
was imported, no master key was created and no deployment or paid API call was
performed by this backend agent. A second model pass is a measured factual
review, not a mathematical guarantee that all prose is true.

## Streaming smoke harness preparation

The research-only Cargo example `deepseek_smoke` now calls the production
provider/shared outbound boundary with immutable full inputs. Default execution
is local validation; reading an explicitly named token file and making one paid
request require a separate reservation UUID plus output path. The harness does
not implement a parallel budget policy or auto-retry. It saves private immutable
inputs, validated publication timing/content, known usage, completion and safe
errors for the root research ledger wrapper. Review rejects any changed snapshot
and requires an exactly retained completed draft. Only read-only accessors were
added to `CompletedGeneration`; production orchestration is unchanged.

`cargo test -p reader-ai --lib --example deepseek_smoke` passed **20 tests in
2.559 s** including compile. `cargo clippy -p reader-ai --all-targets --all-features
-- -D warnings` passed in **4.627 s**. Logs are
`/tmp/inoreader-ai-stream-smoke-tests.log` and
`/tmp/inoreader-ai-stream-smoke-clippy.log`. Tests cover explicit execution
arguments, validation without key/network/output side effects, exact draft/source
matching, mode0600/0700 and overwrite rejection. No real provider request was
made during preparation. Schema/run/settlement details are in
`docs/research/streaming-smoke-harness.md`.

## Exact final-summary title acceptance

The audit confirmed there was no original-title comparison in the stream parser,
worker or final repository transition. Added one shared exact-byte validator for
the first standalone bold heading. It runs only on completed summary review,
after known usage is saved, and again at the PostgreSQL accepted-state boundary.
NBSP→ordinary-space is rejected; no trimming, normalization, replacement or
fallback occurs. Drafts still reach review with a wrong title, so the second
request can repair them; follow-up chat remains title-optional. A failed check
retains the paid draft and both known bills, reports a clear error and allows
explicit verification-only retry. HTTP classification is
`original_title_changed`/422; the existing UI displays the stable explanatory
message and Retry without additional UI changes. The research streaming harness
uses the same final-stage guard.

Verification:

- `cargo test -p reader-ai --lib --example deepseek_smoke`: **21 passed**, **3.195 s**
  including compile (`/tmp/inoreader-ai-title-unit.log`).
- `cargo test -p reader-storage-postgres --test postgres_docker_acceptance`:
  **passed**, **12.158 s** total, **6.64 s** real Docker test
  (`/tmp/inoreader-ai-title-postgres.log`). New scenarios cover exact NBSP,
  changed draft→correct final, changed final→failed with retained usage,
  saved-draft-only retry, title-less follow-up, and direct repository bypass.
- `cargo test -p reader-server --lib changed_summary_title_has_a_stable_actionable_error`:
  **passed in1.533 s** (`/tmp/inoreader-ai-title-server.log`). The initial test
  expected a nested error envelope; it was corrected to the existing flat API
  shape without changing that API contract.
- Scoped Clippy for `reader-ai`, `reader-storage-postgres`, `reader-server`, all
  targets/features: **passed in0.774 s** on the final tree
  (`/tmp/inoreader-ai-title-clippy.log`). A test mutex guard was moved into a
  lexical scope before awaiting; no lint was suppressed. Formatting/diff checks
  pass. Parent owns the final full release rerun.

No prompt, model, source snapshot or frozen cohort was changed. No paid request,
key import or deployment was performed during this correction.


## Current two-stage release gate — 2026-09-27

After final backend/UI integration, streaming harness and candidate04 config freeze:

- `just check-affected`: PASS,4.256s.
- `just check-release`: PASS,102.447s, no ignored/failed tests. Rust workspace200 tests including real PostgreSQL/YDB acceptance and the research harness; separate Chromium extraction3; frontend127/20files; browser30; Docker backup/restore and architecture/inventory/operational checks all completed. Logs: `/tmp/inoreader-ai-two-stage-final-{affected,release}.log`; exact result records `/tmp/inoreader-ai-two-stage-final-results.json`.
- Research Python:37 budget/partition/evaluation tests,26 collection/source/blind-render tests (bundled Python with lxml),9 operational/seed/backup tests. The source tests initially failed under system Python because lxml was absent; using the documented bundled runtime passed.
- Blind-review Chromium5/5 passed in2.348s (`/tmp/inoreader-blind-review-regression.log`). Initial sandbox launch failed on macOS MachPort permission; rerun outside sandbox passed.

These are implementation/transport checks, not author-style or factual acceptance. Candidate04 remains `prompt_approved=false`, the full fresh research arm is still running, and production deployment is pending. Real paid streaming smoke is separately budgeted and recorded; it must not be inferred from mocked or Docker checks.


### Real paid production-adapter streaming smoke

Candidate04 draft+review, exact frozen prompts and known full source127, production `DeepSeekProvider` + shared outbound + stream parser: **both validated_stop**, each6 validated publication events, complete envelopes and known usage. Research-only harness; no Reader profile key was imported and no deployment performed.

- Generating:30.648s total,26.695s first complete segment; costUSD0.0144848.
- Verifying:62.738s total,58.238s first complete segment; costUSD0.031860136.
- Total:93.386s andUSD0.046344936; both research-ledger reservations settled from exact provider usage. Application Decimal tariff calculation matched the independent Python Decimal ledger.
- Full snapshot/request evidence in ignored `streaming-smoke-candidate04`; summaries pass exact title, JSON and quotation checks. This confirms real streaming integration, not general factual accuracy, a latency guarantee, deployed credentials or human style approval.


### Final exact-title boundary included in the full gate

After the title validator and typed `original_title_changed` error were added, reran final-tree gates: `just check-affected` PASS4.419s; `just check-release` PASS101.212s. Rust202, extractionChromium3, frontend127 and browser30 all pass, including real Postgres/YDB and backup/restore. Logs `/tmp/inoreader-ai-title-final-{affected,release}.log`, timings `/tmp/inoreader-ai-title-final-results.json`. This supersedes the prior implementation gate; paid smoke was not repeated because provider/prompt settings did not change, its preserved output already passes the same exact-title check, and the new storage/failure path is covered by real PG tests.


### Final review artifact smoke

After all40 candidate04 calls completed, generated a separate blind-review page with all18 accepted-format outputs, preserving the two failed cases in the research denominator and sidecar. Headless Chromium checked every full raw text/URL against the manifest, dynamic18 count, no preset votes, no remote requests and no page errors: PASS0.777s. Script `/tmp/inoreader-checked-review-smoke.mjs`. This does not update author scores or relax the failed factual gate. No production code changed after the full title-boundary release gate.

## Completed production deployment — 2026-09-27

The owner accepted candidate04/reviewv3 and waived further questionnaires.
Deployment to `158.160.186.87` and authenticated public UI smoke are now complete;
earlier pending-deployment statements above describe historical checkpoints.
The failed factual gate and frozen research manifests remain unchanged.

- Running image: `sha256:35da77df56ccc5a772aafe4784be1a9996eff66dbae1a9d729ac20c44cfa2dbd`.
- Original configuration preserved; complete PostgreSQL dump validated;
  encryption key and separate protected server copy created without logging bytes.
- Compose/config validation and actual startup passed; only the app was recreated.
- Public live/ready204, unauthenticated profile401, authenticated owner profile200
  with missing-key reason, wrong-origin balance request403 before provider I/O.
- Eleven browser checks passed, including immediate pending feedback, duplicate
  prevention, protected key entry, keyless generation blocked, drag/minimize
  geometry stability and focus restoration. No page errors or unexpected writes.
- Temporary test session deletion verified; no article-state changes; all AI
  tables empty. No research key imported and no paid deployed conversation run.

Evidence timestamp: `2026-09-27T00:42:52.725176+00:00`. Private report:
`.inoreader-state/deployments/deepseek-20260927/production-smoke-49a37f79-a6ee-4bea-b015-e0e8f0ca6b8c.json`.
The earlier test failure was an unrelated full-text-ready precondition on the
initial article; its report and cleanup are preserved. Removing that requirement
changed only the smoke script, not product behavior.

The optional local master-key export was rejected by automatic approval review;
it was not performed. The separate protected server copy remains available.
Full build, backup/rollback, package installation, security and test scope details
are recorded in [the deployment report](../deepseek-deployment-2026-09-27.md).

## Compact Flash chat amendment — 2026-09-27

The owner subsequently requested early first-pass publication and a simpler
widget. The full release gate passed again (frontend126, browser31; Rust and
Docker acceptance unchanged). The new image is deployed, the owner's key and
two saved conversations survived, and temporary smoke-session cleanup passed.
See [the amendment record](flash-chat-2026-09-27.md) for exact behavior, image,
backup location and verification scope. Earlier private-draft-only statements
above describe the superseded behavior. No paid Flash latency benchmark was run.
