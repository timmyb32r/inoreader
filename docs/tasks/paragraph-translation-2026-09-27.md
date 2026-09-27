# Paragraph translation — selected design 2

User selected the dictionary-card design: a `文` mode next to Summarize, paragraph
selection, Russian paragraph translation, underlined original words, and a hover
card containing original text, Mandarin pinyin and contextual Russian meaning.

Implemented in isolated `reader-ai::translation`, PostgreSQL translation-job
storage, two authenticated API routes and `web/src/translation`. Existing
summary/chat data and prompts remain untouched. No provider calls on hover.
See [contract and operations](../paragraph-translation.md).

Validation on 2026-09-27:

- `just check-affected`: passed (Rust compile and TypeScript).
- Release formatting, Clippy, complete Rust tests and Chromium acceptance: passed.
- Frontend: 130 tests passed; browser E2E: 32 passed. Four focused translation
  tests subsequently passed, including the added uncertain-POST retry-ID case.
- Pinned YDB backup/restore, crate boundaries, inventory and operational assets:
  passed.
- The first release attempt caught a test mutex scope lint; fixed. A later
  frontend assertion still expected three toolbar actions; updated for the new
  fourth action and its missing-key disabled state. Remaining release stages
  were resumed after that test-only correction, all passed.
- Real PostgreSQL tests cover ownership, exact-source validation, concurrent
  duplicate starts, cached reuse, persisted usage, explicit retry after expired
  leases, stale lease fencing and rejection of corrupt stored identity.
- Browser tests verify immediate pending feedback, no duplicate paid activation,
  preserved source text/links/emphasis and stable toolbar/subsequent-paragraph
  coordinates through completion and word hover.

Deployment on 2026-09-27:

- Server `158.160.186.87`, image `inoreader-app:paragraph-translation-20260927`
  (`sha256:d52c4f484d5af5a3bf41afe08c2025a9e602c8fb7ed5f132405ec1ec97b02159`).
  Only the app container was recreated; readiness returned 204.
- Full PostgreSQL archive (about 835 MiB) saved at
  `/home/timmyb32r/inoreader-backups/paragraph-translation-20260927/database.dump`.
  `pg_restore --list` and full decoding with `pg_restore --file=/dev/null` passed.
  Previous app image retained as `inoreader-app:before-paragraph-translation-20260927`.
- Live authenticated smoke translated a 90-character Chinese paragraph using
  `deepseek-flash`: 49 words, exact source preserved, pinyin present, 5.64 seconds
  including polling and cached reopen; estimated provider cost $0.001623900.
- A second operation ID for the same paragraph returned the existing completed
  job. Anonymous access and foreign-origin POST were rejected. The temporary
  smoke-test session was deleted and its deletion verified.
- Credential-free local smoke evidence is in ignored
  `.inoreader-state/deployments/paragraph-translation-20260927/smoke-report.json`.

## Follow-up: title, introduction and headings

The initial selector only covered `p`/leaf `li` inside the full-text container.
Consequently the displayed title and RSS introduction above that container, and
headings inside it, were not selectable. The annotation boundary now includes the
article title/introduction while keeping the full-text typography container intact.
Frontend and backend both accept h1–h6; title/introduction requests require exact
membership in the owned article metadata. Summary snapshots and stored data are
unchanged.

Validation: `just check-affected` and the complete `just check-release` passed;
132 frontend tests and 35 browser tests passed. New cases cover title/introduction/
heading clicks and dictionary cards, unchanged toolbar/following-paragraph geometry,
exact metadata membership, rejected partial/injected text and cross-account access.

Deployed follow-up image `inoreader-app:translation-intro-20260927`
(`sha256:c56a635df1170c9ac02f00a8211ff850dff709fa99f0bb2e39206f8e92b7b144`)
on `158.160.186.87`; readiness returned 204. No database migration or data rewrite.
Live title/introduction requests passed article membership and were queued, but
three provider attempts returned invalid/incomplete segmentation and failed closed.
Thus live selection/API acceptance is verified; successful real-provider translation
for those particular metadata texts is not verified. Browser tests verify successful
rendering with valid provider results. All temporary smoke sessions were removed.
Provider-output reliability remains a separate limitation; no source normalization,
weakened validation, or automatic paid retry was introduced to conceal it.

