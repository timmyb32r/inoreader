# Recorded prompt experiments

Descriptive single-run measurements; neither causal superiority nor factual correctness follows from these counts. All responses, including failures, are retained in ignored research state. The label contains the exact version; parameters and effective thinking mode are in each immutable request.

## Historical checkpoint: first 102 calls

The original table below is retained unchanged. Its 22 groups reconcile exactly to the first 102 budget entries: **USD 0.198808812**, including the incomplete baseline. The `rules-v6` development row includes five `dev-*` and six `tuning-*` calls after the former validation references became tuning material; it does not include the later five `post-eval-*-rules-v6-thinking` calls.

| Phase | Model / variant | N | Median body words | Median seconds | Conservative USD |
|---|---|---:|---:|---:|---:|
| development | deepseek-flash / baseline | 1 | invalid/incomplete | 16.641 | 0.0062568 |
| development | deepseek-flash / rules-v1 | 1 | 1277 | 11.829 | 0.005439 |
| development | deepseek-flash / examples-v1 | 1 | 1092 | 10.795 | 0.00476268 |
| development | deepseek-flash / rules-v2 | 5 | 341 | 4.371 | 0.009958008 |
| development | deepseek-v4-pro / rules-v2-pro | 1 | 254 | 7.663 | 0.01049004 |
| development | deepseek-flash / baseline-concise | 5 | 342 | 3.691 | 0.009659088 |
| development | deepseek-flash / examples-v2 | 5 | 296 | 3.914 | 0.009335016 |
| development | deepseek-flash / rules-v3 | 1 | 222 | 3.248 | 0.0021687 |
| development | deepseek-flash / baseline-brief | 1 | 427 | 5.444 | 0.0027762 |
| development | deepseek-flash / examples-v3 | 1 | 203 | 2.981 | 0.0022971 |
| development | deepseek-flash / minimal-probe | 1 | 90 | 2.045 | 0.0016152 |
| development | deepseek-flash / examples-v4 | 5 | 190 | 2.929 | 0.00781242 |
| development | deepseek-flash / rules-v4 | 5 | 264 | 3.319 | 0.00833826 |
| development | deepseek-flash / baseline-paragraph | 5 | 175 | 2.632 | 0.007171788 |
| validation | deepseek-flash / baseline-paragraph | 6 | 183.0 | 3.063 | 0.011204832 |
| validation | deepseek-flash / rules-v4 | 6 | 212.5 | 3.141 | 0.01146024 |
| validation | deepseek-flash / examples-v4 | 6 | 181.0 | 2.894 | 0.01096368 |
| development | deepseek-flash / rules-v5 | 5 | 112 | 2.402 | 0.006110688 |
| development | deepseek-flash / baseline-compact | 5 | 105 | 2.104 | 0.006183012 |
| development | deepseek-flash / rules-v6 | 11 | 93 | 1.868 | 0.016182576 |
| development | deepseek-flash / rules-v5-thinking | 5 | 147 | 4.543 | 0.011300844 |
| final | deepseek-flash / candidate-01 | 20 | 102.0 | 2.107 | 0.037322640 |

Word counts exclude the first output line, ordinarily the title; they include any generated quotes and supplemental paragraphs. Invalid/truncated JSON has no complete rendered body and is not assigned a flattering zero-length score. Median latency includes HTTP completion for these requests only. Numeric source scans are review flags, not factual verdicts.

Candidate01 is unapproved. See [summary report](summary-report.md) and the independent development, validation and final source reviews for semantic failures that transport checks do not catch.

## Subsequent experiments: calls 103–209

Accounting checkpoint: final entry completed at **2026-09-26T22:20:09.880831+00:00**. Source of truth: `.inoreader-state/style-research/experiments/budget.json`; each entry's immutable `request.json`, `metadata.json`, `response.json` and existing `evaluation.json` were checked read-only. There were **209 settled entries**, no outstanding reservations, **204 `finish_reason=stop`** responses and **5 `finish_reason=length`** responses. `Settled` means that the request's usage/cost has been accounted for, not that its output is complete, correct or accepted.

The table groups every additional request exactly once. Labels beginning `post-eval-{id}-` and `feedback-01-{id}-` are grouped by their remaining suffix; the two new final arms retain their separate candidate names. Their actual labels both begin `final-02-`: `final-02-{id}-candidate-02` and `final-02-{id}-candidate-03`.

