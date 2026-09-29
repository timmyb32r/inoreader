# Reading history in Feed

Home's read-today card, Read bars and calendar days open the existing Feed with
an explicit read-event interval. The reader URL retains `read_from` and
`read_until` across reload and paging. These are UTC RFC3339 instants calculated
from the selected browser-local calendar day; adding a calendar day rather than
24 hours preserves DST boundaries. The date selector uses the same timezone.

The backend validates that both bounds exist, start precedes end, and precision
fits PostgreSQL microseconds without rounding. The filter is supported only by
workspace Feed, without a subscription or Read later scope. Auth ownership checks
are unchanged. The validated `ReadPeriod` has private fields and a fallible
constructor; applying it to another scope is rejected.

Selection uses persisted `article_read_events` within [start, end). One article
appears once per period, ordered by its latest matching read event, then article
id; cursor pagination uses this same tuple in both directions. Currently unread articles are excluded from both the list and the dashboard.
Marking an article unread leaves the visible batch in place until refresh, then
removes it from the selection. Raw read events are preserved. The read timestamp is a presentation field and
never replaces publication or arrival timestamps. Removed articles are excluded from the visible list and counts; their raw
events are still retained.

Clearing the filter returns the usual unread Feed. Once opened, its filter area
keeps its height while clearing/loading to preserve pointer targets. Bulk marking
is disabled in historical views so it cannot accidentally act on the whole Feed.
An empty interval has an explicit history empty state. Loading disables initiating
controls immediately and uses existing fixed status regions for errors. Cards,
Read bars and calendar days support keyboard activation; Arrived bars retain
informational semantics.

No schema change, backfill, or mutation of existing read history is required.
Tests cover boundaries, repeat events, unread-after-read, workspace isolation,
bidirectional pagination, invalid construction/wire input, DST, URL persistence,
interaction locking and layout stability.
