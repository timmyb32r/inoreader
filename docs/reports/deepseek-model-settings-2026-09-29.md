# DeepSeek model preferences and dated budget exception

User request: separate profile choices for summary and fact-check, both Flash by
default. Raise the owner's limit to USD 5 on 2026-09-29 only, then use USD 3 again.

Implementation:
- Closed Flash/Pro model choices; account preferences persist independently of keys.
- Read choices before each paid request, including queued work and retained drafts.
  Follow-up chat uses the summary choice. Prompt, article, mode and input limits
  stay pinned; finished content is never regenerated.
- Mandatory per-model deployment tariff catalog. Persist each admitted call's
  model/rates atomically with its reservation, and validate reported costs against
  that immutable snapshot. Historical costs and original snapshots are preserved.
- Stable profile selectors and Save models button with immediate pending feedback,
  duplicate-activation guard, and existing reserved success/error area.
- Schema 9 adds preferences, per-call selections and owner/Moscow-date budget
  exceptions. Date is captured once under the budget lock; no reset cron required.
  All five modes share the effective dated limit.

Verification:
- `just check-affected`: passed.
- `just check-release`: passed; Rust/Clippy/format, real PostgreSQL acceptance and
  backup/restore, Chromium acceptance, frontend units, 67 browser scenarios and
  architecture/assets checks. Evidence: `/tmp/models-release.log`.
- PostgreSQL regression exercises preference edits during an in-flight request,
  Pro/Flash price separation, retained-draft retry on a changed checker, owner
  isolation, default Flash/Flash and date-specific limit expiration.
- Browser regression verifies independent choices, reload persistence, immediate
  pending, one mutation on double click and unchanged control geometry.
- Production database clone upgrade 8 -> 9 passed, preserving every preexisting
  table count (19,934 indexed articles). Protected dump retained on the server in
  `.inoreader-state/ai-model-release/`.

Deployment/live verification completed:
- Image `sha256:2271b365585222ee976c2f251bd21449814f38eb8f1d19042a1b27e66cebd25e`
  is running on the authorized host. Offline production upgrade preserved all
  preexisting table counts, including 19,941 search rows at shutdown.
- Authenticated API and Chromium confirm Flash/Flash and the effective USD 5
  limit for 2026-09-29. Invalid model PUT returns HTTP 422 without mutation.
- The first 13 newly admitted background calls all recorded `deepseek-flash`.
- The date-specific exception is one owner row; default deployment limit remains
  USD 3. Existing spent amounts/reserves were retained, not reset.
- Temporary verification session removed; rehearsal database removed after use.
  Protected backups remain on the server. Live screenshot is in ignored
  `.inoreader-state/ai-model-release/live.png`.
