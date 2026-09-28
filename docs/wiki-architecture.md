# Private Wiki

Implementation follows [the approved specification](wiki-spec.md). Wiki and
Reader workspaces are independent authorization domains; Reader admins receive
no implicit Wiki access. No subscription pages are generated automatically.

## Boundaries

- `reader-wiki`: exact names, validated publication commands, roles, storage port.
- `reader-storage-postgres/wiki`: SQLx-instrumented transactions and nine tables.
- `reader-server/wiki`: authenticated, CSRF-checked routes; all responses no-store.
- `reader-server-contracts/wiki`: generated wire schemas and TypeScript contracts.
- `web/src/wiki`: full-page navigation, Markdown, editor, access, history, binding.

Namespace rows serialize writes and membership changes through `FOR UPDATE`.
Reads take `FOR SHARE` before evaluating owner/member access. All page identities,
aliases, revisions, drafts and reference projections include the namespace in
queries and foreign keys. The current revision has a deferred composite FK.
A revoked editor cannot save after the revocation transaction commits. Already
read client content cannot be retroactively recalled.

Publishing uses an expected revision UUID and a request operation UUID. The exact
request and result are retained transactionally. Repeating an identical operation
returns the same revision; reusing its UUID with different input conflicts.
Drafts have independent optimistic revisions, scoped by namespace and author.
An identical draft retry is idempotent; a stale different draft conflicts. Draft
removal also checks its revision, preserving a newer draft from another window.

Names are exact, case-sensitive text. Rename reserves all former names as aliases
to the same page ID. Trash reserves them too. Restoring a historical revision
creates a new revision; no API rewrites or permanently deletes history. A reader
cannot query a trashed page, its aliases or history. Links only resolve names in
the current namespace. Missing links never create pages by themselves.

Subscription bindings verify subscription ownership separately from Wiki read
access. Wiki endpoints never expose a reverse list of subscriptions. A missing
permission or trashed page returns a generic unavailable binding without erasing
it. Removing a subscription cascades only its binding, not Wiki content.

## Rendering and search

Markdown renders text nodes and a closed set of elements: headings, paragraphs,
lists, quotes, emphasis, fenced code, simple pipe tables and links. Raw HTML and
images remain literal. Internal links dispatch through the current namespace;
ordinary Markdown links to this application's Wiki routes are not activated.
There is no server fetching, AI submission or remote embedding of Wiki content.

Search is literal, case-insensitive PostgreSQL `ILIKE`, with SQL wildcard and
escape characters quoted. Both current title and body have pg_trgm GIN indexes;
namespace/trash/name/ID have a listing B-tree. GIN uses `fastupdate=off` so newly
published pages enter the main index immediately rather than accumulating
a pending list that makes selective queries choose a full scan. This read-heavy
feature pays index maintenance on publication. Short search terms can use a scoped
scan because trigram indexes cannot narrow fewer than three characters. All
list/history/member/namespace responses use the configured page size plus one
lookahead, never load the whole collection into the browser.

## Configuration and upgrade

The required `wiki` configuration exposes UTF-8 name/body/search byte limits,
page size and draft autosave delay. Name capacity is validated below PostgreSQL's
B-tree tuple limit; timer delay fits the browser timer range. Example defaults:
512 name bytes, 512 KiB Markdown, 512 search bytes, 200 Unicode characters per
search excerpt, 50 rows, 1000ms autosave. Search snippets explicitly show a prefix
with an ellipsis when more text exists; the source is not modified.
Source Markdown is stored exactly; rendering does not rewrite it.

Schema release 5 adds nine tables and pg_trgm indexes to release 4, without
rewriting existing Reader data. Stop the application, take and verify a full
PostgreSQL backup, run `upgrade-schema`, then start the new image. Startup checks
schema release identity. Full backup/restore acceptance compares every table,
including nonempty Wiki pages, history, private drafts, operations and bindings.
No periodic history pruning or permanent deletion is configured.

## Verification

Domain tests reject invalid command construction. PostgreSQL acceptance exercises
ACL roles including an unrelated administrator, foreign namespace IDs, exact
aliases, literal multilingual search, stale revisions, draft privacy, trash,
bindings, idempotency and a save blocked behind membership revocation. Browser
acceptance uses the real Rust API and PostgreSQL, tests immediate publication
feedback, duplicate activation and stable Save/Cancel geometry, restores a draft
after reload, and checks CSRF and missing-page creation semantics.

History lists contain revision metadata only; selected revision bodies load on demand.
A selective search over 5,000 test pages used the GIN index and took 1.534ms
in the local PostgreSQL acceptance fixture (environment-specific measurement).
The final release gate passed: Rust fmt/Clippy/tests, generated contracts,
real PostgreSQL/browser and backup/restore acceptance, Chromium acceptance,
180 frontend tests, 53 browser tests, architecture and operational checks.
