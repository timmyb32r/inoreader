# Final candidate 01: independent review of cases 1–10

Reviewed on 2026-09-27 Moscow time / 2026-09-26 UTC. This report evaluates the
frozen output as it stands. It does not revise the prompt or responses, and it is
not an author acceptance verdict.

## Boundary and method

The candidate was frozen at `2026-09-26T21:09:50.490803+00:00` in
`.inoreader-state/style-research/final-plan.json`. The ten reviewed generations
started between `21:10:11` and `21:10:17` UTC, after that freeze. Each settled with
`stop`. The reviewer read the complete extracted sources and generated outputs
first, then selected **only** references **198, 127, 587, 464, 882, 900, 47, 299,
597, 449** in Python before printing their author bodies. The remaining ten final
references were not opened by this reviewer.

Records and immutable evidence are in:

- `.inoreader-state/style-research/pairs/final.jsonl`, filtered to the ten IDs and
  `status == "confirmed_title_url"`;
- the `final-{id}-candidate-01` entries in
  `.inoreader-state/style-research/experiments/budget.json`;
- each mapped experiment's `response.json`, `rendered.md`, and `evaluation.json`.

The ten full source selections contain **214,597 characters**. Long transcripts
were read in contiguous chunks; terminal-display omissions were explicitly
re-read. Neither the input artifacts nor generated summaries were truncated for
this review. Findings depend on source evidence, not a model's brand or a prompt
label. The reviewer was not blinded to the fact that these are candidate outputs.

## Outcome

