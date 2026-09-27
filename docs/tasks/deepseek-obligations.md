# DeepSeek obligation ledger

Authoritative scope: docs/deepseek-summary-spec.md, explicitly approved by user.
Research spend ceiling: USD 10. Never count unavailable/unfinished work as verified.
On 2026-09-27 the owner explicitly accepted candidate04 for use and waived another
questionnaire. This is a deployment authorization, not a new human score or a
passed factual gate; see `prompts/reading-data-news/deployment-approval.json`.

| Obligation | Acceptance | Status |
|---|---|---|
| Corpus | Full accessible channel history, preserved formatting/provenance, coverage and classifications | complete accessible archive: 1529 public posts; provenance and classification limits recorded |
| Source matching | Confirmed source/article pairs; unavailable/uncertain matches explicit | 7 development, 6 validation, 20 first-final +20 second-cohort +20 third-cohort source snapshots; third references opened only after candidate04 freeze; #1215 keyword-list reference limitation documented |
| Style study | Temporal/topic analysis, content selection, entities, explanations, vocabulary | 90 full development close readings, 1145 substantive-post metrics, paired independent reviews documented |
| Experiments | Frozen splits, baseline + rules + fixed examples; records and hard $10 ledger | All four frozen arms retained. Completed two-stage candidate04:40/40 calls,18/20 accepted-format outputs;3 definite factual-error cases. Final accounting330 attempts:USD4.542521440 known +USD2.898001920 retained uncertainty =USD7.440523360 underUSD10; no automatic retry/substitution |
| Human evaluation | Original research gate:20 blinded outputs; >=18 style passes and zero detected factual/numeric/quote defects. Later owner waiver permits use without another questionnaire | first20:19like/1unlike with18comments; candidate03:20like/0unlike,5comments, factual gate failed. Candidate04 accepted by explicit owner decision without another questionnaire; no new numeric human score. All18 outputs and missing2 remain in planned20, zero-error gate stays failed |
| Prompt deliverable | Versioned fixed prompt and model parameters, reproducible report | candidate01/02/03/04 frozen manifests retain their original unapproved research state. Separate deployment-approval.json authorizes candidate04 for owner use. Current draftv17 + verifierv3 preserve49+5 requests. Fresh full-source audit complete: definite defects998/273/747;286uncertain,1304title failure. Local abbreviation/selection requirements still partial. Final report, per-candidate sidecar, budget and failure history preserved |
| Profile | Account-isolated encrypted key input/remove/validation and balance | implemented; encrypted owner-bound keys and exact balance, unit/API/real PostgreSQL/browser checks pass |
| Provider | Shared outbound streaming, redacted observability, explicit validated limits | implemented; streaming, redirects, errors, quotes, modes and cancellation verified |
| Chat persistence | Article snapshots, durable jobs, idempotency, versions and no silent history loss | implemented; two-stage private draft/review-only retry/per-call exact billing and uncertainty tested with real PostgreSQL; isolation, restart and immutable snapshots pass |
| API | Auth/CSRF/ownership for every credential/chat/job operation | implemented and covered by server and PostgreSQL checks |
| Widget | Toolbar button, draggable overlay, persistence, drafts, cancel/retry, safe Markdown | implemented; deterministic unit/browser checks pass |
| Verification | Unit + real PG Docker + browser, isolation, errors, layout and quote checks | Current two-stage +exact-title `just check-release` passed101.212s: Rust202 +separateChromium3 +UI127 +browser30 +Dockerbackup/restore/static; researchPython72 andblindbrowser5 pass. Live paid production streaming draft+review passed,93.386s total,USD0.046344936, separate exact usage accounting |
| Deployment | Accepted candidate or documented explicit owner waiver; authorized host, secrets/config and public smoke verification | Completed2026-09-27: owner-only candidate04/reviewv3 on158.160.186.87. Public live/ready204, unauthorized profile401, wrong-origin POST403 and11 Playwright UI checks passed; temporary session deletion verified. Provider key intentionally unconfigured until owner enters it in Profile; no paid deployed request. See [deployment record](../deepseek-deployment-2026-09-27.md). No further questionnaire |

Dependencies: freeze held-out corpus before tuning prompts; reserve final-eval
budget before exploratory calls. Explicit owner acceptance now permits completion
and deployment despite documented unmet research criteria. Do not claim that the
factual gate passed or assign new questionnaire scores. Deployment verification
is completed and recorded separately; the research findings remain unchanged.
