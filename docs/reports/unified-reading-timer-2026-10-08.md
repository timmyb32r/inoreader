# Unified reading timer — 2026-10-08

Implemented selected variant 1: a single upper-right panel displays the active smart/random phase and the remaining time for both phases, pause, stop, progress, and return to the current article.

The account-owned session uses absolute deadlines. Switching browser tabs, opening another application route, and reloading do not pause it. Only explicit pause stops the clock. Phase transitions occur after the current article is completed. Returning restores the article and scroll position and provides immediate pending feedback with duplicate activation protection. Starting a reading session stops the separate general countdown.

The panel has reserved desktop/mobile dimensions; wiki and reader content start below the shared header. No database, budget or configuration changes.

Verification: full `just check-release` passed, including Rust tests, real PostgreSQL/Chromium and backup/restore acceptance, 218 frontend tests and 121 browser tests. Regression coverage includes real browser tab switching, route navigation, reload, scroll restoration, pause/resume, phase transitions, account isolation, invalid saved sessions and stable pending controls. Desktop and mobile screenshots were inspected.

Deployment candidate: `inoreader-app:unified-timer-20261008`, image `sha256:10c581a422d4316844aa1686f1621f0540540402fd2f9bd79d2461dc69000671`. Previous image retained for rollback: `inoreader-app:terms-once-20261008`.

Deployment completed; running image matches the candidate. Local and public `/live` checks passed. Final `just check-affected` passed.
