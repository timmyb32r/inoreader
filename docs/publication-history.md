# Subscription publication history

The Overview tab includes a publication histogram and a twelve-month calendar.
Days shows the selected month, Months the selected year, and Years the complete
collected history. Selecting a calendar month or date opens its daily histogram;
selecting a yearly bar changes the calendar year. Hover and keyboard focus expose
exact counts. Chart controls and calendar rows retain their layout while loading,
retrying, or changing scale.

`GET /api/subscriptions/{id}/publication-history` requires an authenticated
subscription owner. PostgreSQL joins only delivered origins for that subscription
and workspace to their source records, using the existing `by_subscription` and
source-record primary-key indexes. The response contains sparse UTC day/count
buckets, `undated`, and `conflicting`; it never downloads article bodies. Counts
include collected read, unread, and trashed articles, irrespective of the current
reader page. No publications are inferred before collection began.

Each logical article contributes once. Matching origins on the same UTC day count
once; origins on different days are reported as conflicting rather than selecting
a date. Articles with no source publication date are reported as undated. Neither
category is placed on the chart using an ingestion timestamp. Invalid persisted
timestamps fail the query instead of becoming missing dates. UTC day bucketing is
display-only aggregation; stored publication timestamps remain unchanged.

The PostgreSQL implementation supports this endpoint. Other repositories return
an explicit unavailable error after checking ownership until implemented. The UI
reserves the calendar area and displays a retryable error without moving controls.
