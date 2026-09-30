# Grouped reading alternatives and ordinary reading feedback

Three interactive alternatives are prepared for user selection; grouped reading is not yet implemented in the application:
1. Digest grouped by subscription, with summaries on the left and per-article feedback on the right.
2. Subscription navigation with a full-width list of summaries for the selected source.
3. Parallel source columns with per-article summary/rating/reason cards.

All alternatives retain rated cards in place during interaction. Example content is illustrative. Local interactions and overflow were checked at 1024, 736, and 360 pixels in light/dark themes.

Implemented separately:
- Deleted automatic floating-chat opening on ordinary article selection. Summarize explicitly opens the saved conversation; returning from focused reading or switching articles hides the overlay. Focused reading's embedded assistant is unchanged.
- Explicit single-article Mark read opens an optional 1–10 rating and explanation dialog. With empty feedback, “Без оценки” retains the ordinary read action. Typed feedback cannot be silently discarded by that button.
- Rating/reason/read use the existing atomic completion endpoint. Pending saves lock all controls and dismissal; uncertain retries reuse the exact receipt. Drafts are scoped by account/workspace/article and restored before paint. Pending operations reopen for recovery on reload, including when the server already committed them.
- Reader mutation tracking now serves ordinary state writes and rated completion, preserving stale-response fencing, optimistic rollback, stable article rows and unread counters. Existing automatic read-on-open and bulk mark-read behavior are unchanged.

No backend or schema changes for this task. No additional background AI requests. Work remains uncommitted alongside prior changes.

Verification and deployment:
- `just check-affected` passed.
- Final `just check-release` passed: 193 frontend unit tests, 87 browser scenarios without retries, Rust and PostgreSQL/Chromium acceptance and backup/restore checks, tooling tests.
- Deployed the five changed frontend source files to the already authorized 158.160.186.87 and rebuilt/recreated the app container. No schema update was required.
- Live desktop/mobile smoke passed: no automatic chat/history fetch, manual saved-chat opening, optional feedback modal, immediately restored draft, stable toolbar, no JS errors. No article write or paid AI call was made; before/after reading state matched and the temporary session was revoked.
- Initial live smoke caught draft restoration after first paint; changed it to a layout effect and added an immediate-value regression assertion, then repeated the full gate and live check successfully.
