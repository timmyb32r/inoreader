# Unified search

`/search` is the single news/wiki search surface: header, sidebar and Cmd/Ctrl+K
open it. Wiki's page-search entry supplies `kind=wiki&namespace=<id>` to the same
page. Subscription catalog filtering and the deleted-page filter remain local,
because they search management records rather than published reading material.

Exact query, content kind, optional scope, offset and selected preview live in
the URL. Results occupy a fixed left pane; preview uses existing authorized
article/wiki APIs in the right pane. No article is marked read by previewing it.
Loading, errors and pagination retain their footprint. Late requests cannot
replace newer results or previews. Query/excerpt highlighting renders text nodes.

## Search contract

The authenticated account is taken only from the server session. News searches
all workspaces owned by that account (including retained articles from removed
subscriptions). Wiki searches current non-deleted pages only, in namespaces the
account owns or belongs to; administrator privileges do not grant access. Drafts
and historical revisions are excluded. ACL reads lock namespaces against member
revocation and recheck access in the search statement. Explicit inaccessible
scopes return 404. Search and limits responses are `Cache-Control: no-store`.

Queries are literal case-insensitive substrings; `%`, `_`, and backslash are
escaped for SQL, not interpreted as patterns. Chinese, Russian and English need
no tokenizer or stemming. Exact title matches precede other title matches and
body matches. Results are paginated without exposing inaccessible counts.
The selected page is materialized before loading full-body excerpts, avoiding
repeated content reads for matches outside the returned page.

`search.query_bytes`, `search.page_size` and `search.excerpt_characters` are
required, positive validated configuration. Example: 512 bytes, 25 hits, 220
Unicode characters. Oversized queries fail explicitly; no truncation or silent
query rewriting. Excerpts are explicitly abbreviated with ellipses; stored
source documents and HTML bytes remain untouched.

The application owns validated requests, read DTOs and `SearchPort`. Server
contracts expose those DTOs for generated runtime-validated TypeScript clients;
this reviewed dependency contains no storage/transport implementations.
PostgreSQL owns authorization, matching, ordering, pagination and indices.

## Projection and release 6

`search_content` stores visible text extracted from the published safe HTML by
the same native extractor used for article AI. `record_id` references the content
manifest with cascade deletion. Content publication updates both atomically;
invalid UTF-8 or noncontiguous chunks roll back the publication. Trigram GIN
indices cover article titles/descriptions, safe text and wiki name/Markdown.
A source-record index maps matches to library articles without scanning HTML.

Before upgrading release 5: add the search configuration, stop the application,
retain config/image, take a PostgreSQL backup and verify it can be restored.
`upgrade-schema` creates the projection and backfills in configured keyset
batches, one article body at a time, inside one transaction. Indices are built
once after the backfill rather than maintained for each initial insertion. It verifies exact
manifest identity, chunk count/order and UTF-8; any failure rolls back. Existing
source rows are never rewritten. Startup verifies release identity. Compare all
preexisting table counts before restarting the new image.

Tests cover literal matching, CJK, account isolation, admin exclusion, membership
revocation, trash exclusion, pagination, projection replacement/rollback, real
HTTP/wiki search, backup restore, stale UI responses, double submission, fixed
control coordinates and browser Back.
