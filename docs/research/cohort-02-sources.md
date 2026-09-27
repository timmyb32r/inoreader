# Second closed final source cohort

The second source cohort was selected before candidate02 generation. Its 20 author-reference bodies remained unopened throughout source matching. This is source identity/boundary validation, not a claim of semantic pairing or style quality.

## Frozen selection and coverage

- Selection: `.inoreader-state/style-research/final-02-selection.json`, frozen 2026-09-26 21:27:27 UTC before new source fetches. Original `partitions.json` and corpus metadata remained unchanged.
- The original selection order within each month/length stratum is preserved. Round-robin resumes after the first 20 successful slots: August long/short, September long/short, July long/short. Every unavailable candidate is replaced by the next candidate in the same stratum.
- All 25 source groups attempted during cohort01 were excluded, including prefetched unused groups 391, 466 and 618. Explicit source-URL duplication was checked before selection and again against development, validation and cohort01 after matching; no overlap among the 20 selected sources was found.
- 20 sources: 4 August long, 4 August short, 3 September long, 3 September short, 3 July long, 3 July short. Total extracted source text: 368,783 characters.
- 17 unavailable/ambiguous candidates are retained in the same pair manifest. No failed case was silently dropped. Additional metadata-only prefetches 1417 and 1248 were not consumed; their source artifacts remain recorded and should be excluded from future fresh cohorts.

## Source records

All paths below are relative to `.inoreader-state/style-research/`. The authoritative immutable pair manifest is `pairs/final-02.jsonl`; `decisions-02.json` records the selection decisions, extraction boundary audits, PDF metadata-length check and unused prefetches.

