# Automatic AI off-peak scheduling

User request: automatic summaries, interest ranking and term extraction must pause
in DeepSeek peak windows; explicit user requests continue under the daily budget.

Obligations:
- Implementation: verified — filter queued work and scheduling, recheck at paid
  admission, preserve jobs and resume at the exact window end.
- Calendar: implemented — Mon–Fri Beijing 09:00–12:00 / 14:00–18:00; weekends and
  official 2026 Chinese holidays excluded. Calendar expiry pauses automatic work.
- Verification: verified — boundaries, manual bypass, late paid admission,
  end-of-window deferral race, ownership and persistence. Full release gate passed
  on the final tree: `/private/tmp/peak-release-ui-verified.log` (Rustfmt/Clippy,
  all workspace tests, real PostgreSQL/backup restore, Chromium, 208 frontend units,
  117 browser E2E). `just check-affected` passed, 7.28s Rust plus frontend types
  (`/private/tmp/peak-check-ui-final.log`). Release tests required elevated local
  socket/Docker access; the sandbox-only run failed a TCP bind, not an assertion.
  Explicit schema 17→18 regression preserves existing read provenance byte-for-byte.
- Manual reader actions: verified — explicitly promote queued summaries/terms;
  passive article opening stays read-only. Stable targets and duplicate protection
  covered by new frontend unit/browser tests.
- Deployment and live smoke: verified — protected full backup, schema 17→18,
  configuration, restart and service health under standing deployment authorization.
  All preexisting table counts matched exactly before/after migration. Production
  policy returned 13:00 Moscow as the current automatic resume time. Holiday,
  weekend, exact window boundaries and budget-vs-peak deferral probes passed.
  Container healthy; public HTTPS `/live` returned 204. Running image equals
  candidate: `sha256:da8b84e3239684c555c91aee0f9ff2f96245b31c550b3a28dda4489fc721202e`.

Sources:
- https://api-docs.deepseek.com/quick_start/pricing/
- https://www.beijing.gov.cn/cs/gncs/zcwj/202603/t20260327_4568275.html

Configuration `ai.automatic_schedule` explicitly enables the timing policy and
contains the verified holiday dates and calendar validity. Weekend makeup workdays
remain off-peak, matching DeepSeek's explicit Monday–Friday rule. Update the calendar
before 2027; expiry fails closed for paid automatic work while manual actions remain
available. This change does not change configured tariffs or reset today's budget.

All candidates and paid admissions use one PostgreSQL clock/policy function.
Manual requests promote cached automatic jobs and reset their scheduling. A call
already admitted before a peak boundary may finish; subsequent paid stages wait.
Pending jobs are retained, not cancelled or marked read. Deferrals release leases
and resume at noon/18:00 Beijing, rather than waiting until the next budget day.

Protected production artifacts: `/home/timmyb32r/inoreader-backups/` contains
`off-peak-before-20261008.dump` (full archive decompressed with `pg_restore
--file=/dev/null`, verified marker), source/config archive, upgrade log and
before/after counts. The slow initial compression attempt was replaced by a fully
verified fast lossless archive; its own incomplete file was then removed.
Four older preservation reports were compressed losslessly and compared to their
originals before replacing them. Only obsolete project images were removed;
database volumes, historical backups and user data were preserved. Server space
remains about 6.8 GiB (97% used), below the preferred free-space margin; broader
data-retention changes are outside this deployment's authorization.
