# Subscription contribution analysis

Home's arrivals card opens `/source-activity?day=YYYY-MM-DD` with that calendar
 day selected. The default is today, in the same browser IANA timezone as Home.
The From / Through fields select an arbitrary inclusive calendar range; Apply
validates both dates and their order. The authenticated backend repeats validation
and resolves workspace ownership before returning any statistics.

The treemap uses one color. Rectangle area is proportional to the number of
articles attributed to a source, not unread state or publication date. The ranking
lists every source and its exact count/share, including rectangles too small for
an in-place label. Hover/focus details occupy a reserved region; clicking a tile
focuses its ranking row. A current source's name opens its subscription details.

One article contributes once to every distinct subscription linked to it. Multiple
source records from the same subscription do not multiply the contribution.
Percentages use total source contributions; the separate unique-article total
matches Home's arrivals for the same day. Removed subscriptions retain historical
attribution. Articles lacking provenance use the explicit Unattributed category.

PostgreSQL aggregates first-arrival timestamps in one snapshot. Fractional seconds
are floored only for calendar grouping; stored timestamps remain untouched. The
range uses local calendar boundaries, including DST. The treemap does not hide
September 26–28: that explicit display exclusion applies only to the Home bar chart.

The database needs no new table or schema migration. The endpoint is
`GET /api/workspaces/{id}/source-activity?timezone=Europe/Moscow&from=2026-09-29&until=2026-09-29`.
Responses are private and use Cache-Control: no-store.
