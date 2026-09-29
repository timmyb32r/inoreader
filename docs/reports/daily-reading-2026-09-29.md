# Daily article reading activity — 2026-09-29

Replaced Home's browser-local elapsed-time calendar with server-backed daily counts
of distinct articles explicitly marked read. Added today's count and the total of
daily counts over the displayed 364 days. See [contracts](../reading-activity.md).

The event journal is committed in the article mutation transaction, including bulk
updates. Repeated true writes, failed CAS, automatic rules, initial imports and
Read later changes do not count. Repeated reading of the same article on the same
day counts once; changing back to unread does not erase history. Dates use the
browser's IANA timezone, validated and grouped on the server. Workspace ownership
protects the endpoint; successful responses are `Cache-Control: no-store`.

Schema 7 adds an empty journal; existing read flags cannot supply historical dates.
Existing local time data remains untouched and is no longer displayed.

Validation: `just check-release` passed (`/tmp/reading-release-calendar.log`), including
183 frontend tests, 55 browser fixture scenarios, real PostgreSQL + backup/restore,
6 real-backend browser scenarios, Chromium Docker acceptance, Rust fmt/clippy/tests,
architecture, generated API contracts and operational checks. Regression coverage
includes atomic counting, repeats, conflicts, timezone boundaries, isolation, schema
6 -> 7, restoration, pending state, request deduplication and stable UI geometry.

Deployment: rehearsal on a restored production backup passed; schema migration took
14 seconds, preserving all preexisting table counts. Production schema 7 deployed
with a fresh stopped-writer backup. Temporary rehearsal database removed; protected
backups and count comparisons retained under `.inoreader-state/reading-release/`
on the server. Live HTTPS API and Home browser checks passed; temporary owner
verification sessions were revoked. Calendar weekdays align to Monday-first rows.
