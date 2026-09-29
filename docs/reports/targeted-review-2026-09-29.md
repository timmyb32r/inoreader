# Targeted factual review — 2026-09-29

Owner approved targeted replacements, manual-only retries, Flash/low verification.
Specification: [targeted review](../targeted-review.md).

The application now supplies the source heading, the checker returns anchored
corrections rather than the full summary, and publication validates the complete
patch atomically at provider and storage boundaries. Unchanged text stays exact;
invalid/overlapping/ambiguous patches retain the original preview. Paid responses
and actual prompts are retained privately, including rejected model replies.

Removed the automatic paid-call retry selector and exact-title rejection. Failed
checks show the retained preview and an explicit Retry verification action. Existing
model preferences, reasoning settings, source snapshots and daily limits remain.

Verification: just check-affected; just check-release (exit 0), including Rust,
PostgreSQL/Docker acceptance, schema 10→11, server-side patch revalidation,
manual-only retry after restart, stage-only/idempotent manual retry, private raw
response retention, backup/restore, Chromium, 193 frontend and 74 browser tests.
Logs: /tmp/review-affected.log and /tmp/review-release-final.log.
An initial PostgreSQL container startup timed out; an isolated diagnostic container
started normally, was removed, and subsequent complete acceptance gates passed.
No paid provider experiment or new human evaluation was performed. Future cost
reduction and model factual quality are not inferred from engineering tests.

Deployed to 158.160.186.87. The owner explicitly waived the AI-table backup;
none was created. Native additive schema upgrade 10→11 succeeded, the rebuilt
application was activated, and Docker reports running/healthy.

Stopped all 91 already queued retries from the old scheduler through the Stop API.
Each operation verified unchanged message IDs/content and provider-call history.
The 382 first attempts were left budget-deferred. After restart, queued retries: 0.
Production API confirms deepseek-flash for both summary and verification.

Read-only production browser smoke passed: HTTP 200, saved cancelled verification
preview/status, and Retry verification button rendered. Browser mutations were
blocked, and subsequent API comparison confirmed unchanged messages/provider calls.
Temporary owner session was removed. No paid provider requests were initiated by
verification; existing daily budget configuration was not changed.
