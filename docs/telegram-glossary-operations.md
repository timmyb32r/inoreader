# Telegram glossary operations

`reader-glossary` owns validated Telegram identities, UTF-16 formatting, exact
paragraph parsing, polling and public-history retrieval. `reader-ai/definitions`
freezes the complete article, prompt version, Flash model, limits and tariffs.
PostgreSQL owns durable observations, derived definitions and fenced paid jobs.
The server authenticates every route; the browser never receives bot/API keys.

The original 1,529-post corpus is read-only. Import is native and bounded by
`glossary.max_archive_line_bytes`; oversize or invalid records stop import.
No excerpt or truncated body replaces missing full article text.

```sh
inoreader --config config.yaml import-telegram-archive /path/to/corpus OWNER_UUID WORKSPACE_UUID
inoreader --config config.yaml reindex-telegram-glossary OWNER_UUID WORKSPACE_UUID
```

Reindex rebuilds each current post from its retained exact source observation,
then queues failed non-conflicting events. Ambiguous duplicate payloads remain
quarantined. Old edits never replace newer observations. Original raw receipts
are never deleted by reindex. Post text, nullable timestamps, source IDs and
unknown source fields remain retained. The inventory describes 13 unobserved
message IDs; absence is not interpreted as deletion.

To enable fresh Bot API delivery, create a **dedicated** bot with BotFather,
add it to `@reading_data_news`, and enter its token under Settings → Словарь.
The bot needs channel membership, not publishing permission. Do not share the
token in chat or reuse a bot consumed by another service. An existing webhook
is rejected; it is never deleted automatically. Settings shows poll/history
errors, unindexed records and conflicts. Tokens use the existing server AES-GCM
master key and are never returned by GET routes or printed in request logs.

Raw Bot API response and cursor commit in one PostgreSQL transaction. The next
poll acknowledges only that committed offset. A separate projector handles
supported events. Unknown formatting stays explicitly unindexed and replayable.
Public `t.me/s` history and Bot API use the shared configured HTTPS CONNECT pool
when their exact hosts appear in `http.proxy_routes`. Bot credentials travel only
inside verified origin TLS; see [proxy operations](operations.md#shared-https-proxy-pool). It attempts
archive-to-bot catch-up and recovery after outages, but cannot prove completeness
or recover deletions; the coverage note remains visible. Bot API retention is
24 hours, and long downtime can leave irrecoverable gaps. See the official
[Telegram delivery contract](https://core.telegram.org/bots/api#getting-updates).

Paid definitions use one non-thinking Flash request. Exact input equality
prevents duplicate jobs; operation IDs cannot be reused for a different input
or regeneration mode. Explicit refresh creates a new attempt after completion.
Lease expiry fails the attempt without automatic re-billing; raw provider bytes
are retained even if publication loses its lease. Usage absent from a response
means unknown, never zero. Known/new classification is a fresh exact-name lookup
on every read and requires no additional paid request.

New tables are included by ordinary full-database `pg_dump`. The real PostgreSQL
acceptance test dumps and restores all seven glossary/job tables and compares
every field. Keep a separate **server-side** master-key backup; losing that key
makes persisted API/bot credentials unusable. Do not regenerate it on deployment.

Automated tests establish parsing, delivery, job and UI contracts, not complete
LLM extraction or factual accuracy. No new blind-review score is claimed.

Definition projections retain `parser_version`; reindex rebuilds only the current
observation and preserves unresolved conflicts. Name grounding compares the exact
name against the frozen article/title. It does not ask the model to reproduce a
longer quotation: live Flash verification showed whitespace drift in quotations
for otherwise valid SQL/OLAP names. No normalization or silent correction is used.

Deployment on 2026-09-27 imported 1,529 posts, 836 definition paragraphs and 808
exact distinct names; all 1,761 archive receipts were unchanged after repeat import.
Six absent dates remain absent. Telegram public/API TCP connections from the VM
timed out during deployment; this is reported as a delivery error, not a successful
sync. The bot has not been created. Live delivery therefore remains unverified
until a dedicated bot and a working network route are provided.

Final production Flash verification returned 36 entities (one known definition)
in 9.99 seconds; cached reopen reused the same job. The final release gate includes
133 frontend unit tests and 37 browser tests in addition to Rust and real Docker
integration checks. This verifies the workflow, not exhaustive model recall.