`Automatic pass / fail / missing` refers only to the saved evaluator's `complete` field. It checks the response/format contract, not factual truth. A failed bold-title requirement is not a truncated HTTP response; missing evaluation files are neither passes nor failures invented after the fact. `Length` and the evaluator columns overlap and must not be added together.

| Group | Request model, thinking, max tokens | Calls | Stop | Length | Automatic pass / fail / missing | Conservative USD |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| post-eval rules-v6-thinking | Flash, high, 4096 | 5 | 5 | 0 | 4 / 1 / 0 | 0.013755144 |
| rules-v7-none | Flash, disabled, 4096 | 5 | 5 | 0 | 2 / 3 / 0 | 0.010533504 |
| rules-v7-high | Flash, high, 4096 | 5 | 5 | 0 | 2 / 3 / 0 | 0.018727404 |
| rules-v7-pro-high | Pro, high, 4096 | 5 | 4 | 1 | 0 / 5 / 0 | 0.082097136 |
| rules-v7-pro-low-8192 | Pro, low, 8192 | 5 | 5 | 0 | 0 / 5 / 0 | 0.066596992 |
| rules-v7-pro-high-8192 | Pro, high, 8192 | 1 | 1 | 0 | 0 / 1 / 0 | 0.020825640 |
| rules-v8-pro-high | Pro, high, 8192 | 5 | 5 | 0 | 5 / 0 / 0 | 0.068765576 |
| examples-v8-pro-high | Pro, high, 8192 | 5 | 5 | 0 | 5 / 0 / 0 | 0.071201680 |
| rules-v9-pro-high | Pro, high, 8192 | 5 | 5 | 0 | 5 / 0 / 0 | 0.063845584 |
| final candidate-02, rules-v9 | Pro, high, 8192 | 20 | 20 | 0 | 20 / 0 / 0 | 0.248399800 |
| feedback v10 | Pro, high, 8192 | 8 | 5 | 3 | 2 / 1 / 5 | 0.247028320 |
| feedback v11-low | Pro, low, 8192 | 3 | 3 | 0 | 3 / 0 / 0 | 0.042693200 |
| feedback v11-none | Pro, disabled, 8192 | 3 | 3 | 0 | 3 / 0 / 0 | 0.020399632 |
| feedback v11-high | Pro, high, 8192 | 3 | 3 | 0 | 2 / 0 / 1 | 0.056335664 |
| feedback v12-none | Pro, disabled, 8192 | 3 | 3 | 0 | 2 / 0 / 1 | 0.021800944 |
| feedback v12-high | Pro, high, 8192 | 3 | 3 | 0 | 0 / 0 / 3 | 0.061472224 |
| feedback v13 diagnostics | Pro, high, 8192 | 3 | 3 | 0 | 3 / 0 / 0 | 0.059908288 |
| final candidate-03, feedback-v13 | Pro, high, 8192 | 20 | 20 | 0 | 20 / 0 / 0 | 0.403339200 |
| **Subsequent subtotal** | | **107** | **103** | **4** | **78 / 19 / 10** | **1.577725932** |
| **Historical checkpoint above** | | **102** | **101** | **1** | **94 / 8 / 0** | **0.198808812** |
| **All calls** | | **209** | **204** | **5** | **172 / 27 / 10** | **1.776534744** |

Flash means the recorded `deepseek-flash` model identifier; Pro means `deepseek-v4-pro`. High/low requests have `thinking.type=enabled` and the stated `reasoning_effort`; disabled requests do not have that effort field. Model settings were read from the request files, not inferred solely from labels.

The later development diagnostics reuse already revealed difficult examples and author feedback. They are tuning measurements, not fresh independent validation. Candidate-02 and candidate-03 were frozen before the shared second cohort's outputs/reference bodies were opened, then evaluated as paired results; this same cohort cannot be reused as an untouched holdout for a future prompt change.

### Incomplete responses and incomplete evaluation coverage

All five length-limited responses remain in the spend and failure history:

| Label | Run ID | Conservative USD | Saved automatic evaluation |
| --- | --- | ---: | --- |
| `dev-541-baseline` | `39041226-aae2-4470-b3b3-4c30eb3d3394` | 0.006256800 | `incomplete_response_length`, `invalid_json` |
| `post-eval-1107-rules-v7-pro-high` | `5a3294f1-49d6-4a26-a808-e43fd15c861b` | 0.018832440 | `incomplete_response_length`, `invalid_json` |
| `feedback-01-597-v10` | `d7c0c0f6-08b8-4330-806c-937be33277d1` | 0.039222480 | `incomplete_response_length`, `invalid_json` |
| `feedback-01-464-v10` | `af1fdeae-9854-4f2f-a2ff-2ffff8db2764` | 0.034081520 | Missing; response `finish_reason=length` is recorded |
| `feedback-01-299-v10` | `74ff28d4-8fb9-4369-a6e6-96fad75f3c73` | 0.040577240 | Missing; response `finish_reason=length` is recorded |
| **Total retained incomplete spend** | | **0.138970480** | |

Of the 27 saved evaluator failures, 24 contain `title_format_not_bold` and three contain the incomplete/invalid-JSON flags above. The other two length-limited outputs have no saved evaluation, so they are among the ten `missing` entries, not hidden as successes.

The remaining eight entries without a saved `evaluation.json` are `feedback-01-{882,826,127}-v10`, `feedback-01-464-v11-high`, `feedback-01-{198,127,464}-v12-high`, and `feedback-01-464-v12-none`. They returned `stop`; that alone does not establish contract or factual acceptance. This accounting update did not create or replace evaluations, retry calls, alter output text, or revise the budget.

### Reconciliation

The 22 historical table groups were independently matched to labels/model/phase among entries 1–102, including the six `tuning-*` entries. Each count and each Decimal sum matches its retained row. For all 209 entries, metadata labels, request model IDs and response finish reasons match the budget entry. Summation used Python `decimal.Decimal` from stored decimal strings, not floating-point rounding.

```text
102 historical calls + 107 subsequent calls = 209
0.198808812 + 1.577725932 = USD 1.776534744

development: 131 calls, USD 1.053844352
validation:   18 calls, USD 0.033628752
final:        60 calls, USD 0.689061640
                           1.776534744
```

The total includes all three final arms: candidate-01 **USD 0.037322640**, candidate-02 **USD 0.248399800**, candidate-03 **USD 0.403339200**. Under the original USD10 experimental cap, the conservative accounted remainder is **USD 8.223465256**. These are usage-based conservative costs from the research ledger, not a claim that an external billing invoice or live account balance was independently reconciled.

### Candidate-03 outcome at this checkpoint

Both new arms have **20/20 complete JSON/title/transport-contract evaluations**. Candidate-03 also has a real source line and explicit TL;DR in **20/20** reviewed outputs. The latter is formatting/content structure evidence, not a factual verdict or author-style score.

| Arm | Calls | Median body words | Median HTTP seconds | Maximum HTTP seconds | Conservative USD |
| --- | ---: | ---: | ---: | ---: | ---: |
| candidate-02 | 20 | 95.5 | 14.356 | 25.688 | 0.248399800 |
| candidate-03 | 20 | 240.0 | 35.820 | 51.694 | 0.403339200 |

Body-word counts here use the existing evaluator's convention, and timings use immutable completion metadata. One run on twenty sources is not a production latency guarantee or a statistical proof of superiority.

**Candidate-03 factual gate failed.** Four reviewed cases contain definite defects: #539 reverses the EFO/SLO claim and changes the scope of an API metric; #301 defines `fsync` as a rate; #1214 conflates about 100 AWS accounts with hundreds of Terraform repositories; #1483 changes 10 GB to 10 GiB. Candidate-02 has a definite mixed-benchmark-conditions error in #683. Counts describe detected cases, not the true error rate. The improved structure and correction of #683 do not cancel the new defects.

See the complete paired audits [A](paired-cohort02-a.md), [B](paired-cohort02-b.md), [C](paired-cohort02-root.md), [factual cross-check](candidate03-factual-crosscheck.md), and [summary report](summary-report.md). **Human assessment of candidate-03 is complete: 20/20 likes and five comments, exactly matched to the frozen review; see [feedback02](author-feedback-02.md). The factual gate still fails.** The old candidate-01 feedback is retained separately and must not be transferred as acceptance of candidate-03. No frozen prompt was changed in this accounting task.

