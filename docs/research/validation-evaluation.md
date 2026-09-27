# Validation: source alignment and three frozen candidates

Reviewed on 2026-09-26. This is an independent researcher's assessment, **not the
channel author's acceptance and not the final held-out test**. The reviewer knew
the arm labels; this was not a blinded review. No prompt was changed during this
review, and no final author reference was opened.

## Evidence and procedure

- Frozen plan: `.inoreader-state/style-research/validation-plan.json`, created
  `2026-09-26T20:53:33.695266+00:00`.
- Six validation references, and only these, were selected in Python before
  printing their bodies: **16, 260, 604, 669, 1210, 1152**.
- Primary sources: the six `confirmed_title_url` records in
  `.inoreader-state/style-research/pairs/validation.jsonl`. The earlier unavailable
  candidate 163 is not an evaluated case.
- Outputs: 18 `validation-{post_id}-{arm}` entries in
  `.inoreader-state/style-research/experiments/budget.json`, with the corresponding
  `response.json`, `rendered.md`, and `evaluation.json` under each experiment ID.
  Arms are `baseline-paragraph`, `rules-v4`, and `examples-v4`; model is
  `deepseek-flash`. All 18 calls finished with `stop`, not a token-limit finish.
- The full extracted prose/transcript of each source was read, including the
  middle of the Substack source in a separate read after a terminal display limit.
  This limit affected review display, not the archived source or model input.
- Every output was checked against its source for subjects, causal direction,
  values and units, attribution, quotation claims, coverage, and invented personal
  experience. Style was then compared against that case's author reference.
- Settled cost for these 18 calls: **$0.033628752**, from the experiment ledger.

All 18 outputs retain the exact source heading after removing Markdown heading
or bold delimiters. The automatic `title_changed_or_missing` flags on
`validation-16-rules-v4` and `validation-669-rules-v4` are false positives caused by
`##` headings. A different Markdown heading level is not a changed title. These
checks do not measure factual accuracy: the numeric error below passed the
automatic flags.

## Source/reference alignment

All six references clearly discuss the matched source: the original headline or
URL, entities, and several distinctive mechanisms agree. This establishes
alignment; it does **not** make every reference claim a factual gold label.

