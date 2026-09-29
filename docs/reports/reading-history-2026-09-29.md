# Reading-history Feed deployment

Implemented selected design 1 and deployed to 158.160.186.87 on 2026-09-29.
Image: `sha256:b1e448c45345b83643b9a9043de254a05954c6575f47777d672484e26b5085f1`.
No database schema change or historic data mutation.

Home's today card, Read bars and calendar cells open filtered Feed. The date,
clear action and Back to Home stay in a reserved region. Cursor paging and
reload retain the selected interval. The rows include the matching read time;
the list and pager retain their geometry when the filter is cleared.

Verification: `just check-affected`, `just check-release` passed. The release gate
includes real PostgreSQL history, isolation and pagination tests, backup/restore,
Chromium acceptance, 189 frontend unit tests and 66 browser tests. New browser
coverage includes duplicate activation, pending feedback, DST boundaries, reload,
read-time width, and different list lengths before/after clearing.

Live authenticated API returned exactly the two articles counted on Home for
September 29, Moscow time. Invalid/contradictory periods returned 422. Live browser
confirmed the card transition, persisted date on reload, and unchanged list
geometry on clear. The smoke used no article state mutations or paid AI requests;
its temporary authentication session was removed.

See [selection contract](../reading-history.md).
