# Author-style summarization: frozen research protocol

Protocol date: 2026-09-26. Scope and acceptance: `deepseek-summary-spec.md`.
No paid generation or new-corpus style tuning preceded this protocol.

## Question and hypotheses

Can one fixed system prompt plus a complete article produce a faithful Russian
retelling that the author of @reading_data_news would accept as their own?
H0: a generic accurate-summary baseline is sufficient. H1: explicit author-specific
selection, explanation, vocabulary and formatting rules improve author fit.
H2: fixed development examples improve over rules alone. Test H2 against rules,
not only against the baseline. Compare Flash with Pro on a small development
subset; the protected final evaluation currently budgets Flash. Any change of
final model must first update a provable reservation without raising the $10 cap.

## Partitions and contamination controls

Archive all accessible public history with provenance. Freeze a metadata-only
partition manifest before reading newly collected bodies for style. All known
views of the same title or source URL share a partition. Reserve candidate groups
across observed calendar months and short/long post strata using an explicit
seed. A first English-looking line identifies a *candidate*, not confirmed pairing.
The initial inspected IDs 1523–1542 and their duplicate-source groups are development
only. Exclude neighbors of held-out candidates from tuning when they might be
continuations. Keep all original IDs, titles and URLs; grouping keys are explicitly
analysis-only derived representations, never changes to source records.

Confirm pairs by exact/close original title, primary-site URL, and factual alignment;
record the evidence. Ambiguous, inaccessible and non-article posts remain visible
in coverage reporting. Order final candidates before fetching source bodies;
select the first 20 confirmed accessible pairs across reserved strata. Failure to
find a source is reported, never hidden by moving a difficult example into training.
Source-based duplicate discoveries merge groups toward the more protected split;
if a supposedly held-out source was used in tuning, quarantine it from final eval.

All suitable development posts may inform corpus-wide style statistics. Study a
stratified set closely with originals; distinguish computational coverage from
close reading and count both. Do not claim to have manually compared every source.
Standalone definitions inform explanation style but are not independent summary pairs.

## Experiments and measurements

Preserve the generic baseline unchanged. Compare baseline, rules and rules+fixed
examples on the same development inputs. Freeze candidate versions before validation;
validation informs model/prompt selection, so it is not a final independent test.
Retain all outputs, not only winners. Preserve original article title, URL and full
extracted text with explicit extraction boundaries and raw source snapshot.

Record model, effective parameters, input framing, exact style and transport prompt,
usage, time, provider finish reason, and parsing/quotation failures. Prompt examples
must come only from development. Production and experiments share the exact JSON
segments transport and quote validation. A length-truncated response is a failure.

Evaluation rubric: factual support and numeric/unit/caveat fidelity; exact source
quotes; original title; no invented personal opinions/experience; central mechanism
and result selected; abbreviation/entity explanations; structure/length; vocabulary;
selective emphasis. Automatic string/number/quote checks are flags, not a proof of
semantic truth. Human/source review adjudicates them. Lexical overlap alone is not
an author-style score. Document counterexamples and negative comparisons explicitly.

Prepare 20 final outputs with opaque IDs, no visible model/prompt identity. Author
rates whether they could have written each with at most minor edits and annotates
failures. Acceptance: >=18/20 style passes and zero detected factual/numeric/quote
errors. Report a binomial confidence interval and the small-sample/source-availability
limitations. Once used for revision, these examples are no longer a closed test.

## Spending and stopping

All paid research, model judging and smoke requests go through
`tools/style_research/deepseek_experiment.py`. It reserves the full documented
context at verified peak uncached input rates plus configured output, atomically
protects 20 final calls, and retains unknown charges. Successful usage settles at
conservative peak rates. Invalid accounting halts further calls. Keys and payloads
never enter logs; artifacts stay in ignored local research state.

Stop with accepted evidence or when another safe reservation cannot fit $10.
Budget exhaustion, unavailable sources or failed human acceptance are explicit
unmet conditions. Independent engineering continues while the author evaluates.
No automatic prompt acceptance and no silent public Telegram posting.

## Development amendment 1 (before any validation/final generation)

