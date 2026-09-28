# Zhihu session collection

Profile → Zhihu accepts either the exact Cookie header value (without `Cookie:`)
or a complete 12-column, tab-separated Chromium DevTools cookie table. `z_c0`
with root path is required. Values are not decoded, trimmed or normalized. Table
cookies retain their domain/path restrictions; foreign domains and malformed or
ambiguous rows are rejected before networking. The entire input is encrypted
verbatim using the existing account-bound AES-256-GCM credential cipher and the
configured AI encryption key. The key must be backed up separately from PostgreSQL.

Save checks `/api/v4/me` first, then atomically replaces the encrypted session and
switches the account's existing exact `https://www.zhihu.com/people/{slug}`
and `/org/{slug}` sources (with or without `/posts`) to the Zhihu adapter. Source IDs, URLs, stored
records, origins, articles and read states are retained. Save queues refreshes.
For a newly added Zhihu subscription, save the session again to connect it.
Sources shared by multiple accounts fail explicitly: no arbitrary account's
session is chosen. The adapter requests complete public bodies with `include=data[*].content`.
It requires non-truncated, non-login-gated content and an empty paid-info object;
otherwise the whole poll fails explicitly before publishing records. Private/paid
content is not ingested into the shared source cache. Stored bodies go through the
existing sanitization/chunk publication pipeline without another website fetch.

The adapter requests 20 articles per API page, up to the explicit
`ingest.max_web_feed_pages` recent-history window. This is not a complete archive
import. It constructs API pagination itself and never follows provider-supplied
paging URLs. String upstream IDs are preserved byte-for-byte, without numeric conversion; their
canonical publisher links are `https://zhuanlan.zhihu.com/p/{id}`. Publisher Unix
seconds become equivalent UTC RFC3339 publication dates; save dates remain separate.
Duplicate IDs, missing fields, invalid dates, and malformed paging fail the poll.

All credential-bearing requests use the shared outbound boundary with a direct
pinned transport, existing response/time budgets, and redirects explicitly
 disabled (zero hops). No browser, public proxy, external host, logging output or
API response receives cookies. SQL timing logs omit bind values. API mutations
require the reader session and same-origin CSRF validation. Status returns only
`available` and `configured`. Check does not replace the secret. Remove deletes
only this account's credential; articles and source configuration remain intact.
Expired/rejected sessions surface in subscription health and Profile → Check.

Schema release 3 adds only `zhihu_sessions`; upgrade release 2 offline with the
existing `upgrade-schema` command after a verified backup. It does not rewrite
existing user tables. Startup remains read-only and verifies the release.
