# Zhihu account session

Implemented Profile → Zhihu with save, check, and confirmed removal. Both raw
Cookie headers and the supplied Chromium cookie table format are supported,
including comma-separated analytics values and quoted values. Input is preserved
verbatim in encrypted storage; only the HTTP cookie envelope is constructed.

Secrets use the existing account-bound AES-256-GCM cipher/master key. The API
returns only availability/configuration flags, requires authentication and CSRF
checks, and never returns cookies. Direct pinned outbound requests reject all
redirects; public proxies and browser transports never receive this session.

The native author-list adapter supports exact people/org profile URLs, with or
without `/posts`. Switching the collection method preserves source IDs, source
URLs, subscription documents, article origins and reading state. Existing poll
jobs dispatch using the newly configured source kind. Collection uses an explicit
recent-history window from `ingest.max_web_feed_pages`; it is not a full archive
import. Complete public bodies are requested with `include=data[*].content`; gated,
paid or truncated responses fail before publication. Stored bodies use the
existing sanitization pipeline without extra website requests. Shared sources with multiple account owners
fail explicitly instead of selecting another account's credential.

Schema release 3 adds only the encrypted `zhihu_sessions` table. A fresh production
backup was taken after draining the application, its restore catalog was read,
and all pre-existing table row counts were identical across the upgrade.

Final full `just check-release` passed: Rust formatting, Clippy and tests; real
PostgreSQL/schema-upgrade and backup/restore acceptance; Chromium outbound
security acceptance; 176 frontend tests; 53 browser scenarios; contract,
architecture and operational checks. The browser regression verifies stable
control geometry, immediate pending state and duplicate-click protection.

Production verification: the supplied session was saved through the authenticated
profile API and verified again by the native outbound client. Both existing exact
profile URLs were activated without changing their identity. Flink / Ververica and
DataFunTalk each delivered 200 articles and cleared `needsAttention`. Read-only API
checks confirmed nonempty sanitized full HTML and publication dates for both;
sample HTML sizes were 3,712 and 11,981 characters. The complete 10-page API windows
contained 200 unique public, non-truncated articles each with no contract violations.
The Profile UI confirmed session validity without returning its secret, and all
temporary verification sessions were revoked.

Final deployed UI verification also confirmed 22px profile gutters, a non-resizable
masked cookie field and consistent secondary buttons. Final API checks still
showed 200 articles for each source, full text available and no attention flag.
The final container passed its health check. Existing source URLs were not rewritten.
