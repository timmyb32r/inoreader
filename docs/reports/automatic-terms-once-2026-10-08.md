# Automatic term extraction repeats — 2026-10-08

## Cause

Automatic eligibility and exact-input caching included technical source revision
and refresh IDs. A periodic source refresh therefore admitted the same article
again, even with byte-identical extracted text. After 13:55 Moscow, 627 completed
term jobs covered 260 articles; 367 were repeats. One Google Cloud article was
processed 13 times, with identical text in its last 11 attempts.

The 926 summaries shown in the day's screenshot were separate article jobs;
847 concerned older publication dates and ran before the earlier queue clear.
Historical spending and request counts are not rewritten by this fix.

## Change

Automatic terms admission is owned article + prompt version, irrespective of
refresh metadata or text changes. Manual extraction remains available. Manual
cache equality compares the exact provider request plus prompt version, while
retaining the complete original snapshots. Existing automatic queue duplicates
are cancelled without deleting their records, operation history or paid results.
A second duplicate check runs before reserving money. Failed/rejected automatic
attempts also remain blocked across source refreshes.

Schema 19 adds lookup indexes and the duplicate predicate; it does not rewrite
article, job, billing or response records during migration.

## Verification

`just check-release` passed on the final implementation: formatting, Clippy,
workspace tests, real PostgreSQL/Chromium and backup/restore acceptance,
211 frontend tests and 121 browser tests. Database regressions cover semantic
request caching after refresh, one automatic article/prompt attempt, legacy queue
cancellation with unchanged input and completed output, and denial at the final
paid admission boundary with zero spending reservations. A v18-to-v19 upgrade is
rehearsed alongside earlier supported schema upgrades. `just check-affected`
also passed (Rust: 9.35 seconds, plus frontend type checking).

A fresh custom PostgreSQL backup was validated by fully decoding it with
`pg_restore --file=/dev/null`; Docker recorded exit code 0. The verified archive
is retained as `inoreader-backups/terms-once-before-20261008.dump` on the server.

## Deployment

Deployed image `sha256:68a1ca3b9e7c2962a637e92cad85eb970d261f0bdbff918f2dd6c7d17d966cde`
and schema 19 to `158.160.186.87`. All preexisting table row counts matched before
and after the migration. The application restarted and passed liveness checks.

The first owner-scoped queue check found 170 cancelled automatic duplicates and
zero queued superseded jobs; 20 remaining queued jobs were not duplicates.
Cancellation records retained their inputs, had no paid usage or raw provider
response, and did not mask completed results. Today's budget override stayed
$6. Authenticated day/month/year statistics agreed at 2,714 admissions; anonymous
access and forged owner parameters were rejected. The temporary verification
session was removed.
