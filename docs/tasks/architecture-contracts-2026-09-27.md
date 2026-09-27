# Architecture contracts implementation

User authorized all seven findings in the latest architecture audit through autopilot.
Standing deployment authorization: 158.160.186.87. Preserve every source byte and
user record; explicit offline database upgrades require verified backup/restore.

| Obligation | Status | Acceptance criteria |
| --- | --- | --- |
| 1. AI recovery | verified | Bounded configured recovery for chat, translation and definitions; corrupt expired jobs isolated with original payload retained; healthy jobs continue; uncertain paid calls never automatically repeated. |
| 2. Schema contract | verified | Explicit schema version and upgrade journal; incompatible databases rejected before HTTP/workers; separate initialization/upgrade command; missing upgrade and preservation regressions. |
| 3. Frontend commands | verified | Subscription commands own immediate pending, duplicate prevention, errors and workspace scope; resume failure/double-click/close/navigation and stable geometry regressions. |
| 4. Binary content | verified | BYTEA chunks, bounded batch writes, consistent reads, lossless explicit conversion; byte equality and performance/storage/restore evidence. |
| 5. AI read/progress separation | verified | Public reads and progress updates avoid immutable inputs; execution loads exact full context; version-aware unchanged polling; fencing and identity regression coverage. |
| 6. Validated operational types | verified | Private invariant-bearing limits, complete validated construction, alternative construction checks; review adjacent operational types and document scope. |
| 7. Module boundaries | verified | Responsibility-owned ingest storage, startup/discovery/CLI and frontend commands; real TypeScript import graph and Cargo renamed/platform dependency checks with regressions. |
| 8. Integrated release | verified | Relevant regressions, full check-release, reviewed diff, backup/restore rehearsal, authorized deployment, real smoke, commits and evidence. |

Order: recovery and validated contracts; frontend commands and graph checks;
binary storage/schema version and AI data access; module ownership; full release
and data-preserving deployment. No obligations cancelled. No subagents requested.

Baseline: clean repository at 6bdef8c. Audit established static risks; tests
must demonstrate repaired behavior before an obligation becomes verified.

## Implementation checkpoint

All seven slices are implemented. PostgreSQL Docker acceptance passed, including
native upgrade rollback and exact preservation; binary fixture reads and AI WAL
measurements are recorded in `docs/architecture/contracts-2026-09-27.md`. Frontend
and full release checks remain in progress; no production upgrade has happened.

## Local release verified

`just check-affected` and the full `just check-release` passed. The final gate
includes 218 Rust/acceptance tests, 157 frontend tests, 42 Chromium browser
scenarios and 16 Python tests. The final browser scenario proves a late Resume
response preserves the different dialog opened after Settings closes. Corrupt
retained drafts are quarantined before they can abort expired-chat recovery;
quarantined state takes precedence over cached public output. Candidate Linux
image is being rebuilt from the same production sources. Schema/content rollout
and actual production smoke remain in progress.

## Offline conversion adjustment

Production preflight measured 332 ms for a synthetic 262 KiB per-byte SQL
conversion. The rollout was paused before schema changes; replaced conversion
with native Rust keyset batches and transactional table compaction. The fresh
backup remains retained, and release/upgrade tests are being rerun. No requested
obligation was removed or narrowed.

## Production verified

Implementation commits `ef7870c` and `61af2f0` are deployed on 158.160.186.87.
The final full release gate passed after the native conversion adjustment.
A fresh 885,113,728-byte backup was restored to an isolated database; every
original row and content byte was compared before and after its upgrade. The
production upgrade then passed the same comparison: 234,035 rows across 47 tables,
including 9,800 articles, 211 subscriptions and 21,364 content chunks.

The healthy new container passed authenticated API and real browser smoke: feed,
subscription popup and return navigation, latest articles, full article, AI public
reads, unchanged polling and access isolation. Temporary test authentication was
removed; no paid AI generation was requested. All obligations are verified.
Post-start logs separately reported a Telegram history request failure; this
external integration is not established healthy by the architecture release smoke.