| Slot | Post ID | Primary source | Text characters | Evidence |
| --- | --- | --- | ---: | --- |
| 1 | 539 | [Scaling StreamHub: Transitioning from Kinesis to Kafka for 145 Billion Daily Events](https://www.atlassian.com/blog/how-we-build/scaling-streamhub-transitioning-from-kinesis-to-kafka-for-145-billion-daily-events) | 19,240 | Exact source title |
| 2 | 683 | [Faster scaling for Aurora serverless to support agentic AI and other spiky workloads](https://aws.amazon.com/blogs/database/faster-scaling-for-aurora-serverless-to-support-agentic-ai-and-other-spiky-workloads/) | 6,204 | Exact source title |
| 3 | 1119 | [Measuring and improving search quality with Amazon OpenSearch Service](https://aws.amazon.com/blogs/big-data/measuring-and-improving-search-quality-with-amazon-opensearch-service/) | 21,985 | Exact source title |
| 4 | 1159 | [Shopify Introduces Gisting: Compressing LLM System Prompts into Learned Tokens](https://www.infoq.com/news/2026/09/spotify-gisting-llm-performance/) | 3,219 | Exact source title |
| 5 | 201 | [Run log analytics for a fraction of the cost with the new engine for Amazon OpenSearch Service](https://aws.amazon.com/ru/blogs/big-data/run-log-analytics-for-a-fraction-of-the-cost-with-the-new-engine-for-amazon-opensearch-service/) | 9,941 | Exact source title |
| 6 | 17 | [Introducing Durable Functions in PostgreSQL](https://techcommunity.microsoft.com/blog/adforpostgresql/introducing-durable-functions-in-postgresql/4526821) | 13,434 | Exact source title |
| 7 | 748 | [Embabel Agent Framework Reaches 1.0](https://www.infoq.com/news/2026/08/embabel-1/) | 5,553 | Exact source title |
| 8 | 713 | [Migrating Uber Eats Feeds to Webview](https://www.infoq.com/presentations/migration-mobile-application/) | 47,217 | Exact source title |
| 9 | 1012 | [Scaling AI is easy. Trusting it is hard.](https://www.getdbt.com/blog/scaling-ai-is-easy-trusting-it-is-hard) | 4,701 | Exact source title |
| 10 | 1048 | [Building table-aware, partition-aware physical replication for Iceberg tables](https://www.prequel.co/blog/physical-replication-for-iceberg-tables/) | 10,484 | Exact source title |
| 11 | 301 | [Pushing software engineering limits with “napkin math”](https://newsletter.pragmaticengineer.com/p/pushing-software-engineering-limits) | 30,428 | Exact source title |
| 12 | 94 | [Socrates: The New SQL Server in the Cloud](https://www.microsoft.com/en-us/research/wp-content/uploads/2019/05/socrates.pdf) | 69,352 | Exact explicit PDF URL |
| 13 | 687 | [Trace cascading decision failures with a blame graph on Amazon OpenSearch Service](https://aws.amazon.com/blogs/big-data/trace-cascading-decision-failures-with-a-blame-graph-on-amazon-opensearch-service/) | 21,140 | Exact source title |
| 14 | 516 | [Efficient log management with Amazon OpenSearch Service data streams](https://aws.amazon.com/blogs/big-data/efficient-log-management-with-amazon-opensearch-service-data-streams/) | 11,756 | Exact source title |
| 15 | 1214 | [Platform Engineering in the Age of AI](https://www.infoq.com/presentations/ai-platform-engineering-roundtable/) | 40,965 | Exact source title |
| 16 | 1483 | [Postgres on NVMe: performance and the convergence of transactions and analytics](https://clickhouse.com/blog/postgres-on-nvme) | 11,265 | Exact source title |
| 17 | 187 | [How Razorpay Built Real-Time Anomaly Detection with Amazon MSK](https://aws.amazon.com/ru/blogs/big-data/how-razorpay-built-real-time-anomaly-detection-with-amazon-msk/) | 16,617 | Exact source title |
| 18 | 285 | [The only scalable delete in Postgres is DROP TABLE](https://planetscale.com/blog/the-only-scalable-delete) | 7,122 | Exact source title |
| 19 | 609 | [Why we got rid of our small-PR rule](https://rootly.com/blog/why-we-got-rid-of-our-small-pr-rule) | 7,447 | Exact source title |
| 20 | 679 | [The Economics of Agent Optimization: From pilots to measurable returns](https://azure.microsoft.com/en-us/blog/the-economics-of-agent-optimization-from-pilots-to-measurable-returns/) | 10,713 | Exact source title |

## Unavailable and ambiguous candidates

| Post ID | Decision |
| --- | --- |
| 621 | Direct HTTP403 and proxy HTML is a Just a moment challenge, not article text. |
| 1470 | No external hyperlink in metadata. Three targeted exact/partial title searches found only general openGauss event pages, not a matching primary article. No guessed pair accepted. |
| 1316 | No external hyperlink or exact primary title. Searches found related Streamhouse announcement/definition from 15 Sep but title differs; matching could require reading reference body, prohibited before freeze. |
| 126 | Direct HTTP403 and proxy challenge page; full original article unavailable. |
| 309 | Primary YouTube page is a video with title/description; fetched HTML did not provide a full readable transcript. Chapter outline is not the complete source. |
| 339 | Direct HTTP403 and proxy challenge page; full original article unavailable. |
| 99 | Direct HTTP403 and proxy challenge page; full original article unavailable. |
| 217 | Direct HTTP403 and proxy challenge page; full original article unavailable. |
| 336 | Explicit Apache Iceberg source URL, but metadata title candidate describes a different named author/talk in Russian; no exact source title match. URL-only fallback must not hide conflicting title evidence. Semantic ambiguity held unconfirmed without opening author body. |
| 615 | Direct HTTP403 and proxy challenge page; no full original article. |
| 765 | Both direct HTTP403 and proxy challenge page; full original article unavailable. |
| 1243 | Direct HTTP403 and proxy challenge page; search rendering is not a complete audited HTML snapshot. |
| 390 | Direct HTTP403 and proxy challenge page; no full original article. |
| 771 | Primary URL discovered through publisher employee public post; proxy returned a Just a moment challenge; full source unavailable. |
| 567 | No exact title primary match: closest discovered Medium article says 13 challenges rather than 12; public proxy also returned challenge HTML, not full article. No edited number/title accepted. |
| 761 | Metadata has no source hyperlink. Two targeted exact-title searches found unrelated merge algorithms rather than this original article. No source guessed without opening author reference. |
| 547 | Direct SSL timeout; public proxy returned Just a moment challenge page, not full original article. |

## Extraction and reproducibility

HTML was fetched with the existing `tools/style_research/fetch_source.py` helper, using direct HTTPS or the explicitly recorded public `r.jina.ai` transport. Every attempt retains raw bytes, HTTP headers, fetch time and status. Successful sources retain full selected HTML, full derived text, selectors, exact chosen headline and other observed title metadata. Generic `main` selections for dbt and Microsoft Community were narrowed through immutable derivative snapshots after DOM inspection. Rootly’s unique `main` is the article rich-text container itself. AWS body containers retain author biographies; InfoQ news retains its author footer; presentation records retain full transcripts. Article-local CTAs may remain. None of these artifacts is a search snippet.

The Socrates record (#94) has no author title in channel metadata, so its evidence is deliberately `confirmed_explicit_url`, distinct from `confirmed_title_url`. The exact explicit Microsoft PDF URL is preserved. Channel metadata has 855 text characters versus an 80-character link, a non-forwarded linked-text classification, and no body was opened. Its exact source heading is `Socrates: The New SQL Server in the Cloud`, verified against both the first printed heading and PDF `/Title`. The original PDF contains 14 pages and is retained as `sources/final02-94-pdf.pdf`; the original fetch bytes are also preserved. All 14 nonempty pages were extracted in order with pypdf 6.10.0 and joined using an explicit form-feed delimiter. Page text lengths are in its JSON metadata.

For PDF reproduction, read the retained binary with `pypdf.PdfReader`, call `page.extract_text()` on **every** page, reject any empty page, and join the untruncated strings with `"\n\f\n"`. Append one file-ending newline. Heading metadata verification joins only layout line breaks in the first printed heading; it does not rewrite the chosen `/Title`. HTML text extraction follows the existing documented whitespace policy.

Limitations remain explicit: PDF figures and complex table geometry are not reproduced by plain text extraction; raw PDFs preserve them. HTML figure pixels are not part of article text. Code/HTML layout whitespace is normalized by the existing helper; raw HTML remains available. Source titles, whole body boundaries, final paragraphs and preserved text lengths were audited, but factual author/source alignment must be checked only after candidate02 freeze. No reference text was used to tune a prompt or to resolve ambiguous matches.

## Verification

`python3 -m unittest discover -s tools/style_research -p 'test_match_sources.py'` passed 4 tests. The new explicit-URL evidence mode is deliberate, exact, rejects missing/different URLs and rejects any conflicting non-URL title metadata; it never silently downgrades title matching. The completed cohort was assembled through the same frozen-slot implementation and checked for 20 unique original source URLs, no previous-cohort overlap, and exact persisted text lengths. No paid model calls, credentials or production changes were made by this source-collection step.
