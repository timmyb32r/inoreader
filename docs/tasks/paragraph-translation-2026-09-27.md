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
