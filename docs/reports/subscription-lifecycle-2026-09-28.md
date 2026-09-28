# Subscription archive and deletion

The user's final choice is to **retain articles when deleting a subscription**.
This supersedes the earlier answer requesting deletion of its articles.

Implemented:

- Archive, Restore and Delete actions in the subscription catalog and detail
  window, with bulk catalog actions and explicit confirmation.
- POST `/api/subscriptions/{id}/archive` is reversible. POST `/api/subscriptions/{id}/delete`
  removes only that subscription. Both require the authenticated owner and CSRF
  origin validation.
- Deleted subscriptions retain their exact source metadata in a separate
  provenance table. Existing articles, origins, content generations, publication
  data, reading state and AI records are not deleted or rewritten.
- Retained source labels remain readable but removed subscription IDs are no
  longer emitted as links. Already displayed article links are updated locally.
- Pending deletion locks immediately; confirmation controls retain their size;
  completed dialogs stay open until explicitly dismissed. Partial retries skip
  already completed items.

Database schema release 2 adds `removed_subscriptions` and
`article_subscription_provenance`. The explicit offline upgrade from release 1
adds these objects without rewriting any existing user rows. Startup remains
read-only and refuses a mismatched schema.

Regression coverage includes PostgreSQL article/content/state retention,
rejection of stale deletion, in-flight delivery, re-adding the same source URL,
release-1 upgrade with exact document retention, cross-user HTTP isolation,
frontend failure/retry, synchronous duplicate protection and Chromium control
geometry through pending and success states.

Verification: `just check-affected` passed. Full `just check-release` passed,
including Rust formatting/Clippy/tests, real PostgreSQL and backup/restore
acceptance, Chromium integration, 174 frontend unit tests and 52 browser tests.

The obsolete DELETE route is rejected (405). A stale browser that used DELETE
for reversible archiving cannot accidentally trigger permanent subscription removal.

Pre-deployment rehearsal restored a production backup into the separate
`reader_subscription_rehearsal` database. The candidate upgraded schema 1 to 2 and
passed its health command. Exact data-only dumps of all pre-existing tables,
excluding the schema journal, were both 12,063,491,619 bytes and `cmp -s` confirmed
byte-for-byte equality. No source content was printed during comparison.

Deployed to `158.160.186.87`. A fresh pre-upgrade backup was taken after stopping
and draining the application; row counts for every existing table matched after
upgrading the production schema. The new container is healthy on release 2.
Authenticated production checks passed for bootstrap, article and subscription
reads. Chromium verified catalog/detail actions and opened/cancelled the deletion
confirmation without submitting it. No user subscriptions were removed by the
rollout (`removed_subscriptions` remained empty). The temporary test session was
revoked. Both backup dumps and the previous image/configuration were retained.
