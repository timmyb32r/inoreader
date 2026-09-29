# Reading activity

Home shows distinct currently read articles explicitly marked read per calendar day
in the selected workspace. Today and the last 364 days replace the former browser-local elapsed-time
metric. The existing local time data is left untouched, but is never interpreted as
article counts. Historical dates cannot be recovered from the current read flag.

The repository records only an existing unread -> read transition, atomically with
the article revision. Creation/import, automatic rules, failed writes, repeated true
patches, and changes to Read later do not add events. Bulk marking includes every
successfully updated article and rolls back its events if any row conflicts.
Unmarking an article preserves raw history but removes it from the visible read
counts and filtered Feed on the next fetch. Both views join existing articles and
require their current read flag. Rereading on another day counts that day;
multiple transitions for the same article on the same day count once.

Events retain workspace/article identity, revision and database-generated timestamp.
They are not cascade-deleted when article/source data is removed. A primary key
rejects duplicate transition writes. Backup/restore includes the event table.

`GET /api/workspaces/{id}/reading-activity?timezone=Europe/Moscow` requires workspace
ownership. PostgreSQL validates the supplied timezone before aggregation. Calendar
days use that zone, including historical daylight-saving rules. Unsupported zones
are rejected; no UTC fallback silently changes the user's day. The 364-day window is
a presentation range, not a retention limit. All events remain stored.

Home fetches on entry, focus/visibility return and once a minute while visible.
Overlapping requests are suppressed; stale responses are ignored after unmount.
The grid, summary and reserved status region retain their geometry during loading
and errors. Failure never appears as a successful zero count.

## Arrived and read

The same authenticated response includes `arrived` for each day, counting workspace
articles by their immutable `first_arrived_at`, not publication time or source refresh
attempts. Each logical article counts once, even with several subscription origins.
Read events and arrivals use the same timezone and 364-day window in one SQL snapshot.
Arrival history can be reconstructed from existing articles; reading history cannot.
Deleting or archiving a subscription retains its articles and their arrival counts.

Home adds a last-30-days grouped bar chart on one linear integer scale: slate for
arrivals and teal for reads, with immediate hover/keyboard-focus tooltips. Both
zero-count days and pending states preserve the chart dimensions. The two summary
cards show today's arrived and read counts; the existing reading calendar remains.

Aggregation uses the exact indexed arrival-order range. Fractional seconds are
excluded only from the derived calendar-date expression, before PostgreSQL can
round nanoseconds across midnight. Original timestamp values remain unchanged.

The Home Arrived & read chart explicitly hides both series for 2026-09-26,
2026-09-27 and 2026-09-28 at the owner’s request. These hardcoded dates retain
empty calendar slots and do not contribute to the Y-axis scale. Stored activity,
API values, daily cards and the reading calendar remain unchanged.
