# Public channel corpus inventory

Collection completed at **2026-09-26 20:28:55 UTC** for
[@reading_data_news](https://t.me/s/reading_data_news). This is a coverage and
provenance report, not a stylistic analysis or a list of confirmed article pairs.

## Observed coverage

| Measure | Result |
| --- | ---: |
| Distinct visible message IDs retained | 1,529 |
| Lowest / highest observed ID | 1 / 1,542 |
| Public preview pages archived | 77 |
| Returned HTML bytes, excluding headers | 6,862,244 |
| Earliest observed timestamp | 2026-07-05T23:29:43+00:00 |
| Latest observed timestamp | 2026-09-23T21:52:51+00:00 |
| Messages with a timestamp | 1,523 |
| Messages without a visible timestamp | 6 |
| Messages containing explicit external hyperlinks | 538 |
| Distinct external hyperlink URLs | 912 |

Pagination followed `?before=<lowest observed ID>` from the latest public page
back to **message ID 1**. All visible IDs, including forwards and entries without
text, were retained. This establishes coverage of the history returned by that
public-preview traversal; it is not an authenticated Telegram export and does not
prove that inaccessible messages have been recovered.

Thirteen IDs within the observed span were not returned: **101, 419, 775,
1194–1197, 1199, 1382, 1448, 1493, 1500–1501**. They are unobserved IDs, not
confirmed deleted posts. No publication was invented or silently interpolated for
these gaps. All observed timestamp years are 2026; absent timestamps remain null.

## Descriptive categories

The collector assigns only mechanically observable categories, with forwarding
taking precedence. These are deliberately not assertions of authorship or genre.

| Observable category | Messages |
| --- | ---: |
| Nonforwarded text with external hyperlinks | 530 |
| Nonforwarded text without external hyperlinks | 981 |
| Forwarded | 16 |
| Media or unavailable/absent text | 2 |

A nonforwarded linked message is only a **candidate** article summary. Definitions,
original commentary, link collections, author-written summaries, and other formats
must be distinguished during the research stage. Unlinked messages must not be
discarded: title-based matching is required for a substantial part of the corpus.
The hyperlink count includes links in forwards; therefore it differs from the
nonforwarded-linked category count.

## Capture provenance and formatting fidelity

The latest page was supplied as a previously captured direct Telegram response.
Its capture timestamp was derived from that snapshot's filesystem modification
time; its HTTP response headers were not retained. Subsequent direct requests
timed out both locally and from the existing project server.

The remaining 76 pages were explicitly fetched through the public
`https://r.jina.ai/https://t.me/s/reading_data_news?before=...` endpoint using
`X-Return-Format: html`. These files contain **proxy-returned HTML**, which can be
serialized/rendered HTML rather than the original Telegram HTTP response bytes.
Each page records the original requested URL, effective proxy URL, capture time,
HTTP status, local raw-file path, response headers, and byte count. No Telegram
account credentials or model API calls were used.

An independent proxy capture of the already-seen latest 20 messages was compared
against the direct snapshot, without inspecting other messages for prompt tuning:

- 20/20 message IDs, normalized text values, and timestamps agree.
- 20/20 ordered formatting-tag and hyperlink signatures agree, including bold,
  italic, code, blockquotes, line breaks, and link destinations where present.
- Formatted HTML is byte-identical for 8/20 messages. The remaining serialization
  differences are preserved rather than presented as original wire bytes.

This check supports the fidelity of the chosen transport on that sample; it is
not proof of byte identity for unseen Telegram responses. The comparison and its
proxy HTML are retained under `corpus/validation/`.

## Local artifacts and contracts

Artifacts live in the git-ignored directory
`.inoreader-state/style-research/corpus/`. Source posts are not committed to the
repository.

- `raw/*.html`, `raw/*.headers`, `raw/*.json`: immutable page captures and
  provenance. The supplied first page has no corresponding original headers.
- `posts.jsonl`: all messages ordered by ID; source markup, exact returned post
  HTML, derived plain text, timestamps, forwarding metadata, links, title
  candidates, descriptive categories, and all observed page origins.
- `index.jsonl`: matching/partition metadata without full post bodies, formatted
  HTML, raw post HTML, forwarding bodies, or link-preview descriptions. First-line
  and preview-title candidates are still text metadata, not verified source titles.
- `inventory.json`: counts, timestamps, observed gaps, pages, and stop reason.
- `validation/proxy-comparison.json`: transport comparison results.

Raw captured HTML is the authoritative retained representation. Plain text is a
documented derivative: HTML entities are decoded once, line breaks/block boundaries
become newlines, script/style bodies are excluded, and boundary whitespace is
trimmed. Formatting and original returned content remain recoverable from the
stored HTML. No style normalization, title rewriting, fact extraction, or semantic
content classification is performed by the collector.

Explicit hyperlinks carry `evidence: explicit_html_hyperlink`; first-line titles
carry `first_text_line_unverified_title`; preview titles carry
`link_preview_title_unverified_article_match`. An explicit link proves the link
exists in the message, **not** that the destination article is the summary's
confirmed source. Verification belongs to the source-matching stage.

Repeated observations of one ID retain all page provenance. Conflicting text HTML
or timestamps fail explicitly while preserving both raw pages and the first
normalized record. Intermediate JSONL files are replaced atomically so readers
do not observe half-written records.

## Reproduction and holdout hygiene

Collector source: `tools/style_research/collect_channel.py`. Reproduce the archived
normalization without network requests using:

```sh
python3 tools/style_research/collect_channel.py --resume --transport jina
```

To collect a fresh corpus, choose a **new output directory**; do not overwrite a
frozen research corpus:

```sh
python3 tools/style_research/collect_channel.py \
  --output .inoreader-state/style-research/corpus-new \
  --transport jina --timeout-seconds 90
```

Direct Telegram access remains the default transport; proxy use requires the
explicit `--transport jina` choice. Resume reuses existing raw pages and only
fetches missing pages. No concurrent or paid requests are involved.

Before corpus collection, the coordinating researcher had seen IDs **1523–1542**.
Those messages and article/duplicate groups containing them are excluded from the
final held-out pool. During collection, no other complete post texts were analyzed
to tune a prompt. Partition creation and freezing use the metadata index before
development-set reading. Collector tests use synthetic messages, not future
evaluation targets.

Verification performed:

```sh
python3 -m unittest discover -s tools/style_research -p 'test_*.py'
python3 -m py_compile tools/style_research/collect_channel.py tools/style_research/test_collect_channel.py
git diff --check -- tools/style_research docs/research
```

The collector has **nine passing regression tests** covering exact returned HTML,
entity decoding, forwarding, empty/media messages, nested markup, invalid identity
and dates, metadata-only indexing, pagination/resume, and conflicting snapshots.
Successful collection reached message ID 1 and retained 1,529 distinct messages.