## Feedback02 diagnostics: calls210–222

These are known-source development comparisons after the second human review,
not another held-out set. Every failed attempt remains in the ledger.

| Arm | Model/mode/output bound | Calls | Complete transport | Cost USD | Median seconds |
|---|---|---:|---:|---:|---:|
| v14 | Pro/high/8192 | 9 | 7 | 0.265890152 | 38.272 |
| v14 mode comparison | Pro/low/8192 | 4 | 4 | 0.092150168 | 35.557 |

High-mode cases1483 and301 exhausted all8192 completion tokens on reasoning and
returned no usable JSON. The high-mode set retained factual errors539/1214;
low mode avoided incomplete output but did not establish factual acceptance.
See [factual audit](author-feedback-v14-facts.md) and
[style audit](author-feedback-v14-style.md). Labels are
`feedback-02-{post_id}-v14` and `feedback-02-{post_id}-v14-low`.

Total at this checkpoint: **222 settled calls, USD2.134575064, zero unresolved
charges** under the original USD10 cap. “Settled” is an accounting status and
includes all seven incomplete outputs accumulated across the study. Per-run
IDs, costs, completion flags and times are retained in
`feedback02-v14-metrics.json` and `feedback02-v14-low-metrics.json` alongside the
immutable requests/responses in the ignored research state.


## Second-stage selection checkpoint: calls223–267

Known-case diagnostics only; none of the third cohort’s reference bodies have been read. The author approved a second source-verification request for initial/regenerated summaries.

| Arm (all Pro) | Calls | Complete transport | Usage-based USD | Unresolved reserve USD | Median completed HTTP seconds |
|---|---:|---:|---:|---:|---:|
| v15 low/8192 | 9 | 9 | 0.211853136 | 0 | 43.689 |
| v15 disabled/8192 | 12 | 12 | 0.059284632 | 0 | 8.337 |
| review v1 high/8192 | 4 | 2 | 0.150393408 | 0 | 72.969 |
| review v1 low/16384 | 4 | 4 | 0.124366176 | 0 | 39.118 |
| review v1 disabled/16384 | 4 | 4 | 0.019906040 | 0 | 5.063 |
| v16 high/16384 | 8 | 7 | 0.151630600 | 1.44900096 | 32.396 |
| review v2 low/16384 | 4 | 4 | 0.10586488 | 0 | 37.567 |

At this immutable checkpoint: **267 attempts, 266 settled accounting outcomes, USD2.957873936 usage-based cost plus USD1.44900096 retained uncertainty = USD4.406874896 committed** under the original USD10 cap.

Review v1 high exhausted8192 output tokens on two cases; disabled reasoning returned all four defective drafts unchanged. Review v2 low completed and corrected all four targeted defects, but this is tuning on known failures, not a fresh factual result. See [v1](factual-review-v1-diagnostics.md), [v2](factual-review-v2-diagnostics.md), [v15 facts](author-feedback-v15-facts.md), [v15 style](author-feedback-v15-style.md), [v16 feedback](feedback-v16-diagnostics.md), [v16 preservation](author-feedback-v16-preservation.md).

The v16 #516 transport failure remains an uncertain charged attempt with its full-context reservation; a later explicitly labelled recovery is additional spending and will appear in the next checkpoint. No invisible replacement or cost release occurred. Automatic evaluations check JSON/title/quotes; they do not establish factual truth or author approval.

## Known-case pipeline reconciliation: calls 268–288

This checkpoint ends at **2026-09-26T23:33:29.499150+00:00**, the completion of call288. Call numbers are one-based positions in the research ledger's retained `entries` array. The scope is exactly entries268–288: **21 additional calls**, excluding subsequent candidate04/fresh-cohort requests and streaming smoke tests. Earlier tables and checkpoints remain historical records.

Every one of these21 entries is `settled`, has `finish_reason=stop`, a saved response and a saved automatic evaluation with `complete=true` and no flags. There are **no newly incomplete or uncertain calls in this interval**. These are transport/format outcomes; known-case semantic findings below still apply. `response.usage` was matched exactly to the ledger, labels to metadata, and model/thinking/output settings to immutable requests. Reviewer prompts were verified against v2/v3 rather than inferred only from the label suffix.

