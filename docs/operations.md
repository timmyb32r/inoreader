# Operations

## Production topology

The release artifact is one Linux `inoreader` binary containing the compiled
Preact application. Runtime dependencies are the PostgreSQL and Chromium
containers in `compose.yaml`. PostgreSQL uses a persistent volume and is reachable
only through the internal database network. The app joins the database,
public-egress, and internal browser-control networks. Chromium joins only browser-control, which is
declared `internal: true`, so it has no direct public or host-network route. The
CDP port is available only on that network. Browser HTTP(S) is fulfilled through
the app's shared outbound policy; a context-level black-hole proxy is a second
fail-closed barrier. Neither container receives the Docker socket or a host
directory containing browser state.

Copy `config.example.yaml` to `config.yaml`, set the public origin, and create a
random single-line PostgreSQL password in `secrets/postgres-password` with mode
`0600`. Keep configuration and secrets outside version
control. `check-config` validates the PostgreSQL password-file reference without
connecting to the database:

```sh
docker compose run --rm app --config /etc/inoreader/config.yaml check-config
docker compose run --rm app --config /etc/inoreader/config.yaml prepare-schema
docker compose up -d
```

## Explicit schema upgrades

Startup is read-only with respect to schema. A missing, incompatible or physically
drifted critical schema contract fails before listeners/workers start. The release
journal is `schema_releases`; `prepare-schema` never upgrades an occupied database.

Release 7 adds `article_read_events` for the Home calendar. Upgrade from release 6
creates an empty event journal in one transaction, without rewriting articles or
inventing historical reading dates. Use the same backup/rehearsal procedure below.
See [reading activity](reading-activity.md).

For release 6, add required `search.query_bytes`, `search.page_size`, and
`search.excerpt_characters` (example: 512, 25, 220). See [search contracts](search.md).
Build and validate the candidate first. Before upgrading release 5, stop/drain the
application, retain its image/config/master key, create a fresh backup, and restore
it to a separate database. Run the candidate's `upgrade-schema` against that restored
database and compare all preexisting table counts and source content. Only after the
rehearsal succeeds, run the same command against production and start the matching image:

```sh
docker compose run --rm --no-deps app --config /etc/inoreader/config.yaml upgrade-schema
```

The command adds the search projection and trigram indices, validates source chunks,
and backfills visible article text in one transaction. Source rows are not rewritten.
It refuses a second upgrade or invalid historical content. Failed conversion leaves
the previous schema/data intact. Rollback requires the old image and matching backup;
never restore over newer user activity without preserving and reconciling it.

## Incremental container builds

The production Dockerfile keeps Cargo registry, git, target, rustup, and npm
caches in named BuildKit cache mounts. A source edit invalidates the relevant
image layer but reuses compiled dependencies and unchanged workspace crates.
Run the normal command; no host Rust toolchain or bind-mounted build directory
is required:

```sh
docker compose build app
docker compose up -d --force-recreate app
```

Do not use `docker builder prune` during routine deployment because it deletes
these caches and makes the next build cold. `.dockerignore` keeps local build
artifacts, repository history, runtime configuration, and credentials out of
the build context.

The PostgreSQL pool size and acquire deadline are explicit validated settings.
All values must be positive before a connection is attempted.
Authentication exposes the Argon2id memory, time, and parallelism costs; invalid
combinations fail startup validation and the chosen costs are embedded in every
new password hash.

`observability.log_format` selects text or newline-delimited JSON completion
records for external requests. `external_request_metrics` is mandatory in v1
and must remain `true`; completion logging remains enabled independently.

TLS terminates at the Caddy reverse proxy in `compose.yaml`. Set
`EXTERNAL_HOST` in `.env` to the same DNS name as `server.external_origin`.
Caddy obtains and renews its certificate; ports 80 and 443 must be reachable
from the Internet. The proxy must preserve the
configured external origin and must not expose Chromium or PostgreSQL ports.

## Backup and restore