The first three Flash trials on development #541 exposed severe verbosity:
baseline exhausted 4096 output tokens and returned incomplete JSON; rules-v1
produced 1277 body words; rules+examples-v1 produced 1092. All original requests,
outputs and prompts are retained. This fails the observed author compression even
where factual transport checks pass. We therefore added a **matched** 80–180-word
compactness instruction (up to250 for necessary complexity) to all three arms,
including a new concise generic baseline, to avoid comparing a constrained candidate
with an unfairly unconstrained baseline. This is model instruction, not post-output
truncation; overlong or incomplete output remains a measured failure.

## Development amendment 2 (still before validation/final)

Explicit word targets were weak controls: rules-v2 bodies ranged227–458words
on5development sources. A compact single-paragraph diagnostic on#541 yielded90words
without output truncation. We therefore compare a shorter semantic rule prompt
and matched generic baseline, both requesting usually3–4sentences with room for
an essential caveat/list, plus the same fixed development examples. v3 probes and
all verbosity/format failures remain recorded. This is exploratory prompt selection
on development; it is not evidence of final generalization or author acceptance.

## Development amendment 3 (after v4 validation, before final)

Independent reviews found every v4 development output 1.90–6.04 times its author's
body length, plus semantic mistakes in each arm on development or validation.
Examples improved some selection but did not reliably solve factual fidelity.
We therefore test a substantially shorter, strict three-sentence instruction,
with a matched compact baseline and an explicit instruction to preserve the
measurement's subject, denominator and comparison direction. This changes the
selection process: the six reviewed validation references have now informed
tuning and are no longer independent validation. All prior trials remain visible.
Final references and outputs are still untouched. We will describe the selected
candidate's format limitation and these adaptive comparisons explicitly.