All requests use recorded model `deepseek-v4-pro`, thinking enabled and `max_tokens=16384`. Draft generation uses high effort; reviewer calls use low. Rows marked review charge **only that second request**, not its earlier draft. The original candidate03 drafts in the last row are reused unchanged and are not charged again here.

| Group and entry numbers | Effort | Calls | Stop / automatic complete | Prompt tokens | Completion tokens | Usage-based USD | Median HTTP seconds | Maximum HTTP seconds |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| v16 #516 explicit recovery,268 | high | 1 | 1 / 1 | 6,930 | 3,465 | 0.014049288 | 37.001 | 37.001 |
| Review v2 of eight v16 drafts,269–276 | low | 8 | 8 / 8 | 30,722 | 38,760 | 0.180976400 | 35.233 | 83.162 |
| v17 drafts #187/#516/#826,277/278/281 | high | 3 | 3 / 3 | 20,814 | 14,267 | 0.081358552 | 33.552 | 46.837 |
| Review v2 of those three v17 drafts,279/280/282 | low | 3 | 3 / 3 | 12,854 | 12,535 | 0.061706040 | 28.867 | 31.916 |
| Review v3 of unchanged v17 #516/#187 drafts,283–284 | low | 2 | 2 / 2 | 10,675 | 11,766 | 0.059051080 | 61.847 | 63.377 |
| Review v3 of original candidate03 #539/#1214/#1483/#301 drafts,285–288 | low | 4 | 4 / 4 | 31,865 | 28,989 | 0.149181824 | 75.922 | 96.677 |
| **Interval subtotal** | | **21** | **21 / 21** | **113,860** | **109,782** | **0.546323184** | **41.206** | **96.677** |

Completion-token counts include provider-reported reasoning tokens; they are not lengths of the visible summaries. No reasoning content was inspected. Total reported tokens for this interval are **223,642**. Latencies are immutable per-request HTTP completion durations; the table does not sum parallel wall time or claim an end-to-end two-stage production latency. Small known-case groups, cache differences and different inputs prevent interpreting these medians as a controlled v2-versus-v3 speed comparison.

### Exact accounting and unresolved history

Costs were independently recomputed for all21 requests with `decimal.Decimal`, using saved usage and the ledger's recorded Pro rates per million tokens: cache-hit input `$0.044`, cache-miss input `$1.32`, completion `$3.96`. For every request, cache hit + miss equals prompt tokens, prompt + completion equals total tokens, and the recomputed charge equals `cost_usd` exactly. This reconciles the usage-based research estimate, not an external invoice or account balance.

```text
267 previous attempts + 21 new attempts = 288 attempts
266 previous settled + 21 new settled = 287 settled
1 previous uncertain + 0 new uncertain = 1 uncertain

USD 2.957873936 previous usage-based cost
  + 0.546323184 interval usage-based cost
  = 3.504197120 usage-based cost through call288

USD 3.504197120 known cost
  + 1.449000960 retained unknown-charge reserve
  = 4.953198080 committed at this checkpoint

USD10 cap - 4.953198080 = USD5.046801920 then uncommitted
```

The remaining reserve belongs to earlier v16 #516 run `a30b10fd-1f3e-433b-8d39-2d4dc51509f0`, whose provider charge is still unknown. Call268, `feedback-02-516-v16-explicit-recovery`, run `4c7369bc-b2ba-4ede-86e6-4f911cdfe2be`, is an additional explicit attempt costing **$0.014049288**; it neither replaces the failed request nor proves the old charge was zero. A `reserved_usd` field retained on an already settled entry is historical reservation metadata and is **not added again** to its known cost.

Through call288 there are **278 stop responses, nine length-limited responses and one uncertain attempt without a completed response**. All nine earlier incomplete outputs remain charged and incomplete; the21 new complete responses do not erase them. The figures above are a bounded historical checkpoint, not the current ledger total after later research activity.

### What the known-case results establish

The eight v16→review-v2 calls produced useful corrections: #609's table reader no longer becomes its owner; #1012's `a top challenge` is no longer a uniquely leading problem. Several already usable drafts were preserved. But #516 still repeats a contradictory prose statement about a two-minute rollover, despite source code assigning that interval to the warm transition; #187 still lacks full local ADA expansion and the requested name format. See the independent [v16 audit A](two-stage-known-v16-a.md) and [audit B](two-stage-known-v16-b.md). Eight complete reviewer responses are therefore not eight unrestricted factual/style passes.

