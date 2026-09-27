# Candidate 01: source review B

Reviewed 2026-09-27 after the freeze in
`.inoreader-state/style-research/final-plan.json`. This is an assistant review,
not the author's verdict. The complete saved sources, generated answers and
corresponding author references were read for seven cases: 1384,1107,715,948,
826,73,305. The other three cases assigned to this half (82,318,592) receive an
independent review in `final-review-special.md`. No answer or prompt was edited.

## Findings

| Case | Source faithfulness | Selection and style observations |
|---|---|---|
|1384|No substantive invented fact found. The three-track process, six cases, compaction misuse, unauthorized uploads and mixed community reaction are all in the source. These are reported incidents, not instructions followed by this research.|The answer summarizes almost every source paragraph. Its length is unusually close to this long author reference, but replaces the author's visible six-case list with dense paragraphs.|
|1107|**Precision failure:** the source reports a “95th percentile service level objective of less than five seconds”; the output describes setup taking less than five seconds at p95 without retaining that this is a target/SLO. This presents a goal as a measured/achieved performance characteristic. 130,000 tasks and the isolation/gateway design otherwise agree.|A readable compact account of the problem and mechanism. It chooses infrastructure details where the author emphasizes operating scale and the risks of laptop execution.|
|715|249 bugs,207new,48systems,1.4–9.8hours,$57median and21.1/78.9% agree with the blog. The source says slices of48systems; the summary does not make this scope explicit. The author's critique is attributed, rather than invented as the channel owner's opinion.|The reference is organized around circularity, heuristic justification and missing composition. The output spends most space on introductory capabilities and only one critique; it misses the distinctive selection and structure.|
|948|The 99.4%/351 and78.5→91.0 metrics retain their offline-only scope and do not claim business impact. Routing, memory and metric reconciliation agree. No fabricated quote or personal experience found.|The answer is much shorter than the author's explanatory version; it retains the central unsafe-promotion example but loses much of the accessible explanation and emphasis.|
|826|No substantive factual error found in the rendered answer. It distinguishes OS rebasing from application dependencies, next-build versus rebase, typical24hours versus contractual7/14days, and the survey/study denominators.|**Severe compression failure:** nine paragraphs closely paraphrase almost the entire source, despite a short author reference and the requested short form. Do not silently trim this output or select another generation.|
|73|45minutes→about1second, hundreds→few ms, up to40× and90%cost reduction are supported and attributed. AI limitations and human optimization are retained; no new personal opinion found.|Omits the concrete FoundationDB→PostgreSQL change and relational-vs-KV explanation that dominate the author reference. The author's strongly personal wording is correctly not invented, but the neutral explanation could still select the central architectural change.|
|305|21,000operations/600hours/20days/16weeks and customer/account figures are explicitly attributed to Snowflake/its quoted partner and match the article. The arithmetic framing is the source's claim, not an independently measured result.|The author focuses on fixed project pricing, reusable domain expertise and partner economics. The output selects an implementation caveat and a catalogue of business counts instead. This is a content-selection miss, even though the selected claims are supported.|

All seven source/reference pairs are semantically aligned: exact URLs or titles
and distinctive mechanisms/results match. They are not factual gold labels. For
example, the305reference's5–10× generalization is not stated as such in the saved
source; the model must not reproduce it just to increase overlap. Personal
reactions in73and305 must not be fabricated under the approved policy.

## Consequences

The zero-detected-factual-defect criterion is already **not met** by candidate01
because of1107. This must remain visible even if the author likes its style.
Style acceptance is separately pending; this reviewer cannot substitute for it.
The twenty unedited outputs may still be used to collect the promised author
feedback. Any revision informed by these results requires fresh held-out cases;
these cases must never be relabelled as a new closed test.

Review evidence lives under the ignored research state: `pairs/final.jsonl`,
`sources/`, `corpus/posts.jsonl` (only the seven permitted bodies read), and the
`final-{id}-candidate-01` experiment entries. The exact source snapshots and full
responses are preserved; no copyrighted source is republished in this report.