PostgreSQL is the production source of truth. Back up the database persisted in the named
`postgres-data` volume with `pg_dump --format=custom` from the pinned PostgreSQL
image, write to a new operator-owned path, and keep the password in the Docker
secret. A valid backup is not just a successful command: restore it into a fresh
PostgreSQL database, verify the matching schema (or rehearse the explicit upgrade), compare every table count, and run the
authentication, library, content, queue, and cross-user isolation smoke tests
before accepting it.

A custom-format backup can be created from the running PostgreSQL container:

```sh
umask 077
set -o noclobber
docker compose exec -T postgres pg_dump -U inoreader -d inoreader -Fc > NEW_BACKUP_PATH.dump
```

Check the command's exit status and the archive with `pg_restore --list`. Keep
incomplete output after a failed dump separate from accepted backups. Restore
only into a newly created, separate empty database using `pg_restore
--exit-on-error --single-transaction`; do not use `--clean` against the running
database. Preserve the AI credential master key separately. Restoring the DB
without its matching master key cannot recover encrypted API or bot credentials.

The release gate includes the digest-pinned PostgreSQL Docker acceptance test.
It creates the complete schema, checks idempotency and constraints, exercises
lease theft fencing, and proves that two users may share a public fetch source
without sharing workspace, subscription, activity, or article state. It also dumps and restores the entire database, comparing every field in every
application table, restoring schema/indexes and resuming a saved job. Missing Docker is a hard failure, never a skipped test.

## Initial source seed

`source-inventory/inventory.json` preserves all 42 legacy source definitions.
PDF OCR candidates remain untrusted until visual review. Generate a draft for
one explicit owner and workspace:

```sh
just seed-preview ACCOUNT_UUID WORKSPACE_UUID seed-manifest.json
just seed-validate seed-manifest.json
```

The draft records an idempotency key per source and contains no password or
infrastructure secret. All 42 sources map to the shared standard-feed collector,
the validated generic HTML recipe, or one of seven built-in publisher adapters.
Validation rejects unresolved, disabled, duplicate, or cross-owner entries
selected for application. Review it, then apply it with the same binary:

```sh
inoreader --config /etc/inoreader/config.yaml seed seed-manifest.json --apply
```

Application independently parses every selected recipe into validated runtime
types and rejects unknown fields, adapters, selectors, patterns, URLs and limits.
It verifies the manifest account owns the target workspace and commits
all selected subscriptions, source recipes, and initial jobs in one transaction.
Stable manifest keys make retrying the same manifest idempotent; a conflicting
reuse of a key fails without partial changes.

## Failure behavior

PostgreSQL unavailability makes readiness fail and never selects local or in-memory
storage. Chromium unavailability degrades browser-backed jobs while ordinary
feed ingestion and reading saved articles continue. `archive_budget_bytes` is a
diagnostic telemetry threshold in v1; it never deletes content, truncates input,
or rejects records. Configuration and schema errors stop startup before workers
begin.

## Chromium acceptance

The release gate runs `tools/run_chromium_acceptance.sh`. It starts the pinned
the digest-pinned `chromedp/headless-shell` 151.0.7922.109 image on the
loopback-only CDP port,
waits for `/json/version`, runs the real CDP collector
acceptance crate, and removes the container through an exit trap. The suite
checks JavaScript actions, preview geometry, repeated scheduled collection,
SSRF interception, and degraded/recovered probes. Missing Docker, an unavailable
image, or a failed health check is a release failure and is never converted to a
skip.

## Consistency schema upgrade (2026-09-27)