## Follow-up: provider protocol reliability

Reproduced the screenshot failure against DeepSeek Flash using the exact
150-character introduction. The old response omitted a source space after
`“升维”`, so the lossless concatenation check rejected the entire response.
This was a fragile application/provider contract, not an unavailable API.

Replaced model-generated literal segments with an exhaustive ordered dictionary.
The server matches exact source entries and copies original punctuation/whitespace
between them. It neither normalizes source nor guesses missing words. The prompt
explicitly includes function particles, repeated words and truncated final words.
Optional exact punctuation annotations are retained rather than rejected.
Persisted/UI segments still reconstruct the original byte-for-byte. Detailed,
credential-free Russian errors distinguish malformed format, missing/mismatched
words and missing translation/pinyin. There is no automatic paid retry.

Captured a real new-format response as a regression fixture. Also checked the
previously failing mixed Chinese/English RBAC/GreptimeDB title against Flash.
Unit tests exercise NBSP/tabs/newlines, repeated terms, C++, omitted/reordered/
rewritten words, incomplete pinyin, and persisted literal corruption.

Final validation: `just check-affected` and the complete `just check-release`
passed (25 reader-ai tests, real PostgreSQL/YDB/Chromium acceptance, 132 frontend
and 35 browser tests, backup/restore and architecture checks). The real-response
fixture initially exposed an overly strict punctuation-entry check, which was
corrected before deployment; the full release gate was then rerun successfully.

Deployed `inoreader-app:translation-dictionary-20260927`, image
`sha256:071937e0a2bb14e1d2f10e0f6edf70336dc59c49d3c70c63659e87e0ddfaecb5`.
Readiness returned 204. Production smoke on the exact previously failing
150-character introduction completed successfully: 83 annotated entries, exact
source preserved, Chinese pinyin present, cached reopening reused the same job.
Elapsed time including polling/cache check: 6.69 seconds; estimated request cost
$0.002100036. The temporary session was deleted and cleanup verified.
Evidence: ignored `.inoreader-state/deployments/translation-dictionary-20260927/smoke-report.json`.

## Follow-up: clicking an annotated word

Removed the word-click early return that only opened the word tooltip and bypassed
paragraph selection. Word clicks now use the same cached paragraph action as the
surrounding paragraph. Hover/focus still supplies the dictionary card; Enter/Space
activate the same click path. Regression tests close the translation panel, click
an annotated character, and verify immediate reopening with no additional POST or
movement of the toolbar/following paragraph.

Validation: affected check and complete release gate passed; 132 frontend tests
and 35 browser scenarios passed. Deployed `inoreader-app:translation-word-click-20260927`
(`sha256:e9a777128dfabbfc9a65c3acb85cb60be87083624bb174d564dcab9d5482fe42`)
to `158.160.186.87`; readiness returned 204. No provider call or data migration
was needed for this UI fix.

## Follow-up: unwrapped article title text

Inspected the exact article HTML through the owned article API. The secondary
cnblogs title is a nonblank text node at the fragment root, not a paragraph or
heading. Translation now wraps uncovered article text nodes in temporary inline
spans without changing their characters, whitespace, font or layout. Backend
membership accepts the same exact full text nodes, excluding covered paragraph
nodes and code/script/style ancestors. Wrappers are removed on mode exit and
before reannotation; no stored article HTML is rewritten.

Regression coverage checks orphan-text hover/click, completed word cards, stable
following-paragraph/toolbar coordinates, exact whitespace, repeated annotation,
HTML restoration and rejection of partial paragraphs/code/arbitrary source.

Affected checks and full release gate passed: 133 frontend tests and 36 browser
scenarios, plus real PostgreSQL/YDB/Chromium and backup/restore acceptance.
Deployed `inoreader-app:translation-orphan-20260927`, image
`sha256:dd86ef145ae4501f216b6c72e0176da4e29f58fc369fbbdce624b917e227f20c`;
readiness returned 204. Live smoke on the exact marked text node (248 characters
including surrounding whitespace) completed in 3.48 seconds: 18 dictionary entries,
source preserved exactly, pinyin present, cached reopen successful. Estimated
provider cost $0.000589836; temporary session cleanup verified. Evidence is in
ignored `.inoreader-state/deployments/translation-orphan-20260927/smoke-report.json`.
