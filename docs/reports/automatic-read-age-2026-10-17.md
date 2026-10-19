# Automatic reading of old incoming articles

The workspace setting `reading_preferences.auto_read_after_days` is an explicit positive integer; NULL disables the policy. This release enables 30 days only for the requesting user's workspace. It is applied in the same transaction as first article delivery, after ownership and deduplication are resolved, before subscription-origin fan-out and automatic AI admission. Other workspaces retain their policy independently, including subscribers to the same public source.

“Older” compares publication calendar day with the delivery UTC calendar day, strictly more than the configured number of days. Zoned publication dates use their UTC calendar day; day-only and unzoned dates retain their declared calendar day, following the existing `PublicationDate::day` contract. This comparison never rewrites source timestamps or invents a timezone. Exactly 30 days, future dates, and absent/invalid publication evidence do not trigger the rule.

Only genuinely new articles are marked. Re-deliveries, identity remapping, pre-existing articles and manual unread decisions keep their existing state. Source records, text, ratings and provenance are retained. The read event has its own `automatic_age` method and is visible in the existing reader metadata slot. Automatic age events do not contribute to personal read-activity counts, create ratings, or admit automatic AI processing while the article remains automatically read. Explicit manual AI actions continue to work.

Schema 20 adds the nullable validated setting and the new read-event method, and updates the common automatic-AI eligibility boundary. No historical article-state sweep is performed.

## Verification and deployment

`just check-release` passed, including real PostgreSQL ownership/shared-source cases, strict age/date boundaries, rejected zero threshold, preservation of explicit unread on re-delivery, automatic-AI exclusion and no increase in personal reading-activity counts. The initial attempt could not reach a stopped local Colima; the saved VM was started and the gate rerun against its actual socket. The final frontend suite passed 218 tests; browser coverage passed 121 tests outright plus one existing paragraph-translation test on retry (reported flaky), including the new automatic-provenance geometry regression. `just check-affected` and `git diff --check` passed.

The new browser case exposed an existing UI bug: opening an already-read article replaced its provenance optimistically with `single`. Opening now marks only unread articles, preserving automatic/bulk/reader provenance for existing read articles.

Before upgrade, a single temporary full production backup was created and completely decoded with `pg_restore --file=/dev/null`. The application drained gracefully, schema 19 was upgraded transactionally to 20, and article/subscription/source-record/content-manifest/chat/read-event counts were compared unchanged before enabling the workspace policy.

Deployed image `inoreader-app:auto-read-20261017`, `sha256:8ce367af556d4dc6f1f78c77ffc49f7e87c14dff42d57a95e883ff58ca4e914f`. Schema version 20 and the owner's 30-day preference were verified, the container is healthy, and public `/ready` returned 204. The temporary backup archive was removed after successful verification; small before/after count records remain. No existing article states were swept or deleted and no budget was changed.
