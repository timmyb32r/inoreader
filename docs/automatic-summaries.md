# Automatic article summaries and daily budget

Approved by the owner on 2026-09-29; schema release 8. Applies to the account's
workspaces using its encrypted DeepSeek key. Read archive is not backfilled.

## Delivery and conversations

Startup enrolls configured, approved accounts before ingest workers start. An
atomic account enrollment snapshots unread articles exactly once. PostgreSQL
triggers add future articles and first full-text publications to a durable
`ai_summary_queue`; candidates wait for the full manifest before snapshotting.
All workers use the existing article-owned chat creation lock, so opening a post,
background discovery, multiple tabs and restarts reuse one conversation. Existing
historical conversations are retained; the latest is reopened, never regenerated.

The selected summary model produces a complete preview; the independently selected
fact-check model checks it. Both default to Flash.
Review replaces the preview without changing scroll position or active targets.
In ordinary Reader, selecting a post does not open a chat or promote work. Summarize
explicitly opens the saved conversation. Pending or failed full text disables
Summarize. Changing articles or returning from focused reading closes the floating
widget. Focused reading still loads the saved chat in its dedicated assistant pane.
Other pages and modal settings hide the chat. Follow-up questions stay in it.

`automatic_attempts` (production 3) and `automatic_retry_seconds` (production 60)
bound setup retries before a chat has been created. They do not retry paid requests.
A failed/interrupted check preserves the draft and requires manual retry, even
after restart or midnight. A request deferred before budget admission may resume
on the next Moscow day. Unknown usage keeps its reserve. See
[targeted review](targeted-review.md).

## Budget contract

`daily_limit_usd` is an explicit positive decimal (production `3`); no float is
used in enforcement. All five modes share one owner/day ledger: summary,
verification, chat, translation, terms. Daily attribution uses request admission
in Europe/Moscow, including requests finishing after midnight. Other applications
using the key are outside this accounting boundary.

Every paid request must first persist a reservation. A PostgreSQL advisory lock
serializes admissions for the owner across workers and modes. Reservation covers
UTF-8 byte-token input bound plus configured framing, both input tariffs as a
conservative bound, and the entire configured maximum output. The second request
reserves separately; a preview can wait until tomorrow for its review.

Reported tokens settle the reservation at the immutable tariff snapshot. Missing
usage retains the full bound; timeouts and shutdown never fabricate zero cost.
Uncertain amounts remain in the historical day, including after key removal.
No automatic refunds. A limit can be reached before exactly $3 is spent because
the next complete request must fit too. Automatic summaries defer to next Moscow
midnight; manual requests receive `ai_daily_budget` (HTTP 429) or a persisted
budget error when concurrent work consumed the remaining capacity after enqueue.
Previously generated content remains readable.

Prices are conservative **peak-rate estimates**, not DeepSeek invoice totals.
Current deployment tariffs agree with [DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing/)
checked 2026-09-29; off-peak billing can be lower. The profile displays exact
estimated amounts and reserves separately, with a 30-day graph split by mode.
Accounting starts at activation; no guessed per-request dates are backfilled
from old conversation creation dates. Operator tariff changes affect newly admitted requests; already admitted requests
retain their recorded rates. Profile changes apply at that same request boundary.

## Verification and operations

`ai_spending`, `ai_automatic_accounts`, `ai_summary_queue` and chat priority are
additive schema objects. Upgrade requires a verified PostgreSQL backup and a
restored-copy rehearsal. Queue and ledger are included in backup/restore tests.

Tests cover concurrent admission, account isolation, idempotent settlement,
unknown reservations, Moscow rollover, no calls while deferred, single unread
backfill, new arrivals, complete automatic two-stage execution and reuse. Browser
coverage checks explicit opening and article switching, closing/reopening, stable targets, existing
translation/wiki flows, preview replacement and per-mode graph data.

## Date-specific limit exceptions (schema 9)

`ai_budget_overrides(owner, day, limit_usd)` records explicit administrator-approved
exceptions. `day` is a Moscow calendar date. Admission and the profile use the
same effective limit; absent/expired exceptions use `daily_limit_usd` (3).
On 2026-09-29 the owner requested 5 USD for that date only. No timer or cleanup
is required to return to 3 at midnight. Historical spend/reserves are preserved.
When raising a limit, wake only that owner's queued budget-deferred jobs; they
must still reserve again before calling DeepSeek. Never refund unknown charges.
