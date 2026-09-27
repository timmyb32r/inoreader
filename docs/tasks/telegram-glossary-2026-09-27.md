# Telegram glossary implementation — 2026-09-27

Approved spec: [telegram-glossary-spec.md](../telegram-glossary-spec.md).
User explicitly requested implementation. Standing deployment authorization covers
158.160.186.87. Bot not created yet (user answer); do not fabricate live delivery.

## Execution ledger

- Implemented: validated glossary domain, lossless archive/Telegram parsing and tests.
- Implemented: PostgreSQL raw archive, exact definitions, connections and fenced jobs.
- Implemented: native Telegram polling/reconciliation, settings and encrypted credentials.
- Implemented: Flash definitions provider/service/API and account isolation.
- Implemented: article action, floating panel/rich copy and channel settings UI.
- Passed: `just check-affected` and full `just check-release`, including real
  PostgreSQL, seven-table dump/restore, YDB/Chromium Docker acceptance, 133 frontend
  unit tests and 37 browser tests. Final v2 release gate passed; affected checks
  took 4.9 seconds.
- Backed up: production PostgreSQL (835 MiB custom archive, readable TOC), config,
  existing master key on the server, and pre-deployment application image.
- Imported: all 1,529 posts; 836 definitions / 808 exact names; 0 unindexed posts;
  six missing dates retained. Repeating import left all 1,761 raw archive receipts,
  current posts and definitions unchanged.
- Deployed: schema, archived index, settings, article panel and final prompt v2
  container on 158.160.186.87; container healthy.
- Production Flash smoke passed: 36 entities, 1 exact known definition, 9.99s,
  estimated $0.004708908. Reopening reused the same paid job. Authentication and
  CSRF rejection verified; temporary test session removed. Earlier failed response
  remains retained for diagnosis; no automatic paid retry was introduced.
- Production smoke caught two longer evidence quotes with changed whitespace.
  Removed redundant model-generated quotation: exact source name is the evidence
  fragment. Added a regression for SQL/OLAP surrounded by original line breaks;
  no normalization, hidden repair or dropping of entities is permitted.
- Network prerequisite: VM TCP connections to both t.me:443 and api.telegram.org:443
  time out; public reconciliation exposes its failure in Settings. No proxy or
  relay has been enabled implicitly.
- External prerequisite: user creates bot and adds it to @reading_data_news;
  live delivery verification must remain explicitly outstanding until connected.

All edits belong to root; bounded exploration agents are read-only. No research
budget is consumed automatically and no messages are sent to the channel.