This release requires the explicit offline SQL steps documented in
[consistency architecture](architecture/consistency-2026-09-27.md#deployment-and-rollback-contract).
The normal startup path does not migrate obsolete duplicate article snapshots or
split old AI documents implicitly. Preserve a verified old database/image/config
set before upgrading. Configure `server.graceful_shutdown_seconds: 700` and
`CONTAINER_STOP_GRACE_SECONDS=720` (Compose default); startup rejects insufficient
budgets for admitted two-stage AI work. Larger configured AI leases require larger
application and container budgets too.

### Source collection recovery

`retry_attempts` and `max_retry_age_seconds` bound one collection attempt cycle.
Exhausting either budget on a recurring `PollSource` or `CollectWebFeed` schedules
a fresh cycle after `scheduler.polling_interval_seconds`, resetting its attempt
counter and cycle age. The failed diagnostic and source health remain visible
until a successful collection. One-shot work (including manual refresh, full-text
extraction and deliveries) still becomes failed on exhaustion; it is not silently
replayed forever. A paused source is not fetched even when its periodic job wakes.

HTTP backoff starts when the failed request finishes, rather than when its queue
lease was claimed. For HTTP 429/503, the shared secure fetcher retains `Retry-After`
(delta seconds or HTTP date). Both retries and the next recurring cycle respect
that minimum time. Invalid or unrepresentable cooldowns fail explicitly; there is
no silent cap. Browser navigation errors currently do not expose response headers
through their collector contract, so this header handling applies to the direct
secure HTTP fetch path.

Existing failed recurring jobs need an explicit recovery/requeue after deploying
this fix. Scope that operation to the intended user's subscriptions and sources;
do not reset unrelated users' jobs or clear source health to make errors disappear.
For each recovered source, verify a successful collection and a future scheduled
job, not just an HTTP 200 response. XML/HTML parsing and card identity validation
can still fail after a successful HTTP fetch.

### Shared HTTPS proxy pool

`http.proxy_pool` declares the single ordered endpoint list and retry settings.
`http.proxy_routes` assigns disjoint exact hosts to that pool, with per-route
`direct_first`. Both feeds and Telegram clone the same transport and share its
per-host health. With no pool and no routes, requests remain direct. A configured
pool without routes, or routes without a pool, fails configuration validation.

```yaml
http:
  # Other required HTTP settings remain unchanged.
  proxy_pool:
    endpoints: ["45.147.179.118:3129", "90.156.196.230:3128"]
    max_connect_header_bytes: 8192
    endpoint_attempt_timeout_ms: 35000
    quarantine_ms: 60000
  proxy_routes:
    - target_hosts: [api.telegram.org, t.me, medium.com]
      direct_first: false
    - target_hosts: [www.youtube.com]
      direct_first: true
```

Endpoint addresses are examples, not guaranteed services. Preserve every configured
endpoint for recovery, even when presently unhealthy. The attempt budget includes
CONNECT, TLS, headers and the streamed body. It must fit the HTTP deadline and,
when Telegram is routed, exceed the Bot API long-poll timeout and fit its deadline.
The client's connect timeout bounds each CONNECT/TLS phase separately. All limits
are explicit configuration, validated before I/O.

CONNECT contains only an authorized public destination IP and port. Origin paths,
headers and bodies are written **after** certificate and hostname validation inside
end-to-end TLS. HTTP destinations fail before connecting. Redirects revalidate
scheme, origin and DNS/IP; the Bot API client is restricted to api.telegram.org.
Proxy authentication is unsupported. Neither plaintext origin requests nor TLS
verification bypasses are allowed. Direct clients ignore ambient proxy variables.

Public mode still rejects cookies, authorization, arbitrary headers and bodies.
The user explicitly authorized Telegram credentials through this same free pool:
a separate narrow capability accepts only HTTPS POST to getMe/getWebhookInfo/
getChat/getChatMember/getUpdates on api.telegram.org. These operations are replay-safe
with the identical parameters; getUpdates acknowledges only the already committed
offset. sendMessage and other mutating methods fail closed. DeepSeek and Zhihu
remain direct. Public history uses the pool without this credential capability.

Transport failures quarantine only the failing **target host + endpoint** pair.
Other hosts can continue using that endpoint. Healthy endpoints are preferred to
untested ones, and active quarantines are skipped. After `quarantine_ms`, the next
real request may claim one recovery probe; concurrent requests cannot claim the
same probe. If it fails, a healthy endpoint is tried before other expired
quarantines. Normal recurring source polls provide periodic recovery opportunities;
there is no synthetic background traffic and the configured pool never grows.
Each endpoint is attempted at most once per authorized destination-IP attempt.
Health is shared by feed, browser, icon and Telegram transport clones in this process and
is reset on restart. An entirely quarantined pool fails explicitly without
connecting; later scheduled work retries after eligibility returns.

Any origin HTTP status, including 403, 429, 503 and Retry-After, is returned directly;
it never causes proxy rotation. A body failure remains visible to the caller and
never splices or replays content from another endpoint. An interrupted successful
body quarantines its pair; an HTTP error body's failure does not turn the received
status into evidence against the endpoint. Caller cancellation releases a recovery
probe without manufacturing a transport failure. TLS, DNS/IP checks and redirect
policy stay unchanged. No implicit direct fallback or proxy discovery is enabled.

Shared external-request diagnostics use system `public_proxy` with operations
`quarantine_probe`, `endpoint_quarantined` and `endpoint_attempt`. They contain
stable classifications/timings only, never source URLs, headers or secret data.
The enclosing HTTP request continues reporting its complete logical duration.

### Explicit identical RSS repeats

`SourceKind::RssCoalesceIdentical` is an operator-selected policy, never inferred
from a URL and never the default. It applies to UTF-8 RSS with one channel.
Complete original `<item>` bytes (including metadata outside the reader's field
projection) must match for repeated upstream IDs. The first occurrence and original
order are retained; differing repeats reject the entire poll before writes. The
`feed_identical_repeats` diagnostic records the source ID and number coalesced.
Existing upstream IDs, article IDs, read state and content are not rewritten.
The user approved this policy specifically for their Yandex Cloud subscription;
other subscriptions retain strict duplicate rejection.

Lease renewal and collection commits must be polled concurrently. A commit holds
its leased queue row while publishing source records; renewal can block on that
row from another connection. Awaiting renewal inside a selected heartbeat branch
suspends the very commit needed to release its lock. The worker therefore selects
between the entire renewal loop and the entire work future, dropping the other
future when either finishes. SQLx transaction drop rolls back cancelled work.
An `idle in transaction` application session after `INSERT source_records` with a
renewal waiting on the same queue row is a diagnostic signature of this deadlock.

### Public Telegram post adapter

`BuiltIn.telegram.max_pages` explicitly selects a bounded public-history window
(positive page count). The source URL must be exactly an HTTPS `t.me/<channel>`
URL. The adapter fetches `/s/<channel>` and follows only that channel's numeric
`before` cursor. Message permalinks are identities; edits preserve the identity.

Telegram posts do not have authored article titles. This adapter deliberately
persists an empty title and keeps the full caption in description/body. Photo-only
posts are retained. Original message-bubble markup is retained; the selected
adapter explicitly adds `img` elements for Telegram's CSS-background photos and
exact media-URL links for audio/video tags that the shared safe renderer omits.
Raw content remains stored separately from sanitized display content. Unsupported
Telegram labels are preserved, never filtered away as empty posts.

Full-text jobs for this adapter publish the stored post body for the exact source
revision. They never fetch the public permalink's Telegram application landing
page and never run article readability extraction over a social post. Other
providers retain their existing full-text-fetch behavior. Existing subscriptions
keep their IDs when an operator selects this adapter.

For rate-limited public sources, `scheduler.rate_limit_retry_seconds` sets a
positive minimum delay for HTTP 429 (example:300 seconds).
`scheduler.retry_jitter_seconds` spreads jobs within `[0, configured value)` using
stable job-ID offsets; zero explicitly disables jitter. The delay starts after
the response. A longer server `Retry-After` wins. Attempt exhaustion passes the
same minimum timestamp to recurring recovery, so starting a fresh cycle cannot
shorten the cooldown. Scheduler duration ranges and the combined retry/jitter
budget are validated before startup conversions; unsupported values fail rather
than wrapping signed seconds or panicking in chrono.

Subscription descriptions live in linked wiki pages. To retire an older installation’s
subscription notes, first copy every nonempty note verbatim into a private owner wiki
page and bind that page to its subscription. Back up the database, stop application
writers, then run `tools/remove_subscription_notes.sql`. It aborts unless every
nonempty note has an exact bound private copy. Startup rejects unconverted documents.
