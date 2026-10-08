# Private DeepSeek statistics

Requested: a dedicated admin page for timmyb32r showing request volume by task and
by day/month/year, with charts and selectable date intervals.

- Implementation: verified.
- Access: authenticated exact username timmyb32r; other administrators are denied.
  Queries derive owner from the verified session, never a caller owner identifier.
- History: existing ai_spending journal, no migration or historical data rewrite.
  One admitted paid attempt counts once, including retries and known zero charges.
  Unknown billing is explicit; admission is not proof of provider receipt.
- Dates: inclusive Moscow billing dates; fixed day/month/year grouping; invalid
  dates, reversed ranges, unknown grouping and unknown query fields rejected.
- UI: /ai-statistics; private sidebar entry; arbitrary dates; period drilldown;
  stacked request chart, task distribution, exact decimal USD table and totals.
  Empty periods are zero only after successful loading, errors remain explicit.
- Verification: full `just check-release` passed on the final tree
  (`/private/tmp/ds-statistics-release-verified.log`): Rustfmt/Clippy, workspace
  tests, real PostgreSQL aggregation/isolation, Docker backup/restore, Chromium,
  211 frontend unit tests and 121 browser E2E. New tests cover exact decimals,
  zero-charge attempts, inclusive boundaries, month/year grouping, unknown billing,
  other owners, other admins, anonymous/forged-owner requests, range rejection,
  fixed controls, duplicate protection, error recovery and 1440/390px views.
  Desktop/mobile screenshots inspected. Fixed plot viewport reserves scrollbar
  space across different date ranges. No paid provider requests were used.
- Deployment: verified under standing AGENTS.md authorization. Container healthy;
  public HTTPS page 200, `/live` 204. Running image matches candidate
  `sha256:9fbcad321fb6b47679e07a926e87df92e06a235a6fc43ff88edfa76121fac20f`.
  Authenticated production day/month/year probes agree (1948 attempts in the
  selected current day: summary 850, terms 880, ranking 218 at smoke time).
  Anonymous access 401, forged-owner query 400, reversed dates 422; responses
  are `no-store`. Temporary smoke session was removed in a finally block.
  No schema upgrade or configuration edits; retained off-peak image for rollback.
- Final affected check passed (`/private/tmp/ds-statistics-check-verified.log`):
  Rust 5.46s plus frontend types. `git diff --check` passed.
