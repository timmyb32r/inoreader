# Arrived/read daily chart — 2026-09-29

Home now shows adjacent Arrived and Read bars for the last 30 local calendar days,
a common integer scale, immediate hover/focus tooltips and two today counters.
The annual read calendar remains below the chart. All chart geometry is reserved
before data arrives; error feedback replaces the existing subtitle without shifting
controls. Colours reuse slate/teal semantic tokens.

The existing authenticated activity response now includes `arrived`. A single SQL
snapshot aggregates first arrivals and explicit reading events for the same workspace,
timezone and date window. Arrival counts include read/unread articles and retain
history after subscription archiving/removal. Nanosecond source timestamps are not
rounded across midnight or rewritten. No database migration was needed.

Validation: `just check-affected` and `just check-release` passed. Final gate log:
`/tmp/flow-release-verified.log` (185 frontend tests, 55 fixture browser scenarios,
6 real-backend browser scenarios, Rust tests/fmt/clippy, PostgreSQL backup/restore,
Chromium Docker and architecture/operational checks). Tests cover arrivals without
reads, stable arrival counts after read changes, timezone/nanosecond boundaries,
tooltip immediacy, common scale, loading/error handling and stable control geometry.

Deployed to 158.160.186.87, image
`sha256:950891c4c514f35ff4f9d2e25482831051c40b033399f4e864f2393c5020e3c8`.
Container healthy, readiness 204; live authenticated API and browser chart checks
passed. Temporary verification session revoked. Previous image retained under
`inoreader-before-flow:20260929`. Live screenshot: `/tmp/reader-flow-live.png`.
