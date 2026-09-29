# Automatic summaries deployment — 2026-09-29

Deployed to the authorized production host, 158.160.186.87.
Image: `sha256:4b2b6539e4beb42064e6e0ba609e08d9cd2cc669b9a8d3eddb5d7098a6d5b5af`.
Schema: 8. Daily shared limit: USD 3, Europe/Moscow. Automatic summaries enabled;
three attempts, 60-second retry cooldown. See [contract](../automatic-summaries.md).

## Verification

- `just check-affected` passed.
- `just check-release` passed: formatting, Clippy, Rust tests, real PostgreSQL,
  backup/restore, Chromium, real-backend browser acceptance, frontend unit and
  browser suites. One existing profile-save browser test passed on retry;
  its expectation was corrected for automatic chat opening, then passed three
  consecutive runs without retries.
- Restored the production backup into a separate database and rehearsed 7→8.
  Every existing table retained its row count; schema history gained one row.
- Drained the application, made a protected final backup, migrated production,
  compared every existing table count, then recreated the application container.
  Search retained 19,910 indexed rows. Removed only the rehearsal database.
- Authenticated live browser confirmed automatic chat opening, absence of the
  regeneration control, and the per-mode spending chart. Temporary test session
  was deleted afterward. No test follow-up messages were sent.

## Initial live observations

595 queue candidates at the first snapshot, 602 at the later snapshot as new
content arrived. Two newly created chats had completed both stages; further
work was queued or verifying. Six generation usage records and four verification
usage records had settled. Total settled estimates plus reserved amounts remained
within USD 3 per account/day.

A provider response changing the original title was rejected explicitly rather
than published as verified; the bounded retry path handles that error. Candidates
whose manifest did not provide usable full text reported the explicit full-text
error; RSS excerpts were not silently substituted.

Backlog processing continues under the daily budget; these observations do not
claim the full backlog has already completed. Spending estimates begin at
activation and are not a reconstruction of historic provider billing.
