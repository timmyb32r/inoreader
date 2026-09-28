# Needs-attention investigation — 2026-09-28

The list changed during the audit as recurring polls failed and recovered. The
initial DB snapshot included transient Hazelcast HTTP 522 and five failed YouTube
feeds; a later snapshot contained the following active failures:

| Source | Most recent diagnostic |
|---|---|
| Oracle data integration | `remote_http_status: 403` |
| Oracle Cloud Infrastructure Blog | `remote_http_status: 403` |
| Precisely | `outbound_rejected: browser_navigation_failed` |
| Streamkap | `outbound_rejected: transport failed: request_timeout` |
| Snowplow — YouTube | `outbound_rejected: transport failed: proxy_endpoint_body_timeout` |
| Airbyte — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| NATS JetStream mirrors/sources — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| Nexla — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| Dataslayer — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| Supermetrics — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| Transferia Go / Yandex Data Transfer — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| OWOX — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |
| Windsor.ai — YouTube | `outbound_rejected: transport failed: proxy_pool_quarantined` |

Lauren Balik remained archived and was not reactivated. Its old 404 must not be
counted as an active-source incident.

## Evidence

- Repeated collection was already enabled: five attempts/cycle, 30-minute polling,
  exponential backoff, configured jitter, rate-limit floor and Retry-After support.
  The old warning threshold counted attempts (three), not outage duration.
- Both Oracle endpoints still returned direct HTTP 403; Precisely timed out in the
  public probe and browser collection failed. These had roughly two days of failed
  history and must remain visible under a 24-hour policy. More burst retries do not
  establish that these publishers are reachable.
- Streamkap returned complete HTML (215930 bytes) in 0.19 seconds despite the latest
  collection timeout. Hazelcast recovered in the collection history during the audit
  but also returned a direct 403 in a separate probe; it is intermittent.
- The first direct YouTube request returned valid Atom in 0.23 seconds. The broader
  test of all 64 subscribed channel URLs returned 20 valid Atom feeds and 44 connect
  timeouts with a six-second connect deadline. This supports direct-first with a
  network-failure fallback, not removal of all reserves or a claim of stable access.
- YouTube proxy rechecks: 37.114.41.103:3128 (6.13s), 45.147.179.118:3129 (2.29s),
  and the previously removed 116.101.76.225:2080 (2.19s) returned valid Atom.
  193.37.71.46:10808 and 5.128.189.112:10808 timed out in this probe.

Public-only diagnostic snapshots are stored under the ignored
`.inoreader-state/attention-streak/` directory. They contain no sessions or cookies.

## Implemented policy

See [source-attention.md](../source-attention.md). A persisted failure streak,
backfilled from retained activity, controls the 24-hour threshold. Attempt errors
stay available immediately. Successful collection clears the streak. The frontend
uses the backend flag for filters, sorting and warning marks; queued continuation
alone does not create a warning.

YouTube uses direct-first HTTPS and a reserve pool on transport errors. Eight
previously successful proxy endpoints remain configured (including temporarily
failed ones). No random unverified candidates are enrolled. Recovery probes rotate
by least-recent attempt so a short request deadline cannot starve the pool tail.
Authoritative HTTP errors and Retry-After do not trigger proxy rotation.

## Verification

Regression coverage includes the exact 24-hour boundary, rapid repeated failures,
restart persistence, reset on success, new outage after recovery, retained immediate
diagnostics, offline backfill, byte-exact subscription preservation, frontend
classification, direct-success/no-proxy, transport-failure fallback and fair
quarantine recovery. `just check-release` passed: full Rust format/Clippy/tests, real PostgreSQL
and Chromium acceptance, 176 frontend tests, 53 browser scenarios, generated
contracts and architecture/operational checks. Production checks follow deployment.


## Production verification

Deployed release 4 on 158.160.186.87. The offline pg_dump catalog was verified;
all pre-existing table row counts matched before and after the upgrade. The app
container became healthy. API classification returned exactly three active
attention sources: Oracle Data Integration, Oracle Cloud Infrastructure and
Precisely. No warning was cleared by modifying a subscription or deleting history.

All 64 YouTube sources had zero consecutive failures in the final API snapshot.
The four still failing at the start of the explicit refresh verification recovered
within 30 seconds (Transferia, Nexla, Airbyte and Windsor.ai). Runtime instrumentation
observed direct successes averaging 362 ms and reserve-proxy successes averaging
4348 ms in the sampled window; direct failures still occurred, confirming the
need for a reserve. These are a short production sample, not a future SLA.
The temporary owner verification session was revoked after the check.