The three v17 drafts and their v2 reviews are additional known-source development probes. The two v3 reviews then reuse unchanged v17 drafts #516/#187. In those two cases the key numbers and engineering content were preserved, but repeated ISM/ADA were still left unexplained locally despite the explicit v3 rule. #516's code-consistent choice did not disclose the prose/code conflict; #187 lost two useful explanatory definitions. These limitations are documented in [v17 + v3 audit A](final-pipeline-known-a.md), and must not be hidden behind their2/2 automatic-complete result.

The last four v3 calls reused the original candidate03 defective drafts, not the newer v17 outputs. The independent [audit B](final-pipeline-known-b.md) found all four targeted error groups corrected: EFO/SLO direction and API ownership in #539, AWS-account/repository quantities in #1214, GB/GiB in #1483, and fsync-operation versus rate in #301. It did not find a newly introduced definite distortion, while retaining the source's memory-denominator ambiguity in #1483 and limitations in terminology and length. Correcting these four known drafts is useful diagnostic evidence; it does not estimate performance on untouched articles or transfer the earlier20 author likes to a new pipeline.

This accounting update changed only this report. It did not retry a request, edit a prompt, modify any saved input/output/evaluation, release an uncertainty reserve, include later fresh/streaming calls, or declare a prompt accepted or deployed.
## Completed two-stage arm and streaming smoke: calls289–330

This section records the completed candidate04 arm, frozen before cohort03
outputs/reference bodies were opened. All40 planned calls were attempted:
20 drafts (Pro/high/16384), then20 source reviews (Pro/low/16384). The two
production-adapter smoke calls used the same frozen settings on a known source;
they are engineering evidence, not additional fresh cases.

| Group | Attempts | Usage settled | Known USD | Retained uncertain USD | Median completed HTTP seconds |
|---|---:|---:|---:|---:|---:|
| Fresh draft | 20 | 20 | 0.484186120 | 0 | 53.478 |
| Fresh source review | 20 | 19 | 0.507793264 | 1.449000960 | 51.298 |
| Production streaming draft smoke | 1 | 1 | 0.014484800 | 0 | 30.648 |
| Production streaming review smoke | 1 | 1 | 0.031860136 | 0 | 62.738 |
| Subsequent subtotal | 42 | 41 | 1.038324320 | 1.449000960 | — |

Final accounting checkpoint: **330 attempts,328 settled,2 uncertain;
USD4.542521440 usage-based cost +USD2.898001920 retained reservation
=USD7.440523360 committed**, leaving **USD2.559476640** under the original
USD10 research cap. These are ledger/tariff calculations, not a provider invoice.
All earlier incomplete outputs remain in these totals. Unknown charges retain
their original full-context reservation; they are not treated as free failures.

The uncertain calls are `feedback-02-516-v16`
(`a30b10fd-1f3e-433b-8d39-2d4dc51509f0`) and
`final-03-286-candidate-04-review`
(`8c0e9512-960d-4a9f-a983-7925eaa53687`). The latter was not retried or replaced.

**18/20 final summaries pass the JSON/exact-heading/quotation contract.** One
review has no usable response (#286); #1304 returned complete JSON but changed
the original title's U+00A0 to U+0020. That output is rejected, not silently
normalized. Its first review was deferred because the research orchestrator
incorrectly applied final title validation before review; the original draft
and failed gate remain archived. The deferred call consumed its unused slot
within the original40-call plan and used the exact frozen draft/source/prompts.
It was not a retry or an extra selected case.

The fresh factual gate fails: definite errors remain in #998 (canary rollout
object), #273 (table measurement attributed to one column), and #747 (number
of coding levels attributed to the analytics ladder). Full-source reviews:
[A](fresh-cohort03-a.md), [B](fresh-cohort03-b.md), [C](fresh-cohort03-c.md).
Formatting checks and the successful production streaming smoke do not certify
these facts. Comparing this count with candidate03's four errors does not prove
an improvement: the cohorts differ, and neither is a large random sample.

The review pages preserve every completed accepted-format output, including
the factually defective cases. Failed/missing cases remain in the planned
20-case denominator. No new human style score or accepted prompt is claimed.
