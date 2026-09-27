# Remove YDB storage — 2026-09-27

The user requested removal of YDB storage. PostgreSQL is the sole runtime backend.
The standing deployment permission covers 158.160.186.87.

Removed the storage crate, SDK, one-way migration command and importer, migration
configuration, Compose credential mount and environment reference, local YDB
Compose overlay, and YDB backup/restore wrappers and Docker fixtures. Cargo.lock
lost 84 packages without adding packages. Retired configuration and CLI commands
are explicitly rejected by regression tests.

Updated current architecture, product specification, operations, CI and release
checks. Historical execution reports retain the names of tests actually run.
The PostgreSQL Docker acceptance now restores all 45 tables into a separate DB,
compares every row and column/index contract, resumes a persisted queued job,
and checks that the source DB remains unchanged. Catalog names differ explicitly
because the restored test database has a separate identity.

Validation: `just check-affected` and `just check-release` passed, including Rust
format/Clippy/tests, real PostgreSQL and Chromium, frontend unit/browser tests,
architecture and operational checks. The seven Python operational/seed tests now
run in the release gate and CI as well.

Deployment: previous image and exact config/Compose saved under the private
remove-ydb-20260927 deployment directory. No database schema changes or user-data
deletions are part of this change. Historical databases, volumes, archives and
credential files are preserved; the app no longer mounts or uses YDB credentials.
Deployed and verified: container healthy, no YDB environment entries or secret
mounts. Authenticated bootstrap, feed and glossary APIs return 200; unauthenticated
bootstrap is rejected. Temporary verification session was removed. Preserved data:
211 subscriptions, 9,792 articles, 9,576 content manifests, 1,529 channel posts and
836 glossary definitions. No paid AI requests were made for this deployment.

Final release evidence: 198 Rust tests, 133 frontend unit tests, 37 browser tests,
and 7 Python operational/seed tests passed. The first extended backup test needed
its separate catalog identity and queue priority accounted for in the fixture;
the complete gate passed after those test corrections.
