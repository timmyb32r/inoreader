# Wiki organization

Each namespace has an independent page hierarchy. A page has at most one parent;
parents must belong to the same namespace. Self-links and cycles are rejected
before commit. Namespace write locks serialize simultaneous moves, including
competing A→B/B→A changes. Parent edits use the page's revision and operation ID,
create a history revision, preserve Markdown and titles, and can be restored
through history. Restoring a historical parent also checks cycles and availability.

Trash retains relationships and favorites. Deleted pages are omitted from child
navigation and collections; existing edges remain, so moving a parent to trash
never silently promotes its children. Selecting a new deleted parent is rejected.
Standalone pages have no parent and no children, including retained trash edges.

Favorites belong to the signed-in account, within the selected namespace. Readers
may star pages they can read. Favorites never grant access; every read and write
checks current membership. Revocation immediately hides the namespace and its
collections. All collections and child lists use configured pagination limits.

The Subscriptions parent is an ordinary wiki page with a stable namespace-scoped
registration. Creation is explicit and idempotent. A conflicting existing name
fails instead of adopting or renaming someone else's page. New pages created from
an article's W+ action become its children before linking to their subscription.
An existing page linked manually keeps its deliberately chosen hierarchy.

## Upgrade

Schema release 10 adds nullable parent columns to pages/revisions and tables for
favorites and subscription roots. Back up, stop writers, run the explicit
`upgrade-schema` command (release 9 is supported), or run
`tools/upgrade_wiki_organization.sql` with psql. Both consume the same organization
DDL. Start the new app, create the registered Subscriptions parent through the
API, and assign each existing subscription-bound page with revision-checked
`set_parent` commands. No Markdown, titles, page IDs, aliases or old revisions are
rewritten.

UI: one global search entry in the reader sidebar; Ctrl/⌘ K also opens it. The
search field receives focus only when its page opens, not after result updates.
Within a wiki namespace, Favorites and Standalone pages have distinct URLs.
The page organization section reserves its dimensions before asynchronous data
arrives; stars use pressed/pending feedback and duplicate-activation protection.
