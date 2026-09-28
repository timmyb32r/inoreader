# Source health and retry policy

`subscriptions.attention_after_seconds` is required, positive and defaults in the
example configuration to 86400 (24 hours). A source needs attention only when its
latest failed attempt is at least this long after the first failure in the current
unbroken streak. Different error messages do not restart the clock. Successful
collection, including HTTP 304 or a committed partial page with queued continuation,
clears the streak. Merely waiting while collection is stopped does not create a
new failure observation. Errors and attempt history remain available immediately.

The PostgreSQL `source_health.failure_since_ms` projection persists across restarts.
It is written transactionally with failed-attempt diagnostics and cleared with a
successful collection. The server owns classification; frontend indicators,
filters and sorting use `needsAttention` exclusively. A queued continuation alone
is not an incident. Changing the threshold changes classification without deleting
history or touching articles.

Schema release 4 adds the nullable projection. The explicit offline upgrade from
release 3 recovers the earliest retained failed attempt after the last success;
when none survives, the latest recorded failure is the earliest provable start.
Existing health JSON and article documents are not rewritten. Back up and stop
writers before `upgrade-schema`. Release 2 must first be upgraded to release 3.

Retries remain bounded by `scheduler.retry_attempts`, exponential delays and
configured jitter. Recurring sources resume after `polling_interval_seconds`, even
when one cycle fails. HTTP 429 respects the configured floor and server Retry-After.
Increasing the burst of retries does not repair an authoritative 403.

## Public proxy routing

Each route explicitly declares `direct_first`. With `true`, the shared pinned
transport tries direct HTTPS first, within the configured endpoint attempt budget.
Only transport failure/timeout enters the proxy pool. HTTP responses (including
403, 429 and 503) are authoritative, not a reason to rotate exit addresses. Direct
and proxy attempts share the overall deadline. A body failure is reported and
retried by the normal job scheduler, not spliced with another response.

Proxy addresses remain in configuration after failures. Health is per host and
endpoint; a failed pair is eligible for a recovery probe after `quarantine_ms`.
Eligible probes are ordered by oldest attempt so endpoints at the end of a pool
cannot starve across short-deadline requests. At most one recovery probe per pair
runs concurrently. No claim is made that every endpoint fits into one request.
The runtime does not discover arbitrary new proxies automatically.

All proxy traffic is unauthenticated public HTTPS; cookies, authorization and
request bodies are rejected. Zhihu, DeepSeek and Telegram Bot credentials keep
separate direct transports. TLS and per-hop destination validation remain enabled.