| Post | Matched source and genre | Reference-specific limitation |
| --- | --- | --- |
| 16 | [Meta storage architecture](https://engineering.fb.com/2026/07/01/data-infrastructure/metas-ai-storage-blueprint-at-scale/). Long explainer with questions, lists, analogy, and additional context. The O(1) metadata lookup, removal of the data proxy, GPU-host cache, 80% hit rate, and L1/L2/L3 hierarchy establish alignment. | The opening calls the whole AI storage system ZippyDB, while the source describes BLOB storage over Tectonic and uses ZippyDB for metadata; the reference later clarifies the distinction. The approximately 15-fold ingestion claim points to an image and is not recoverable from the extracted prose alone. RocksDB, proprietary status, and the universal claim about training needing a key-value store are additional context, not supported by this source text. They must not become requirements for a source-only answer. |
| 260 | [Data-Perspective-Action](https://practicaldatacommunity.substack.com/p/why-technically-excellent-data-teams). Short business summary followed by the channel author's personal reaction. The framework and engineer/analyst pairing agree. | The final personal opinion must not be fabricated by a model under the approved policy. Its absence is not a failure. The model can still reproduce the reference's concise explanation without copying the author's opinion. |
| 604 | [Cloudflare community program](https://blog.cloudflare.com/community-program-refresh/). News summary followed by a question-and-answer explainer of Astro, Hono, and Vite. Two program tracks, the new fund, and the Discord committee establish alignment. | Project definitions and the Astro acquisition claim are external context absent from this article. The reference also simplifies partial automation of moderation into complete automation, and lists Vite where the new fund's initial project list instead names Astro, Agents SDK, EmDash, Hono, and Vinext. Vite belongs to the separately mentioned earlier fund. Do not reward a model for copying these simplifications. |
| 669 | [DynamoDB selective recovery](https://aws.amazon.com/blogs/database/recover-from-accidental-dynamodb-changes-using-bulk-executor/). Short procedural summary: PITR, export with old/new images, command, old-image restoration. | The reference omits the source's important warning that an unfiltered revert also removes valid concurrent changes. The outputs' transform warning is a useful addition, even though it is longer than the reference. |
| 1210 | [Chaos engineering in ECS payments](https://www.infoq.com/articles/chaos-engineering-ecs-payments/). Dense technical abstract with failure cases, numbers, staged rollout, and limitations. Distinctive 60/93-second DNS behavior and settlement incident agree. | The reference's broad phrase about 2.4× retry load loses the source's specific measurement: sustained database connection usage. Its employer attribution is not present in the selected article body; it was not independently verified in this review. The source's infrastructure claims are evaluated as the author's account, not independently proven product behavior. |
| 1152 | [Instrumentation talk transcript](https://www.infoq.com/presentations/instrumenting-scale/). Colloquial educational explanation of contention, sharding, and histograms. | The reference's claim that the entire service can slow down 200× changes the scope of the source's comparison of counter-increment costs. The speaker is at IOP Systems, formerly on Twitter's infrastructure performance team, not simply a current Twitter employee. The source's hypothetical 6.4-billion ceiling assumes only metric updates and zero business logic; it is not observed business-service throughput. |

The approved allowance for **verified** explanatory context cannot be tested by
expecting these source-only runs to reproduce unsupported additions in the
references. Conversely, factual errors in the references must not be learned as
style. These six posts span substantially different formats; one universal
two-paragraph shape cannot match all six.

## Per-output assessment

Scores are ordinal judgments, not calibrated measurements or a claim of
statistical significance. **F** is source faithfulness: 5 = no substantive error
found; 4 = localized loss of precision or overstatement; 3 = material ambiguity;
2 = material fact/metric conflation; 1 = broadly unreliable. **M** is selection of
the main content: 5 = particularly well selected; 4 = useful with omissions or
extra detail; 3 = noticeably unfocused; 2 = main point obscured; 1 = wrong focus.
**S** is resemblance to the case's reference: 5 = very close; 4 = close in density
and explanatory style; 3 = partial; 2 = mostly generic summary; 1 = poor fit.
No claim is made that a one-point difference will survive human evaluation.

`B` = baseline-paragraph; `R` = rules-v4; `E` = examples-v4.

| Case / arm | F | M | S | Evidence and limitation |
| --- | ---: | ---: | ---: | --- |
| 16 / B | 5 | 4 | 2 | Correctly separates the storage stack, ZippyDB metadata, caches, 80%, and 1–2 ms. Formal catalogue of mechanisms; the second paragraph spends space on future work and an unnecessary generic caveat. Little of the reference's accessible explanation. |
| 16 / R | 5 | 5 | 2 | Most complete causal account of the two problems; retains global HDD source of truth, caching, prefetch, and TTL/LRU. Correct numeric scope. Dense terminology and three long paragraphs replace the reference's questions and explanatory lists. |
| 16 / E | 5 | 4 | 3 | The explicit list of changes is closer to the reference's structure; claims and units agree. The global HDD source of truth is omitted, and the final adoption/production sentence is less useful than explaining the effect on researcher iteration. Considerable untranslated jargon remains. |
| 260 / B | 5 | 3 | 2 | Framework and 93%/7% survey split are accurately attributed. Overweights the survey, techniques, and caveats relative to the concise reference; the pairing appears only indirectly in a caveat. Correctly invents no personal reaction. |
| 260 / R | 5 | 3 | 2 | Accurate layers, survey, 15 editions, final 20% co-creation, and pairing. At 247 whitespace-delimited words versus 104 in the reference, it reads like a detailed abstract rather than a short takeaway. No fabricated user opinion. |
| 260 / E | 5 | 3 | 2 | Accurate and slightly leaner than R, but still 203 words versus 104; includes the author's partial success and operational checklist rather than prioritizing the simple organizational lesson. No invented first-person judgment. |
| 604 / B | 5 | 4 | 2 | Correctly distinguishes the earlier and new $1M funds, two-year periods, dates, eligible projects, and committee role. Uses “almost 100,000” accurately. Does not explain the technologies as the reference does. |
| 604 / R | 5 | 4 | 2 | Accurate roles, funds, eligibility timing, and moderation distinction. Three clear but generic news paragraphs. “Для поддержки порядка” is broad framing, subsequently clarified as connecting the community with experts, not moderation. |
| 604 / E | 4 | 4 | 2 | Compact single-paragraph news digest; numeric amounts, dates, and terms agree. The parenthetical says automation takes over moderation, whereas the source says automation reduces the burden and human insight remains. It also compresses eligibility into a blanket grant statement. The reference's explanatory Q&A is absent. |
| 669 / B | 5 | 4 | 3 | Accurate in-place revert, old/new-image export, 35-day PITR comparison, and the crucial transform warning. No incorrect quote or number. Formal explanation, not the reference's short executable workflow; enabling PITR as a prerequisite is not stated. |
| 669 / R | 5 | 4 | 3 | Adds correct insert/delete/update semantics and selective keep/undo/fix behavior. Correct cost scope. More implementation detail than this short reference needs; repeats the transform caveat. PITR prerequisite and direct command example are absent. |
| 669 / E | 5 | 4 | 4 | “Схема такая” and the simple explanation of Bulk Executor are closest to the reference's practical voice. Correctly says transform executes before revert and preserves the risk warning. Still about twice the reference's length and omits the explicit PITR prerequisite. The price/speed advantage is stated more categorically than B/R, but remains consistent with the article's described scenario. |
| 1210 / B | 5 | 4 | 4 | Correct 60 → 93 seconds, 400 TPS → approximately 37,000 requests, 500 ms, connection usage ×2.4, and settlement figures. Captures more of the reference's incidents than E. Repeats the Stage 4 timing in two forms and has a generic caveat paragraph. |
| 1210 / R | 5 | 4 | 4 | Preserves “connection usage” for ×2.4 and the DNS quantities; good staged approach and initial experiments. Omits the distinctive Spot/14,000-transaction incident. The described AZ remedy remains within the author's account, but “лечится” is too general if read as an independently verified guarantee. |
| 1210 / E | 4 | 4 | 4 | Closest compact density to the reference, with useful failure conditions and scope limits. Replaces “sustained database connection usage ×2.4” with generic “database load ×2.4”: the factor survives but the measured quantity does not. Omits the Spot incident and the 400 TPS/37,000 consequence. |
| 1152 / B | 2 | 3 | 2 | Material conflation: follows a counter increment of approximately 1.5 µs with Prometheus >3 µs and goodmetrics double-digit µs without switching the metric; those latter figures concern histograms. Also calls hypothetical metric-only ceilings requests/sec without the zero-business-logic assumption. Long catalogue of caveats replaces the reference's simple contention explanation. |
| 1152 / R | 4 | 4 | 3 | Correctly separates histogram costs and explains false sharing, unlike B. The 27M/119M/6.4B values are described as requests/sec “on measurements,” without the source's hypothetical metric-only ceiling assumption. The source should not be read as demonstrating that service throughput. More readable than B but still jargon-heavy. |
| 1152 / E | 5 | 4 | 3 | Best numeric scope in this case: uses increments/sec for 6.4B versus 27M, and keeps the 200×/1400× figures attached to operation costs. Correct current/former affiliation. Avoids the reference's false whole-service slowdown claim. Does not translate contention or histogram mechanics into the reference's everyday analogy; some space goes to author tools and registry details. |

All 18 outputs avoid invented user experiences, first-person endorsements, and
new personal opinions attributed to the channel owner. None fabricates a sourced
verbatim quotation. Quotation marks around translated concepts, idioms, or
technical labels are not presented as the author's exact words. The exact
headline is retained in every case.

## Candidate recommendation and negative evidence

**Use the unchanged `examples-v4` as the candidate for the subsequent independent
final test**, with `rules-v4` retained as the serious alternative. This is a modest
selection preference, not evidence of indistinguishability. E has the most useful
structural match on #16, the clearest practical voice on #669, and the best numeric
scope on the difficult #1152. R is more consistently careful about factual detail
on #604 and #1210, but its extra detail does not reliably bring it closer to the
author's style. The baseline offers no corresponding style advantage and has the
largest factual error.

Negative evidence is substantial:

1. No arm reaches “very close” in any of these six style judgments. The compact
   #1210 reference is the easiest fit; the colloquial #1152 and explanatory #16
   are not closely reproduced.
2. E still changes a measurement's scope on #1210 and overstates automation on
   #604. These are real defects despite a clean automatic flag list. Finding a
   literal number somewhere in the source is not a units/direction check.
3. All arms make short #260 and #669 much longer. The additional safety warning
   on #669 earns its space, but does not explain all of the expansion.
4. E uses a compact news paragraph for #604 instead of the reference's technology
   explanations. The source does not contain those explanations, so matching
   them requires verified context or conversation, not invention.
5. Proper names and English engineering vocabulary survive, but everyday
   explanations and the author's economical choice of what matters are uneven.
6. There is one generation per arm per source, six sources total, no independent
   human raters, no repeated-sampling stability estimate, and no blinded author
   verdict. No statistical superiority or “1:1 voice” claim is justified.

Any prompt adjustment informed by these findings is a **post-validation change**.
It must be frozen before the final test and must not be described as having
passed an independent test on these same six references. Preserve these outputs
and judgments even if later candidates improve on them.

## Source completeness and remaining limits

The six archived primary-source selections contain **117,335 characters**, with
raw HTML and extraction selectors retained. There was no application-level
truncation to a preview or model context budget. Sources #16, #260, #604, and #669
were fetched directly; #1210 and #1152 were fetched through the documented public
Jina HTML proxy. The latter is an explicit transport limitation, not hidden
publisher provenance.

- #16 contains the full article prose, figure captions, future work, and trailing
  share labels. The figure pixels, including the ingestion graph, are not in the
  text supplied to the model.
- #260 includes its article body plus author biography and subscription calls to
  action; #669 includes author biographies. These are retained residual material,
  not a claim of perfectly clean article-only extraction.
- #604 includes the complete program announcement, not external project
  documentation or linked grant/application pages.
- #1210 includes the article's code examples and #1152 the full public transcript,
  including Q&A. They do not replace source video, slide images, or a technical
  audit of the speakers' claims.
- HTML whitespace normalization preserves the selected prose but can remove code
  indentation. The exact HTML remains archived. These plaintext artifacts are
  suitable for this prose-summary review, not a lossless executable-code export.
- Sources were collected on the review date and may have changed since the
  corresponding Telegram post. The snapshot proves what was supplied to this
  experiment, not byte-for-byte publication-time identity.

No external explanation, company acquisition, speaker employer, benchmark, or
cloud-platform behavior was silently verified through a search snippet. Where
the selected source does not establish a claim, the review explicitly leaves it
unverified. Final author references remain unopened by this reviewer.
