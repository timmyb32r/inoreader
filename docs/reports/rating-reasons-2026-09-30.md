# Rating explanations — 2026-09-30

Implemented selected variant 3 in focused reading: rating → Read & next → optional explanation dialog → Save & next. Closing the dialog preserves the draft; sessionStorage scopes it to owner/workspace/article and retains it across tab reloads. Saving is atomic with the rating/read state. Uncertain responses replay the same command, including exact explanation. Undo restores the previous rating and explanation. Completion receipts preserve submitted feedback for future training; no training is performed.

Schema 13 adds nullable article_ratings.reason. Nonempty text is preserved without trimming or truncation; U+0000 is rejected before persistence. Empty UI input explicitly means no explanation. Existing ratings and receipt documents remain unchanged.

Verification:
- `just check-affected` passed.
- Final `just check-release` passed, including Rust checks/tests, real PostgreSQL upgrade and backup/restore acceptance, 193 frontend unit tests, 85 browser scenarios without retries, and tooling tests.
- Related focused-reading/translation browser suite: 15 passed with retries disabled.
- Production desktop/mobile checks confirmed autofocus, draft retention, stable footer coordinates, and no JavaScript errors. Checks made no article writes or paid AI requests; temporary authentication was revoked.
- Screenshots: `/tmp/rating-reason-production-1440.png`, `/tmp/rating-reason-production-390.png`.

Deployed to 158.160.186.87. Protected ratings/receipts backup: `/home/timmyb32r/inoreader-backups/rating-reasons-20260930T030619Z`. Before/after ordered exports of existing rating values and completion documents compare byte-for-byte equal. Schema 13 and the deployed UI were verified. Deployment initially stopped after schema upgrade because the Compose command consumed the SSH script input; the remaining comparison/start steps were executed from a separate script, and application availability was restored before the successful production checks.

Changes remain uncommitted alongside the preceding fact-check settings work.
