# Subscription digest — deployed 2026-09-30

Implemented selected alternative 1 at `/digest`. Feed's Reading mode opens it;
One by one preserves the sequential reader. Source-grouped cards show a labelled
preview of the existing summary and inline 1–10 rating plus optional explanation.
Full summary opens without truncating stored text; Read & discuss uses the existing
focused reader and returns to the digest. Successful ratings keep cards in place
until explicit refresh; Feed is refreshed on exit.

Uses the existing authenticated Feed cursor API in batches of up to 50. Group
counts explicitly describe the current batch. Each article appears once under
its first subscription (the same displayed-source convention as ordinary reader).
Visible cards load existing conversations lazily with GET only; no new generation,
queue promotion, polling loop, schema or backend change was introduced.

Extracted ArticleFeedback and its independent styles from MarkReadDialog so modal
and inline scoring share validation, exact-text drafts, pending receipts and
atomic completion. Existing focused-reading CSS no longer depends on shared
feedback import order; embedded Terms header specificity prevents a previously
order-sensitive layout regression.

Verification:
- `just check-affected` passed.
- `just check-release` passed, including Rust, real PostgreSQL/Chromium and
  backup/restore acceptance, 193 frontend unit tests, 89 browser scenarios without
  retries, and tooling tests.
- Targeted digest/ordinary-feedback/focused-reading browser suite: 14 passed.
- Production desktop/mobile smoke verified source grouping, lazy saved summaries,
  full-summary modal, stable adjacent card geometry, exact draft restoration and
  no horizontal overflow. No article writes or paid AI requests were made;
  inspected reading states remained unchanged and the temporary session was revoked.
- Screenshots: `/tmp/digest-production-desktop.png`,
  `/tmp/digest-production-mobile.png`.

Eleven frontend source files were transferred to the user's authorized
158.160.186.87 host. Container rebuilt and restarted successfully. No migration
was needed. Changes remain uncommitted with the preceding work preserved.
