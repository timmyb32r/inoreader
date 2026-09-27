# Second architecture audit: execution contracts

## Reader consistency

The account controller owns in-flight writes independently of the displayed page
and workspace. Navigation publishes pending feedback synchronously, waits for the
write barrier, then fetches a snapshot. A write beginning during the fetch makes
the snapshot obsolete: it is fetched again after writes settle. Page application
never drops outstanding writes. Workspace replacement still fences late UI updates;
the account barrier retains the underlying operation until completion. This adds
latency to navigation overlapping a write in exchange for correct snapshots and
counts. Idle navigation adds no API requests. No rows are removed under the pointer.

`ArticleListPanel` owns list rendering, source refresh pending state and duplicate
activation protection. `ReaderApplication` composes account-owned controllers and
feature panes. Source refresh is a local pane interaction, not global reader state.

## Durable AI responses

Non-streaming translation/definitions capture immutable response bytes before
parsing or accounting. Capture checks account/workspace ownership, but does not
require an unexpired publication lease. Exact repeated capture is idempotent;
different bytes for the same attempt fail closed. Finishing changes only the
published state and is still lease-fenced. A failed or expired attempt never
silently repeats a paid request; an explicit retry receives a new identity.
Existing chat call/draft accounting remains separate from publication.

A crash between receiving external bytes and committing them to PostgreSQL is
still possible: there is no distributed transaction with the provider. A storage
failure is surfaced and the uncertain attempt is not automatically regenerated.
Capture adds one short database write per non-streaming provider response.

## Article operations

`ArticleScope` makes subscription identity mandatory only for subscription
history. `SelectionLimit` validates positivity and PostgreSQL lookahead capacity.
`ArticlePageRequest` has private fields and a fallible constructor; newer traversal
requires a cursor. Operational requests have no unchecked deserialization path.
HTTP DTOs are converted at the boundary; the configured bulk limit is validated
before server construction.

Bulk mark-read belongs to the application layer, after `OwnedWorkspace` resolution.
PostgreSQL selects unread matching records in deterministic arrival/key order (using the unread read index), with at most
configured limit + one lookahead row. Over-limit selection makes no writes. Revision
conflicts roll back the whole transaction; arrivals after the selection belong to
the next operation. No chunked partial-success behavior was introduced.

## Wire responses

Every API client method explicitly passes a response contract to transport; there
is no URL-regex dispatch or implicit unvalidated response. Empty and scalar replies
have contracts too. Schemas remain generated from Rust. Generation rejects schema
keywords outside the runtime decoder's closed supported vocabulary. JSON Schema
2020-12 `format` is an annotation; domain identity/date validation remains in Rust.
Unknown fields are retained unless the owning schema explicitly forbids them.

## Verification and performance

Real browser acceptance enables AI through the production HTTP routes, supervisor,
workers, store and PostgreSQL. Only the external provider is deterministic. It
covers pending feedback, duplicate clicks, fixed panel geometry, invalid JSON,
explicit retry and successful result persistence across reload. Separate storage
tests cover expired leases, immutable reply capture and cross-account access.

`latency-budgets.json` owns the local fixture budget: three warmups, twenty timed
HTTP samples per bootstrap/list/subscriptions/article route, p95 below 500 ms.
The reader startup also limits article fetch count and page row count. These are
regression budgets for the hermetic fixture, not production SLOs or runtime limits.
Playwright attaches the exact samples as `reader-latency.json`.

All PostgreSQL connection acquisitions now emit SQLx's stable acquire timing,
separate from statement completion duration. Existing AI queue/stage and matched
API route logs share request context. For production JSON logs:

```
python3 tools/latency_report.py application.jsonl
python3 tools/latency_report.py application.jsonl --budgets approved-budgets.json
```

The streaming report computes bounded-memory distributions for SQL statements,
connection acquisition, API routes, AI stages and durable queue wait. It outputs
counts, histogram buckets and p50/p95/p99 bucket upper bounds (powers of two
microseconds), not falsely precise percentiles. It never outputs query text,
response bodies or account/job identifiers. Missing budget metrics fail the check.
The report retains only histogram counts, not all samples. Logs must cover the
same workload/window for comparisons. SQLx emits successful acquisitions; timeouts
remain explicit request errors, not fabricated successful wait samples.

## Verification evidence

Pending final release run and deployment; see the authoritative task ledger.