There is **one material unsupported assurance** (#464), **two cautions** (#900,
#299), and **seven cases without a substantive source-fidelity error found**.
Several otherwise faithful summaries miss the source's or author's main emphasis.
All ten are retained in this report and should remain in the feedback collection.

The material privacy assurance alone prevents treating this half of the candidate
as having passed a strict factual gate. Successful transport, preserved titles,
and numbers that literally appear in a source do not establish factual fidelity.

| ID | Source fidelity | Main finding | Source/reference alignment |
| --- | --- | --- | --- |
| 198 | Pass, with selection weakness | Spark 3.5.6, November 2025, 60-worker default, and 20-minute idle timeout agree. The summary spends its limited space on configuration and launch history, but omits the article's three actual integration patterns: Jupyter, VS Code, and dbt + Airflow. | Confirmed by exact headline/URL, serverless Spark, and Spark Connect. The reference is an extended explainer, not merely a launch abstract. |
| 127 | Pass | Current/Trailing split, 1–6-week buffer, EventBridge → Lambda → Fargate, approximately 35 catalog queries, driver tests, S3 and SNS all agree. A rollback is something the evidence supports requesting, not an automatic feature; the output correctly says “запросить откат.” | Confirmed. The reference adds personal criticism and a Redshift/Greenplum analogy not established by this source. Those opinions are not permissible invented user opinions. |
| 587 | Pass, compressed | Lance/object storage, graph semantics, Git branching/merging, and probabilistic writers agree. No made-up numerical result. The 68-word result is a broad description and omits the schema-typed query interface, division of graph semantics from traversal analytics, and present limitations. | Confirmed by the episode title, OmniGraph, Lance, and Git-style coordination. One important reference claim is contradicted by the full transcript; see below. |
| 464 | **Fail: unsupported privacy guarantee** | The output says the code never leaves the customer's infrastructure because the evaluator runs locally. The source establishes local evaluation and API generation of candidate programs; it does not establish that all seed code, problem context, or code content is never transmitted. Correct performance figures do not support that broader assurance. | Confirmed. The reference centers the evolutionary loop and OpenEvolve; the output instead selects customer performance testimonials and omits OpenEvolve. |
| 882 | Pass, with substantial selection weakness | The examples and 30-person/flat-organization qualification are source-supported. The output reduces a leadership talk to expanding non-coding skills and a catalogue of example projects, omitting most of its five-part argument. | Confirmed by the exact talk title and distinctive legal-policy/standards examples. The reference emphasizes mentoring, audience-aware communication, ego, supporting others, and patience. |
| 900 | Caution: rollout ambiguity | Correct property name, `{broker_id}`, KRaft restriction of the former method, and customer-owned network/trust prerequisites. “Применяется сразу ко всем брокерам” can sound simultaneous, whereas the source explicitly says rolling restart, with halt-on-failure and mixed old/new configuration during failure. | Confirmed by headline and the one-property change. The output also misses the reference's simple motivation: stable customer-controlled names when underlying clusters change. |
| 47 | Pass | Correct distinction between Meerkat and QuePaxa, optional leader, concurrent proposals, control-plane scope, experimental status, up to 50 replicas, and no error-rate increase in the PoC. No invented production deployment or zero-latency claim. | Confirmed by title/URL and the distinctive consensus explanation. The reference is much longer and explains linearizability and the common log; the output compresses these out. |
| 299 | Caution: vendor possibility becomes certain result | The architecture, named engines, 15-minute execution limit, and default 1,000 concurrent executions agree. “За секунды находит корневую причину” reads as a guaranteed result; the source describes what AI can do, gives representative examples, and expressly says output depends on model, prompt, and available artifacts. | Confirmed by exact title/URL, scheduled internal-state snapshots, S3, and AI root-cause analysis. The reference includes the user's own negative judgment and a cross-post link; these are not facts the model should invent. |
| 597 | Pass | The three infrastructure components, five-step loop, human validation/deployment, and four observability dimensions agree. It avoids claiming that all production remediation already runs without human approval. | The main post is confirmed. Its final line links a second Monte Carlo article; that separate article was not supplied and is not included in the single-source alignment claim. |
| 449 | Pass | More than 30 sources, query languages, all eight listed new enterprise sources, existing permissions, and the telemetry/model-quality caveat agree. “Unified observability” is correctly attributed to the vendor. | Confirmed by the exact title and the same source expansion, operational workflow, and quality caveat. This is the closest genre/coverage match in this half. |

“Pass” here means no substantive source-fidelity error was found by this reviewer;
it does not mean author-style acceptance, external verification of vendor claims,
or a proof that the source itself is correct.

## Material and caution findings in detail

### 464: local evaluation does not prove no code transmission

The [AlphaEvolve article](https://www.infoq.com/news/2026/07/alphaevolve-generally-available/)
describes a client-side evaluator, API-produced candidate programs, local scoring,
and submission of results. The output adds a categorical confidentiality guarantee
about code never leaving the customer's infrastructure. No statement in the
archived source establishes that full boundary. This is an unsupported inference
with operational significance, not merely different wording.

The source's measurements are otherwise transferred accurately: Klarna doubled
training throughput, approximately 6,000 candidates over three weeks; JetBrains
15–20% lower completion latency; warehouse picking routes 10.4% shorter; forecasting
accuracy up 22% and runtime down 90%. The output preserves that these are
vendor/customer testimonials without independent benchmarks. It does not reverse
improvement direction or confuse time with throughput in these figures.

The reference's OpenEvolve explanation is a notable missed content preference.
The original source includes OpenEvolve, so this omission does not require
external browsing to repair in a future candidate. This report does not repair
the frozen output.

### 900: cluster-wide scope versus simultaneous application

The [MSK source](https://aws.amazon.com/blogs/big-data/amazon-msk-simplifies-configuring-custom-domain-names/)
distinguishes a single cluster-wide configuration from its rolling application.
The candidate's wording can be read in either sense. It is therefore a caution,
not a proven claim that the model intentionally asserted an atomic rollout.
The source also says the network layer does not automatically scale with newly
added brokers. The output does not incorrectly claim otherwise, but its compact
description gives no rollout or scale-out qualifications.

### 299: the strength of the root-cause promise

The [RDS article](https://aws.amazon.com/ru/blogs/database/ai-powered-incident-analysis-for-amazon-rds-using-automated-forensic-artifacts/)
is itself promotional and repeatedly describes rapid analysis. However, it also
labels its examples representative and says results vary with the model, prompt,
and available artifacts. The output changes capability into an unqualified
successful outcome. This is weaker evidence than the privacy claim in #464, so
it is recorded as a caution, with the expected boundary explicit.

The introductory claim that CloudWatch “does not preserve causes” is similarly
too absolute: the article says metrics show symptoms, evidence is often lost,
and this solution complements existing database-monitoring tools. It does not
establish that CloudWatch is categorically incapable of retaining useful causal
evidence.

## Reference limits: style is not factual gold

The post/source matching task and the generation-fidelity task must remain
separate. Several selected author posts include extra context, personal reactions,
or factual overstatements:

- **587:** the reference promises automatic Parquet/Iceberg compatibility and
  graph querying over existing lakehouse data. At approximately 00:28:46 the
  [full podcast transcript](https://www.dataengineeringpodcast.com/episodepage/why-multi-agent-systems-need-shared-state-graph-semantics-and-governance)
  explicitly says existing Iceberg querying is not currently supported and is
  being requested/on the roadmap. Migrating Iceberg data to Lance is a different
  capability. The candidate does **not** copy this reference error.
- **198:** the reference's assertion that one pays only for work, not idle time,
  conflicts with the source's explicit billing for active sessions even while
  idle before timeout. Its 4-hour/5-minute Trino/Spark comparison is not established
  by this article. The candidate does not copy either assertion. The reference
  also treats Spark in Athena as wholly new, whereas the article describes a
  November 2025 update and notes previous iterations.
- **127:** the source tests existing Dev/QA clusters after patch events; it does
  not promise to provision a fresh Redshift installation for every patch. The
  reference's sarcastic interpretation and product comparison are author opinion,
  not factual requirements for the summary.
- **597:** an early reference sentence describes improvement without humans,
  while later bullets correctly retain human approval. The candidate preserves
  the source's human gate. The source itself describes its “first four” autonomous
  stages using a second set of labels, different from the numbered loop; its
  high-level distinction should not be turned into a precise new stage contract.
- **882:** the reference adds a Technical Lead/Staff/Principal hierarchy and
  a suggested one-to-two-year period, neither established as a universal ladder
  or duration by this talk. The transcript does say “CTO of GFXReconstruct” in
  its opening, so the candidate's unusual affiliation is source-faithful. This
  review does not independently certify that title/company naming outside the
  transcript.

The rule forbidding invented user opinions means that neutral treatment of the
AWS articles is not itself a model failure. Matching the user's sarcasm by
manufacturing a new opinion would violate the agreed product boundary. Brevity,
word choice, concrete explanations, and selection of the useful mechanism can
still be evaluated without inventing those opinions.

## Style and selection observations for human feedback

These are observations, not a new tuning round:

- #198 contains accurate implementation facts but omits the integration pattern
  that answers “how do I use this?”; its language is less conversational than
  the reference and lacks its explanatory comparisons.
- #882 retains one of five development directions and drops mentoring,
  audience-aware communication, ego, and team growth. This is an over-compression
  problem even without requiring the reference's added career-ladder explanation.
- #464 emphasizes testimonials rather than the repeatable optimization mechanism
  and the open implementation that the author chose to explain.
- #587, #47, and #597 are much shorter than their explanatory references. They
  remain coherent but do not establish that one short-paragraph template can
  reproduce the channel's variable depth and structure.
- #127 and #299 are longer, more procedural, and more neutral than the author's
  opinion-led posts. The neutral stance is intentional under the factual policy;
  the appropriate remaining balance of concise description and explanation
  requires the author's judgment.
- #449 has a similar news-summary genre and preserves the reference's caveat,
  although it foregrounds concrete source names and query languages more than
  the reference.

No invented personal experience, first-person endorsement, or fabricated sourced
quotation was found in these ten outputs. This does not cancel the unsupported
privacy assurance or content-selection weaknesses.

## Completeness and uncertainty

The stored sources are the complete selected primary article bodies/transcripts,
not search snippets. InfoQ #464, #882, and #449 were acquired through the recorded
public Jina HTML proxy; the other seven used direct acquisition. The podcast
selection includes its full approximately one-hour transcript, not just episode
show notes. Transcripts contain speech-recognition/wording errors and repetitions;
video/audio was not separately audited.

Source figures, diagrams, embedded screenshots, and external pages are not part
of the plaintext inputs. HTML remains archived, but whitespace normalization can
remove code indentation. Some AWS bodies retain author biographies. The sources
may have changed since the Telegram posts; the snapshots establish what this
experiment actually supplied, not publication-time byte identity. No universal
claims about product capabilities beyond these sources were independently
verified here.

The blind-review artifact should include every generated case, including #464.
Its role is gathering author feedback on an imperfect frozen candidate; it must
not be described as evidence that the candidate passed factual or style acceptance.
