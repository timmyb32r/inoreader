# Subscription recovery implementation — 2026-09-28

The approved recovery plan is implemented with explicit source contracts, owner-scoped configuration changes and live verification. No article rows or subscription identities were deleted; historical job records were retained. Errors requiring external access remain visible.

## Implemented

- Public HTTP CONNECT routing validates public destinations and TLS independently of the proxy. Finite per-host endpoint pools use an explicit attempt deadline and temporary host/endpoint quarantine. Failed transport attempts may advance to another endpoint; origin HTTP errors and partially received bodies are never silently replayed through another proxy.
- HTTP 429 retries respect the later of `Retry-After` and a configured minimum delay, with bounded per-job jitter. Exhausting one retry cycle preserves that cooldown. The two Reddit schedules were separated by at least five minutes.
- DBConvert and Databricks now use verified publisher RSS. Alibaba Tech uses its verified current Medium RSS; the former URL returns a genuine404. Qlik retains its original feed and identities.
- Dropbox explicitly reads its publisher's complete409-entry static archive. Existing RSS identities, precise publication dates, descriptions and fullHTML are retained when the listing contains less information. Contradictory dates fail visibly.
- Telegram has a native public-preview adapter. Original permalink identity, timestamps, captions, media and absent titles are preserved. Full-text extraction uses the stored post body instead of the Telegram landing page. Lists label a missing title separately without fabricating source text.
- Missing recurring collection jobs were restored with owner, source-kind, revision and lease checks. Future retry deadlines and source-health errors were retained.
- Repeated manual refresh requests no longer reuse a completed job forever; pending requests are deduplicated, including persisted shared-source associations.

## Production evidence

- Dropbox: all409 records delivered to the library;10 prior record identities retained.
- CDO:14 posts; photo-only post3078 returned through the article API with empty authored title, visible image and ready fulltext.
- SQL Ninja: text post163 has its actual caption/body, media-only post144 remains accessible; all19 pre-existing posts preserve read/later/protect-unread state by source identity.
- Qlik20, Databricks10, Alibaba10, Maxime6, Apache Flink Medium10 and Devart10 records successfully collected. Greptime's Chinese feed is complete at358 records.
- The18-source snapshot comparison reported zero missing record identities or reading-state differences.

Final production snapshot at2026-09-28 10:40UTC: **449subscriptions retained; active Needs attention18→7**. All18target sources have one active recurring collection job. Eleven original problem sources collected successfully. Additional transient failures in StreamNative, HighGo and SupermetricsYouTube also recovered. DBConvert delivered15articles and RedditApacheFlink25, verified through the owner API after fanout. The temporary verification session was revoked after the final check.

## External limitations

Zhihu's two exact authored-post endpoints return403/40353 even in fresh Chromium with guestcookies and the current RSSHub request signature. No account session was used. Oracle's two feeds and Precisely still return access-denied/challenge responses on tested routes. Lauren's old Medium URL returns404; a different empty profile is not substituted. Reddit's original feeds are retained and rate-limit cooldowns respected. ApacheFlink recovered with25records; DataEngineering still returns403/429. Its supported alternative is approved RedditOAuth access, not an unregistered anonymous retry loop.

Free public proxies are transient: a successful response is evidence of a working route at that time, not an availability guarantee. Diagnostics used verified TLS, public IP destinations and no user credentials. AI and Telegram Bot credentials never use these routes.

## Verification

The final complete `just check-release` passed, including real PostgreSQL with manual-refresh concurrency and ownership regressions, backup/restore and Chromium acceptance,22configuration tests,172frontend unit tests and51browser scenarios. Strict proxy-route parsing rejects null/coercible nonlists and overlapping host groups.

Detailed operational evidence and protected before/after snapshots are under `.inoreader-state/attention-next/` (not a deployable data fixture).

## Remaining seven — explicit next steps

| Source | Current evidence | Next feasible step |
| --- | --- | --- |
| Flink / Zhihu | Guest Chromium and signed articles API403/40353 | Authorized account-session pilot on trusted transport; no guarantee of access |
| DataFunTalk / Zhihu | Same guest-access refusal | Same authorized-session pilot |
| Oracle Data Integration | Publisher403 through tested direct/browser/proxy routes | Test a stable trusted alternative egress; validate actual RSS before replacing configuration |
| Oracle Cloud Infrastructure | Publisher403 through tested routes | Same egress pilot, original feed retained |
| Precisely | Challenge/access-denied page, browser navigation failure | Trusted browser/egress pilot; retain original source until actual articles are verified |
| Lauren Balik / Medium | Original feed404; alternate profile has zeroentries | Obtain the author's current valid feed; do not silently substitute a different publication |
| Reddit Data Engineering | Original RSS403/429; same response through tested alternate routes | Respect cooldowns; apply for supported Reddit API access if ongoing anonymous access remains blocked |

DBConvert has its own finite pool (193.37.71.46,37.114.41.103); YouTube prefers37.114.41.103; Reddit uses45.147.179.118 then37.114.41.103. Other explicitly routed public hosts use193.37.71.46 then45.147.179.118. Exact ports and validated limits remain in the server configuration and the protected operational snapshot. No automatic public-proxy discovery is enabled in production.
