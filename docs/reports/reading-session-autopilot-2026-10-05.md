# Reading session autopilot

Started 2026-10-05; completed and deployed 2026-10-06 (Europe/Moscow).

## Authoritative obligations

- verified: Enter submits article discussion to the server; Shift+Enter inserts a
  newline. IME composition does not send, and pending sends cannot duplicate.
- verified: the feedback dialog includes the rating picker and allows changing
  the selected score while retaining its reason draft. Pending/uncertain receipts
  lock editing until resolved.
- verified: an additional random smart feed and reader retain their explicit seed,
  use stable keyset pagination and enforce owner/workspace isolation. The existing
  ranked feed remains available; predicted1 articles stay hidden in random mode.
- verified: Home opens the reading wizard, with defaults45 smart minutes and15
  random minutes. The reader labels its current phase, supports pause/reload,
  excludes hidden-tab time and switches only after the current article. Invalid
  plans and corrupt saved sessions fail explicitly before reading starts.
- verified: Mark all read requires confirmation and gives immediate pending
  feedback. Its configured batch capacity is no longer a total selection cap.
  Owned scope updates remain atomic across batches, including read events; a late
  failure rolls everything back. Unknown JSON metadata and exact timestamps survive.
- verified: persisted read provenance distinguishes reader completion, individual
  marking and Mark all read. Existing events retain identity/time and become
  explicitly unknown; no unrelated rating is used to invent historical evidence.
  Bulk marking creates no ratings. Undo shows unread while preserving history;
  retries create no extra events. The metadata row and adjacent wiki target stay fixed.
- verified: final affected check, full release gate, full-data upgrade rehearsal,
  production deployment and read-only HTTPS smoke.

## Verification

`just check-affected`, `just check-release` and `git diff --check` passed on the
final code tree. The full gate includes formatting, Clippy/all Rust targets,
real PostgreSQL and backup/restore Docker acceptance, real browser/backend
acceptance, Chromium ingestion acceptance,206 frontend unit tests,116 browser
E2E, and architecture/operational asset checks. Browser geometry checks cover
mobile/desktop, pending/error/retry states, session transitions and provenance.

The existing macOS socket fixture now explicitly makes its accepted socket
blocking before its bounded read. The real-backend translation test waits for
its explicit interactive marker after reload. Backend acceptance has its own
artifact directory, so independent browser suites cannot delete its traces.

## Deployment and preservation

Production: https://inoreader.duckdns.org, schema17,
`reading-session-provenance-2026-10-06`.
Image: `inoreader-app:reading-session-20261006`,
`sha256:fdf40c1dcde3bc12d3bb1d2407cb36f922dd23cce8a732da9d35d9d430656d50`.

A complete production backup was restored into an isolated database before
rehearsal. Exact retained rows in24 user-data tables matched before/after both
rehearsal and production upgrade, excluding only the newly added event-method
column. Rehearsal migrated7134 historical events; the fresh production snapshot
migrated7135. All became unknown without changing their timestamps/revisions.

The old application was drained before the fresh production backup and snapshot.
The temporary rehearsal database/config were removed after verification. Full
backups, the previous image and losslessly compressed comparison evidence remain
under `/home/timmyb32r/inoreader-backups/`. User configuration/secrets and DeepSeek
budget settings were unchanged. Backup retention takes priority over the generic
free-space target: the server had16GiB free after compressing our evidence; no
user data or unrelated caches were removed to increase that number.

The deployed container is healthy; local liveness and certificate-verified public
HTTPS passed. Anonymous random-feed access returns401. Public frontend assets
contain the new session/provenance UI. Bootstrap remains10 articles and all491
existing subscriptions remain ready. No paid generation was used as a smoke test.

## Contracts and design choices

See [read provenance](../designs/read-provenance.md). `readMethod` is a nullable UI
projection of the owned event, with a closed storage/DTO enum. History selects the
method of its event within the requested period, rather than a later read's method.

Session progress is local UI state, scoped by account/workspace/session. Authored
minute inputs and restored state cross validated factories before use; durations
are never sent to the backend or used as billing/authorization inputs. The
controlled hook owns mutations; this deliberately avoids a separate backend
session execution object in the experimental UI.

Random ordering uses PostgreSQL's native noncryptographic hash in the shared SQL
keyset query. This avoids fetching all unread articles into Rust or installing a
new database extension. The seed is explicit and fenced in the cursor; no article
identifier or content is replaced by a hash.