We also compare Flash's explicit `thinking: enabled, reasoning_effort: high` on
the same five development inputs with rules-v5. Temperature is omitted because
[DeepSeek documents](https://api-docs.deepseek.com/guides/thinking_mode/) that it
does not affect thinking mode. The4096output-token allowance includes reasoning;
the same full-context budget reservations apply. Only final answer content is
evaluated or shown. This is an additional declared parameter comparison, not a
change to the untouched final sample.

## Second stage after candidate01 failed its factual gate

The first closed batch is retained as a failed test: at least cases82,1107,464
contain a changed measurement, goal-to-achievement shift or unsupported system
privacy guarantee. Human style feedback is still pending. Those examples are now
explicitly tuning data; they cannot be reused as a closed confirmation set.

A diagnostic thinking-enabled repeat on1107/82/464/826/1152 corrected several
problems but still misstated the metric-only throughput ceiling in1152. It is
one stochastic repeat per case, not proof that thinking alone caused the fixes.
We now compare rules-v7 in standard and high-thinking modes on these same five
diagnostic cases. The prompt distinguishes modality, measurement scope and causal
links, and replaces a rigid three-sentence instruction with a short, variable
format. Selection priority fixed before outputs: fewer source-fidelity defects,
then clearer selection/compression, then latency/cost. No significance claim.

Another twenty Flash calls are reserved under the unchanged USD10cap. Cohort02
follows the original frozen final candidate order/strata after excluding every
initial attempted source and duplicate; author-reference bodies remain closed
until candidate02 freezes. Both full final batches will be reported, including
failed results; no silent replacement, pruning, or repeated-test success claim.

### Diagnostic model comparison before candidate02

rules-v7 did not reliably follow its length/title instructions in either Flash
mode. Standard mode repeated the unsupported no-egress guarantee and omitted
the zero-business-work condition on throughput; high thinking improved these
known cases but remained verbose. Before freezing candidate02, compare Pro with
high thinking on the same five source snapshots and the same v7 prompt. All
results are exploratory, with one stochastic sample per case. The existing
Flash final reservation and hard USD10 ledger remain in force during this probe.
Changing the final model requires a separate documented budget-plan amendment.

### Explicit candidate02 budget-plan amendment: sequential Pro calls

The original twenty-call Flash protection was an internal completion guarantee,
not permission to spend beyond the user's **$10 total research cap**. Twenty
full-context Pro reservations cannot fit that cap: at the verified peak rates,
one Pro request with a 1,048,576-token input bound and an 8,192-token output bound
reserves **$1.41656064**, and twenty would require **$28.33121280**. Actual observed
token usage does not justify reducing the bound for an unknown next request.

This amendment authorizes an explicit transition of candidate02's **twenty
untouched future Flash slots** to a named Pro plan with a maximum output of 8192.
The target remains twenty cases; **completion of all twenty is conditional on
the remaining budget and is not guaranteed**. Freeze the selected prompt/model
parameters and ordered closed cohort before the first final request. Model/style
acceptance criteria and contamination controls above remain unchanged.

The ledger transition is a deliberate call to
`Budget.amend_final_plan_to_sequential(label, 'deepseek-v4-pro', 8192, reason)`.
It must follow completion/joining of all exploratory workers. The method rejects
any in-flight operation, an accounting halt, an already-started final batch, or
a repeated amendment. Existing ledgers keep their original protection until that
method is called; loading the runner never silently changes the spending policy.

The amendment records the old plan, the new named plan, the reason, the exact
committed amount and the per-call upper bound. It releases only the unspent
future Flash protection; every previous entry, settled cost, failed outcome,
unknown charge, price and the $10 cap is preserved. A timeout or ambiguous result
keeps its complete reservation. Accounting-contract violations still halt all
further requests. No balance estimate or assumed refund clears an unknown charge.

For every final call, under the same ledger lock, require:

`sum(settled costs + all unresolved full reservations) + next full reservation <= $10`.

The runner permits at most one client-side final attempt in flight, rejects the
wrong model, an output bound above 8192, or a 21st attempt, and records the named
plan plus a monotonically increasing attempt number on each entry. Settling an
attempt releases only its known unused money; it never refunds a consumed slot.
Earlier batches and unsuccessful final attempts remain visible in reporting.

Stop before any request that cannot satisfy the full reservation, on an
accounting halt, or after twenty final attempts. An unknown outcome is retained
as an unresolved/failed case and is never automatically retried or silently
replaced. A budget-stopped or otherwise incomplete cohort does not satisfy the
twenty-case author gate and cannot approve the production prompt. Report the
completed count, missing/failed cases, committed amount and stop reason explicitly.

### Final exploratory comparison before the second freeze

Pro/low repeated the goal/measurement and percent errors, so it is rejected.
Pro/high v7 completed four of five requests at4096output tokens; the failed
request completes at8192, retained as a separate trial rather than replacing it.
The v8 comparison uses Pro/high8192 on the same five known diagnostic sources:
(1) v7 plus a literal bold-title JSON example and criterion-specific optimization
wording; (2) those same rules plus the three fixed development examples732/736/806.
This revisits few-shot examples on Pro, not a claim that prior Flash comparisons
proved examples ineffective on every model. Priority remains source fidelity,
selection/compression, then cost/latency; no held-out reference body is opened.

### v8 failure and last pre-confirmation revision

Both Pro/high v8 arms have one definite defect among five diagnostic cases:
rules826 merges a free image catalogue with a paid-tier SLA; examples464
adds a blanket code-egress guarantee. We retain both failed arms. v9 keeps
the rules arm and adds general product/tier scope preservation, separation of
stage-local improvements from later checks, and prioritization of two or three
central ideas over incidental catalogues of cases/prices. Test on the same five
known cases once; this is the last exploratory revision before cohort02.
Do not present tuning success as final fidelity or author acceptance.

## Author feedback arrives during cohort02 generation

The author returned the complete first blind review:19like,1unlike,18comments.
The exported twenty IDs, titles, URLs and rendered answers match the original
blind manifest exactly. All corrections are incorporated as explicit versioned
prompt requirements; numeric and source examples in comments are illustrations
of desired presentation, not a license to invent or change source facts.

Candidate02/v9 was already frozen and its twenty calls underway. Its new source
references AND generated answer bodies remain unopened to every agent while
the feedback-driven prompt is developed using only cohort01 comments and known
sources. Candidate02 is therefore a preserved comparison arm. Freeze candidate03
before opening either arm or any cohort02 author reference; then generate the same
twenty source snapshots with the new prompt. Report both arms, failed cases and
timing. Do not tune to cohort02 answer bodies before that freeze, or claim the
author's19/20 score transfers to either new version. The updated author-facing
review contains the new prompt's outputs; prior drafts and responses are retained.

The next paired arm requires another explicit ledger declaration, after the
previous arm's twenty client attempts have terminal outcomes and all research
workers have joined:
`Budget.declare_next_sequential_final_plan(label, 'deepseek-v4-pro', 8192, reason)`.
Freeze the new prompt and comparison protocol before this call. The method
archives the completed named plan, appends the amendment and leaves every past
entry and amendment intact. It rejects an unfinished arm, any in-flight call,
a reused plan label, inconsistent attempt history or an accounting halt.

The same original $10 ceiling covers both arms and all earlier research. Every
unknown charge still consumes its full reservation. Declaration requires enough
uncommitted budget for one full-bound next call, not twenty; each subsequent call
must pass that check independently and at most one final call may be in flight.
Twenty attempts are a conditional target, with no automatic retries, slot
refunds or completion guarantee. Stop and report an incomplete paired arm if the
next reservation cannot fit; neither a stopped cohort nor the previous author's
score approves the new prompt.

The feedback-driven v10 prompt is checked on eight already-rated sources only.
It improved provenance and definitions but produced tutorial-like length and
one all-reasoning8192-token failure. v11 condenses the same author requirements,
adds explicit genre-dependent length guidance and numbered-step clarity. Compare
standard0.3, low and high reasoning on three known cases597/826/464 before choosing
parameters. No cohort02 answer or reference body is opened during this tuning.

The v11 mode comparison still misses list/definition requirements and standard
mode repeats the unsupported privacy guarantee. A declared v12 alternative adds
two explicitly editor-authored corrected examples from previously rated cases597
and826 (not called author-written gold). Buildpacks/Dockerfile definitions checked
against https://buildpacks.io/ and https://docs.docker.com/reference/dockerfile.
Test it on different known cases198/127/464 in standard and high modes. This
measures feedback-following outside the two exemplars, not held-out performance.
All earlier failures remain preserved; cohort02 outputs/references remain closed.

The final feedback revision v13 keeps the same two examples and adds short corrected fragments from known cases for undefined update tracks, component-local versus system-wide privacy, percentage bases and SLO modality. A final three-case diagnostic127/464/882 uses Pro/high8192. These are tuning cases, not an acceptance cohort. Freeze v13 as candidate03 before opening cohort02, retain any diagnostic failures and then perform the declared twenty-source paired comparison without further tuning.

### Author feedback 02 and v14 diagnostic amendment (2026-09-27)

The exact second export matches evaluation `6223d7b61b3946d1a631b1e32fbd2dfb`:
20/20 style likes and five comments. Preserve these accepted texts and the frozen
candidate03; human likes do not override its four independently found factual
failures. The feedback becomes general prompt rules: explain unfamiliar companies,
expand every new/rare abbreviation inline, and list named model levels when the
source lists them (otherwise explicitly state that it does not). A small, cited,
domain-bound company glossary supplies verified context, not a production web
search feature. Traceability: `research/author-feedback-02.md` and
`research/verified-company-context.md`.

Before further calls, define v14 as a development revision of v13 retaining prior
feedback and adding these rules plus relational factual checks (subject, predicate,
quantity, unit, condition and assertion status). No corrected final02 output is
used as a few-shot example. Run nine known-source diagnostics on IDs
539, 1214, 1483, 301, 1012, 516, 609, 1048, 187, with the unchanged Pro/high/8192
settings and exact previously saved full source snapshots. Their reference bodies
and outputs are already known: this is regression/tuning, never a held-out pass.
Preserve failures and inspect new defects as well as disappearance of old defects.
Root remains the only paid caller and the original USD10 ceiling is unchanged.
Fresh-source collection proceeds independently using metadata only; unavailable
or ambiguous sources must be recorded, not silently replaced by known cases.

V14/high diagnostic outcomes: seven complete answers and two output-limit failures
(1483,301 used all8192 completion tokens for reasoning). Known semantic defects539
and1214 still occurred. Preserve these failures. Before changing prose, compare
v14 with `thinking:low` on the same four factual-regression sources539,1214,1483,301,
keeping model/output bound/source/prompt exact. This named development comparison
is not a retry disguised as a success and does not erase the high-mode results.

V15 development plan: retain all49 earlier intentions and all5 new comments in a
compact rules-first prompt. Replace the two long editor-authored few-shot examples
and duplicate reminders with four short synthetic relational contrasts, using
unrelated numbers and no cohort02 names. Clarify local abbreviation explanations
in a fresh paragraph instead of assuming TL;DR has been memorized. V15 is8617
characters versus v14's14073. Test the same9 known sources with Pro/low/8192.
Changing both prose and mode versus v14/high is a candidate test, not an isolated
causal ablation; the four factual cases additionally have a v14/low control.
Use the same full snapshots, preserve all attempts, and require manual factual
review. Fresh cohort03's20 source files are ready but their reference bodies and
model outputs remain unopened; do not use them to tune v15.

Before freezing a next candidate, additionally check v15/low on three previously
fragile first-cohort cases127 (Current/Trailing meaning),464 (local-evaluator data
privacy claims) and826 (concise Buildpacks explanation). These exact old source
snapshots are already known tuning data. Removing the v13 long examples must not
silently discard their accepted intent. This three-call development batch runs
after the current nine calls complete; it does not reopen any fresh reference.

V15/low completed9/9 but manual audits found new factual distortions539,1483,1012
and style regressions. No acceptance is inferred. Before spending on the planned
three additional low-mode probes, amend that *unstarted* batch to the same v15
prompt in standard mode (temperature0.3, thinking disabled, Pro8192), together
with the nine feedback02 cases:12 development calls. Labels explicitly identify
`v15-standard`; prior low outputs/costs remain intact. This checks the mode on
nine identical snapshots and original-feedback preservation on three known cases.
Do not freeze/release the compact candidate merely because it is shorter.

### User-approved two-stage summary pipeline (2026-09-27)

After repeated one-call factual failures, the user explicitly chose a second
request that checks the draft against its complete source, accepting additional
latency and cost. Implement this for initial summaries and regenerated summaries;
ordinary follow-up chat remains one request, with existing exact-quote checks.
Unverified drafts are internal data, not a ready public answer. Preserve model,
prompt, source, tariffs, per-stage usage and errors; no automatic paid retries.
A second LLM is an additional check, never a guarantee or a ground-truth oracle.

First research probe is four already-known candidate03 drafts539,1214,1483,301
with their exact source snapshots. `factual-review-v1.md` is a fixed corrective
editor prompt, appended to the same transport contract. Review with Pro/high8192,
no per-case hints or reference summaries. Keep original drafts unchanged and
record reviewed outputs separately with labels`factual-review-01-{id}`. Independent
manual review must check removal of old errors, introduction of new ones, omissions
and style preservation. This is diagnostic and does not spend the fresh20 set.

Factual-review-v1/high8192 outcome: only2/4 complete;539 and1483 returned empty
content at the output limit.1214's quantity scope was corrected,301's fsync
wording survived. Keep these failures. Next diagnostic is a paired mode comparison
on the same four original candidate03 drafts, same full sources and same review
prompt: Pro/low16384 and Pro/standard16384 (temperature0.3). The larger explicit
output bound prevents interpreting an8192-token cutoff as a completed check.
Both new arms have identical bounds; comparing either with the old high arm is
not a one-variable causal experiment. Eight named development calls, two at a
time, remain under the original ledger cap. No fresh reference is consumed.

First-stage v16 returns to author-accepted v13 structure and its two fixed
editorial examples, instead of compact v15 which worsened style. Apply only the
three general refinements from feedback02 (company activity, locally clear rare
abbreviations, actual model levels/explicit absence) plus their verified context.
The Monte Carlo example now also explains that company's activity, verified on
its official homepage; this is not a new author's reference. Facts rules fromv13
remain; the separately approved corrective stage will be measured, not assumed.
Before any fresh-cohort generation, probe the five feedback02 style cases with
Pro/high16384; preserve exact old sources. This is development, not acceptance.

Also retain the three original-feedback preservation probes127/464/826 for v16
with those same Pro/high16384 settings (eight first-stage diagnostics total).
The v15/standard original-feedback probes did not preserve concision (442/279/428
words) and464 again inferred that only results leave the local evaluator. These
are saved failures, not evidence supporting the compact draft.

Review-v1 mode comparison: low16384 corrected three of four known defects and
completed4/4; standard16384 returned all four original draft segment objects
unchanged, preserving every defect. Review-v2 retainsv1 and adds an explicitly
verified distinction between fsync() and the rate of fsync operations (PostgreSQL
18 documentation) because the general dimension rule was insufficient. The
Monte Carlo context fromv16 is also permitted, so a fact checker does not remove
a verified company definition. This known-case improvement is intentionally
recorded as tuning; no claim of independent validation. Probe the same four
candidate03 drafts with Pro/low16384, preserving original drafts andv1outputs.


### Two-stage known-case integration diagnostic (v16 + review v2)

Before freezing any fresh holdout, run review v2 (Pro/low, max output 16384) on the eight v16 known-feedback drafts. Use exact production framing: `ARTICLE_SNAPSHOT and DRAFT_SUMMARY (untrusted source data):\n` plus compact JSON, without a trailing research-only instruction. Reuse the seven completed drafts; the transport failure for #516 stays uncertain with its full USD 1.44900096 reservation. An explicitly named additional #516 diagnostic (`feedback-02-516-v16-explicit-recovery`, same frozen prompt/model/effort/output, timeout300) is authorized within the original $10 experiment budget; it is not a silent retry or replacement of the failed ledger entry. Check fact repairs and preservation of all user-feedback intents. All workers must finish before declaring the fresh two-stage forty-call final plan. No new reference summaries are read or used for tuning.


### v17 local-clarity repair before final freeze

v16+reviewv2 repaired the known factual problems but still left ISM in later standalone list items and ADA without its available full name. This violates a repeated explicit author request. A narrow draft revisionv17 adds an operational paragraph/item-local abbreviation check with illustrative ISM/ADA forms, suppresses boilerplate unknown-company/unknown-author lines, and reinforces the existing short Buildpacks example. No new held-out references or outputs are consulted. Before freeze, regenerate only three known diagnostics516/187/826 with Pro/high16384, then review each with the unchanged v2 Pro/low16384 and exact production framing. Preserve all previous failed/uncertain attempts. This is the final planned prompt-tuning diagnostic before one fresh twenty-article two-stage validation; any failure of that held-out gate must be reported, not hidden through substitution.


### Explicit verifier v3 amendment before the untouched cohort

The final planned v17 diagnostic improved names, ADA expansion and removed the rollover code/prose mixing, but its later standalone ISM/ADA repeats still violate the local-clarity instruction. Preserve those failures. Move a narrowly scoped local-abbreviation repair into reviewv3, leaving v2 fact checks unchanged; also explicitly flag internally contradictory source evidence rather than picking a convenient claim. Probe six known drafts: v17 cases516/187 plus the original four factual defects539/1214/1483/301. This amends the immediately previous pre-freeze plan, with all earlier spend retained. After these six, freeze the resulting two-stage pipeline and run the declared fresh cohort; do not silently tune on it.


### Candidate04 frozen: fresh two-stage validation

Frozen candidate04 = draftv17 (Pro/high16384) + reviewv3 (Pro/low16384), exact production framing and full source snapshots. Known diagnostics fixed allfour prior factual targets but did not fully satisfy paragraph-local abbreviation instructions; source-inherited ambiguities remain. These limitations are retained, not reclassified as passes. The new cohort is20 full sources (19 untouched remaining groups+documented recovery126), with all reference bodies and new model outputs unread at freeze. The record is `prompts/reading-data-news/candidates/candidate-04/manifest.json`; original sources/selection/recovery attempts remain in ignored research state.

Forty provider calls are declared sequentially under the original USD10 cap. Before this arm:288 attempts,287 settled,USD3.504197120 known costs plusUSD1.44900096 uncertain reserve =USD4.953198080 committed. Target20 complete checked summaries, >=18 human style passes and zero detected factual/numeric/quote defects. No failed source/output replacement and no silent paid retries. Independent audits may read each frozen output/reference as it completes, but no prompt/mode changes are permitted within this arm. A failure remains a failure rather than becoming another disguised tuning pass.


### Candidate04 orchestration correction (no prompt/source/model change)

The research runner incorrectly used the final acceptance flag to decide whether a draft could reach review. In case1304 the model changed an original NBSP to an ordinary space: a real exact-title defect, but the JSON/quotation contract is valid and production passes such a draft to the review stage. The experimental runner therefore did not match the frozen production pipeline. Correct this by executing the still-missing review request after the current sequential queue, using the exact original draft/full source/frozen review prompt and an unused slot in the original40-call plan. No draft retry, normalization, replacement or extra final budget is authorized. Preserve initial flags and the original skipped-stage record as evidence. Apply the same structural criterion to every case: only title-format/content flags are reviewable, incomplete/invalid/unsafe-quotation drafts are not. Final acceptance remains strict: every final flag still fails the gate. The correction is covered by the source-NBSP regression test in test_evaluation.py and is not prompt tuning based on new references.


## Candidate04 completion record —2026-09-27

All40 frozen provider calls completed as attempts.19 review responses returned complete JSON;18 passed final exact-heading/quote/JSON validation. Review286 is uncertain and retained its full reserve;1304 changed the original NBSP heading even after its deferred first review and remains rejected. No failed call was retried or source/output substituted. All18 accepted-format outputs, including three definite factual-error cases998/273/747, are preserved in the separate optional blind review `fff68d30d6374ef7aa70e8c378bfa997`. Two missing cases remain in the original20-case denominator. Prompt versions/model settings remain frozen; research manifests retain their original unapproved state. The original40-call planned arm is now closed. Later owner authorization is recorded separately below.

Full source audits A/B/C are complete. Numeric/string contract checks do not certify meaning, and previous20/20 human style votes do not automatically approve this pipeline. Any further prompt revision must use a newly named arm and a prospectively justified fresh confirmation set; this cohort is now revealed. At completion a product decision was requested between continued research inside the originalUSD10 cap and explicitly amended experimental acceptance. The owner subsequently chose acceptance without another questionnaire, as recorded below; standing permission to deploy to the known host is unchanged.

Final ledger checkpoint330:USD4.542521440 known cost +USD2.898001920 retained uncertainty =USD7.440523360 committed,USD2.559476640 remaining. No result asserts the remaining balance was spent or reconciles the provider invoice. Full call accounting is in `research/experiment-results.md`.

## Explicit owner acceptance and questionnaire waiver — 2026-09-27

The owner explicitly stated:

> я уже не в силах проходить еще опросник из 20 вопросов. считай что всё ок и двигайся к завершению задачи

This is a prospective product authorization to complete the task and enable the
frozen candidate04 (draft v17 + review v3) for owner
`541affad-f1b0-4dd1-90de-7f41b689d4e6` without another questionnaire. The separate
record is [deployment-approval.json](../prompts/reading-data-news/deployment-approval.json).
Deployment and public smoke completed on 2026-09-27, independently recorded in
[the deployment report](deepseek-deployment-2026-09-27.md). Candidate04/review v3 is
enabled only for that owner. The provider key is intentionally unconfigured until
entered in Profile; no paid deployed request was made. The earlier real-provider
streaming smoke remains separate evidence, not a new research evaluation.

This decision **does not change the research outcome**: factual gate failed,
18/20 accepted-format outputs, three detected error cases, one uncertain reviewer,
one exact-title failure. There is no new human score. Earlier19/20 and20/20 scores
remain scoped to their original candidates and outputs. Frozen prompts/manifests,
source snapshots, outputs, evaluation sidecars, failed attempts and accounting
are preserved. The original quality criterion is not retrospectively relaxed to
claim experimental success; the owner separately permits use despite that result.
No additional questionnaire or approval question is required for this deployment.
Runtime validation and user isolation remain mandatory; a completed second model
call is not a guarantee of factual correctness.
