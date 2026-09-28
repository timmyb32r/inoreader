# Wiki implementation ledger

Authority: user approved docs/wiki-spec.md and requested implementation.
Existing unrelated dirty changes are preserved. No sub-agents requested for this task.

| Obligation | Status | Evidence |
|---|---|---|
| Namespace ownership, reader/editor ACL, no implicit admin access | verified | Real PostgreSQL tests cover owner/editor/reader and unrelated admin isolation. |
| Markdown pages and same-namespace missing-page links | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Names, rename aliases, cross-namespace link prohibition | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Immutable history, restore as revision, conflict comparison | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Private persistent drafts, explicit unsaved/pending feedback | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Trash/restore, readers excluded, no hard deletion | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| PostgreSQL scoped search and paginated lists | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Private one-page subscription bindings and reader return navigation | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Separate Wiki UI, role controls, narrow screen and dark theme | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Safety: auth/CSRF/XSS, revocation concurrency, request idempotency | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| PostgreSQL migration, preservation, backup/restore verification | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Unit/integration/browser tests, layout/pending/dedup tests, check-release | verified | Full `just check-release` passed on 2026-09-28, including real PostgreSQL/browser and backup/restore acceptance. |
| Deployment and production smoke check, final spec reconciliation | verified | Deployed to 158.160.186.87 on 2026-09-28; healthy, authenticated Wiki 200, unknown namespace 404, anonymous API 401. |

Plan: implement validated domain contracts and PostgreSQL operations; add HTTP API
and contract generation; add Wiki UI and subscription bindings; run focused tests,
then release gate; deploy with verified backup and verify production isolation.

Production schema upgraded from release 4 to 5. Full backup:
`.inoreader-state/wiki-release/production-20260928T164724Z.dump` on the server;
`pg_restore --list` validated its catalog. All preexisting table row counts were
identical across the offline upgrade. Production smoke sessions were revoked.
Temporary configuration copies remain mode 0600; no credentials were printed.
No source commit was created; preexisting unrelated changes were preserved.
