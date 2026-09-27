# Frozen evaluation source matching

The source-only audit completed on 2026-09-26. It produced **20 final candidates**
and **six validation candidates** with exact title evidence and retained complete
published source text. This does **not** claim that the author-summary pairs have
passed factual alignment: that separate check must follow prompt freeze.

No held-out author-summary bodies were read for matching or extraction. The
researcher used only the frozen metadata index, source titles/links, public search,
and the original publishers' article/transcript pages. No paid API calls occurred.

## Selection and retained failures

Selection follows `partitions.json`'s frozen round-robin stratum slots. When a
candidate is unavailable, the next candidate **in the same stratum** fills that
slot; a difficult source is not silently replaced with an easier topic elsewhere.

| Stratum | Final sources | Validation sources |
| --- | ---: | ---: |
| 2026-07 / long | 4 | 1 |
| 2026-07 / short | 4 | 1 |
| 2026-08 / long | 3 | 1 |
| 2026-08 / short | 3 | 1 |
| 2026-09 / long | 3 | 1 |
| 2026-09 / short | 3 | 1 |

Final candidate IDs, in selected slot order:
`198, 127, 587, 464, 882, 900, 47, 299, 597, 449, 1384, 1107, 318, 82, 592, 715, 948, 826, 73, 305`.

Validation candidate IDs:
`16, 260, 604, 669, 1210, 1152`.

Unavailable candidate IDs **27, 13** (final) and **163** (validation) remain in the
pair manifests with reasons and attempt paths. Their original publishers returned
HTTP 403 directly, and the explicit public HTML proxy returned an access-challenge
page rather than article content. The challenges were preserved and rejected as
source text. No authenticated session, access-control bypass, or paywall bypass
was attempted. Other prefetched but unselected sources remain under `sources/`;
they were not quietly substituted into the frozen cohort.

## Source and extraction coverage

The final cohort includes nine publishing domains: AWS (4), InfoQ (9), and one
each from Data Engineering Podcast, Cloudflare, Monte Carlo, arXiv, Murat Demirbas's
blog, Grab Engineering, and Snowflake. Its complete extracted source texts total
**426,341 characters**, ranging from **3,716 to 79,839** per source. Validation
texts total **117,335 characters** across five publishing domains. Character
counts describe stored complete text, not an input truncation budget.

The cohort includes articles, complete presentation transcripts, a research paper,
and a full public podcast transcript. The long podcast candidate was retained:
the publisher's episode-page `loadTranscript()` function identified a public
transcript endpoint. Its full 69,610-character transcript was fetched from the
same publisher; both endpoint bytes and linking page are retained. Episode title
evidence is the publisher's displayed title including its episode suffix. Show
notes were not substituted for the transcript.

Extraction targets were checked using source-only beginning/end inspection and
DOM boundaries. Examples include `blog-post-content` for AWS, `article__data`
for InfoQ, `article-content` for Cloudflare, `blog-single-body` for Monte Carlo,
and the complete `article` for arXiv. Snowflake required an explicit unique content
container covering its three body sections. Original broader captures remain
available alongside narrower derivatives; no raw HTML was overwritten.

Global navigation and unrelated recommendations are excluded by the selected
container. Author bios, references, source-authored concluding calls to action,
and small in-article sharing labels remain when part of that selected body.
Podcast introduction/outro and paper references remain too. The text extraction
policy documents whitespace and paragraph-boundary handling; it never truncates
to a character or token length.

Title audit uses the **actual article heading or semantic headline**, with source
path and selector retained. Open Graph titles are retained as independent match
evidence, not used to strip or invent branding. Two initial OG choices, IDs 47
and 597, were corrected to their exact rendered headings before the final factual
gate; prior manifests and extraction derivatives are preserved. ID 715 uses its
semantic `entry-title`, avoiding an unrelated global `h1` reading “Metadata”.
No site/menu heading remains in the selected cohort.

## Pair evidence and reproducibility

Ignored research artifacts:

- `.inoreader-state/style-research/pairs/final.jsonl`
- `.inoreader-state/style-research/pairs/validation.jsonl`
- `.inoreader-state/style-research/source-decisions.json`
- `.inoreader-state/style-research/sources/`
- `.inoreader-state/style-research/pairs/development-overlap-audit.json`

Consumers must select records with `status == "confirmed_title_url"`; unavailable
records are deliberately present in the same files. Each selected record stores
post/group IDs, original source URL, exact chosen title and heading provenance,
raw/text paths, capture time, extraction selector, transport, character count,
matching evidence, and `content_alignment_verified: false`. The matching check is
exact Unicode title equality after boundary whitespace trimming, against retained
source headings or metadata. No fuzzy semantic similarity is called proof.

Direct requests and explicitly selected public-proxy requests are distinguished.
Proxy HTML is its returned representation, not asserted to be the original wire
response. All initial failures and overly broad captures are retained. The
source helper never consults Telegram content or reads an API key.

An explicit-link overlap audit compared all 26 selected URLs to development-set
metadata; no cross-development URL collisions were found. The comparison ignores
`www`, final slashes and tracking query parameters, and recognizes AWS's `/ru`
and arXiv's `/abs` versus `/html` variants as alternate renderings. Original URLs
are not rewritten. This audit cannot disprove unseen aliases or semantic duplicate
bodies, which remain part of the later factual/group audit.

Tools:

- `tools/style_research/fetch_source.py`: fetch immutable raw snapshots; extract
  complete selected text; create new local derivatives with `derive_article()`.
  Requires `lxml` from the bundled research Python.
- `tools/style_research/match_sources.py`: assemble a cohort from reviewed source
  decisions without loading author-summary bodies; enforce exact title evidence,
  same-stratum replacement, explicit missing decisions, and stored text length.

For a new manifest location or reproduction directory, assembly is:

```sh
python3 tools/style_research/match_sources.py \
  --decisions .inoreader-state/style-research/source-decisions.json \
  --cohort final --count 20
```

The tool refuses to overwrite an existing pair manifest. Keep the audited current
manifests for evaluation; `*.before-heading-audit*.jsonl` are superseded evidence,
not current inputs. Source snapshot keys likewise refuse conflicting reuse.

Verification: **18 tests pass** across collection, source extraction and cohort
assembly using:

```sh
/Users/timmyb32r/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 \
  -m unittest discover -s tools/style_research -p 'test_*.py'
```

Tests cover preservation, pagination, conflicting versions, title selection,
unambiguous article boundaries, large inputs without truncation, immutable raw
snapshots, local derivatives, frozen stratum selection, explicit inaccessible
candidates, exact-title rejection, and changed source-length detection.
