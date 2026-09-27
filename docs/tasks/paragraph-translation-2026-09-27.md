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
