# Compact chat and early Flash summary — 2026-09-27

The owner explicitly requested a much simpler chat, icon-only copying, removal of
its version selector/profile shortcut, Flash by default, and an immediate first
summary with background fact-checking and later replacement. This supersedes the
previous requirement to hide all first-pass summaries until verification finishes.
No additional questionnaire or research-quality acceptance is inferred.

## Behavior and preservation

- New summaries and regenerated versions use `deepseek-flash`, non-thinking
  (standard mode, temperature0.3); the checker remains Pro/thinking-low. Both
  original style and review prompts are unchanged. New Flash tariff estimates
  use the published peak prices: USD0.3 input,0.006 cached input,1.2 output per
  million tokens. [Source checked 2026-09-27](https://api-docs.deepseek.com/quick_start/pricing/).
- After the first complete response passes transport/quote validation,
  `ChatRecord::into_public_view` exposes its retained draft as the incomplete
  summary's content. Before that, summary content is empty. Partial verifier
  output never leaks into that projection. Final verification replaces the
  preview only after its existing checks pass. Status and the UI distinguish
  unchecked previews from final summaries.
- Failure, cancellation, worker loss and review-only retry retain the original
  preview. Draft envelopes, old conversation versions, frozen settings, exact
  billing and unknown usage are not deleted or rewritten. No schema migration.
- The widget removes the version row, Profile buttons, accounting panel and
  persistent technical commands. Copy, send and new summary use labelled icons;
  copying has immediate pending/success/error feedback and duplicate protection.
  Stop/retry/reconnect appear only when useful, in reserved fixed positions.
- The status row and composer have fixed sizes. Remote text replacement is
  deferred during a pointer press, text selection or scrolling (180ms idle).
  Replacement retains scroll position and minimum reading-surface height. Menu
  layering was corrected so the account's Profile remains reachable over chat.
- Sending follow-ups still waits until checking finishes; typed drafts are kept.
  Existing conversations retain their original models. The header's New summary
  icon explicitly creates a new version using Flash; opening saved chat is free.

## Verification

- `just check-affected`: PASS,6.418s.
- Focused real PostgreSQL acceptance: PASS; confirms preview availability during
  a blocked verifier, hiding partial verifier output, account isolation,
  malformed-envelope/forged-quote rejection, cancellation, restart and retry.
- `just check-release`: PASS. Rust202 tests, separate Chromium extraction3,
  frontend126 tests/20files, browser31, real PostgreSQL/YDB, Docker backup/restore,
  architecture and operational asset checks. Log `/tmp/inoreader-flash-release.log`.
- Eight chat browser scenarios include early preview/final replacement, retained
  reading coordinates, held pointer, selected text, error/retry/stop, duplicate
  prevention, responsive geometry and Profile/key flows. Screenshots were
  inspected under `web/test-results/deepseek-early-summary-is--b557f-ading-position-and-controls-chromium/`.
- The first browser run found Profile's account menu behind the chat; z-index was
  corrected and the complete gate passed. No forced clicks or skipped tests.

## Deployment

Deployed to `158.160.186.87` / <https://inoreader.duckdns.org>.
Running image:
`sha256:24929210d1617d58f2e9630ae8573f51c7f84c5777c0961ea9b9a851e4ecbdc6`.
Tag `inoreader-app:flash-chat-20260927`, also `inoreader-app:latest`.

Original config/rollback metadata:
`/home/timmyb32r/inoreader-backups/flash-chat-20260927T092356Z` (root0700,
files0600). Prior image retained as
`inoreader-app:before-flash-chat-20260927t092356z`.
The existing config prefix was preserved byte for byte; only the AI model,
mode and Flash tariff fields changed. Key files, provider credentials, owner
allowlist, prompts, database schema and chat snapshots were not changed.
Zero active AI jobs were confirmed before restarting only the app container.
Compose application configuration validation and actual startup passed.

Production smoke at `2026-09-27T09:24:45.781984+00:00`: live/ready204,
unauthenticated profile401, owner profile200 with existing key enabled,
wrong-origin balance403 before provider I/O, both saved conversations readable
with preserved Pro snapshots (one and three messages). Temporary five-minute
session deleted and absence verified. Report under ignored
`.inoreader-state/deployments/flash-chat-20260927/production-smoke-8983391c-d02a-458d-9902-220f398b7efc.json`.

No new paid provider request was made for this change and no seconds-to-answer
benchmark is claimed. Earlier Pro/high style scores and timing measurements do
not measure the new Flash configuration. The visible wait now ends after the
first generation, without waiting for background verification.
