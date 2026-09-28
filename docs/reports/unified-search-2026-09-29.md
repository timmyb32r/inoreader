# Unified search — design 2

Implemented and deployed the full search page: results on the left, authorized
article/wiki preview on the right; All / News / Wiki filters; URL-preserved query,
scope, pagination and selection; header/sidebar/Cmd-or-Ctrl+K entries. The wiki
search field uses the same page with its namespace selected. Catalog and trash
filters remain local management filters.

Search covers owned workspaces and explicitly accessible wiki namespaces. Admin
status does not grant wiki access. Drafts, history and deleted wiki pages are not
searched. Queries and excerpts remain text, with literal case-insensitive matching
and highlighting. Preview does not mark articles read. A late wiki listing no
longer clears text already entered in its search field.

## Verification

Final `just check-release` passed (`/tmp/search-release-final.log`): Rust formatting,
Clippy, workspace tests, real PostgreSQL/browser/backup-restore acceptance, Docker
Chromium acceptance, 183 frontend tests, 54 browser scenarios, architecture checks,
source inventory and operational tests. Live Chromium rendered the real search
page and selected article (`/tmp/reader-search-live.png`). Temporary verification
session was revoked and its local credential file removed.

SQL plan comparison on the full restored database produced exactly equal decoded
result rows for all four test queries. Materializing the result page before reading
full-body excerpts reduced warm Postgres-query execution from 2198 to 1142 ms and
ClickHouse from 721 to 413 ms. `etlworks` remained approximately 24 ms. Short CJK
substrings such as `数据` still require broad text scanning (~3.9 seconds); trigram
indices cannot accelerate every one/two-character substring.

## Deployment and preservation

Host: `158.160.186.87`. Schema release 6: `unified-search-2026-09-28`.
Image: `sha256:ed587b4a3eea5ab247d7cae3d4197c38df8db0a340a031fb9814fd0d37266c0e`.

A protected backup was restored to a separate database. Backfilling 19,793 records
and building indices after the backfill took 86 seconds; all preexisting table
counts were preserved. The initial per-insert-index experiment was cancelled on
the test copy and rolled back before this successful rehearsal.

Production was drained before a fresh backup and migration. All original table
counts were retained, with the expected release-journal addition. The new
projection contained 19,799 rows, matching content manifests. Original article
and content tables are not rewritten by this migration. `/live` and `/ready`
returned 204; the container was healthy.

Server operator artifacts are protected under:
`/home/timmyb32r/inoreader/.inoreader-state/search-release/`.
This includes `production-before.dump`, `config.before.yaml`, previous image ID,
before/after row counts, migration logs and timing reports. No credentials are in
this report or the source archive. The rehearsal database is disposable; the
protected backups remain available for recovery.

Live authenticated smoke checks:

- `ClickHouse`, News: HTTP 200, 25 hits, 591 ms including HTTP.
- `etlworks`, Wiki: HTTP 200, 2 hits, 77 ms.
- `数据`, All: HTTP 200, 25 hits, 4080 ms.
- Authorized article/wiki previews: HTTP 200.
- An inaccessible namespace: HTTP 404.

See [search contracts and operations](../search.md).
