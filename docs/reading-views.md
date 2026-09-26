# Reading views

Feed is the workspace's unread queue. Read later is the only bookmark list and
can include read articles. A subscription's article list contains its complete
collected history. There is no global All view, separate Unread or Saved section,
article deletion, or Trash. The `l` shortcut toggles Read later; `j` and `k` select
adjacent articles. Rules can mark articles read but cannot delete them.

The article API accepts `feed`, `later`, or `subscription`; the latter requires a
subscription ID. Both bootstrap and ordinary page requests default to `feed`.
Article updates accept only `read` and `later`. The unread counter updates when
reading, while the fetched page stays in place until another page/view request,
so rows never disappear under the pointer and the open article remains readable.

For the explicit state conversion, stop the app, back up `articles`,
`library_dedup`, and `rules`, then run `tools/simplify_article_state.sql`. It moves
Saved into Read later, retains trashed articles as read, removes obsolete flags,
and preserves article identities, content, and dates in one transaction. It
refuses to proceed if deletion rules exist and checks that row counts remain
unchanged. Run the new application only after conversion succeeds; the new model
rejects obsolete state fields rather than silently discarding user flags.
