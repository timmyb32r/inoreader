# Telegram and feeds: shared HTTPS proxy pool

Implemented and deployed on 2026-09-28 to the authorized server.

- Replaced per-route endpoint lists with `http.proxy_pool` and disjoint
  `http.proxy_routes`; removed the old configuration contract.
- Retained all eight existing endpoints. Initially prefer the two endpoints that
  answered Telegram HTTPS probes. YouTube retains direct-first routing.
- Feed/browser/icon clients and glossary API/history clone one transport. Health
  remains scoped to destination host + endpoint; clones share quarantine/recovery.
- Telegram's explicit capability accepts only the five replay-safe Bot API POST
  operations. Pinned public destination IP, verified origin certificate/hostname,
  HTTPS-only URLs, origin-restricted redirects and credential-free CONNECT remain
  mandatory. Public mode still rejects bodies and sensitive/custom headers.
- Shared full attempt budget is 35 seconds, quarantine 60 seconds. Telegram
  CONNECT/TLS deadline is 3 seconds; long-poll timeout remains 25 seconds and
  overall request deadline 45 seconds. Startup validates compatible budgets.
- Updated AGENTS.md to record the user's explicit authorization for Bot API
  secrets inside verified end-to-end HTTPS through the public pool.

Verification: all 65 web-runtime tests passed, including wire-level secret
placement, certificate/hostname rejection and shared retry health. Complete
`just check-release` passed, including PostgreSQL/backup/browser and Chromium
Docker acceptance. Initial Chromium execution ran out of Docker disk; two
unreferenced dangling images were removed (no volumes or containers pruned),
then the entire gate passed. `git diff --check` passed.

Production: new app is healthy; protected previous config and image ID retained
under `.inoreader-state/telegram-pool/`. No schema migration was necessary.
A deliberately invalid token exercised the application's proxy route: the first
endpoint timed out after 3 seconds, the next returned an authoritative response;
no real bot secret was used or stored by that check. Public history requests now
succeed in roughly 1–5 seconds, and the earlier history error cleared. History
catch-up was still pending at the final status check, with zero conflicts and
unindexed observations. The bot is not yet configured: the user must resubmit
its token using the existing Connect bot control. Test owner sessions were revoked.
