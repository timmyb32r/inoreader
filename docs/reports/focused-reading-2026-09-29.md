# Focused reading — 2026-09-29

Separate Feed-launched reading workspace: a wide original, an embedded assistant
with Summary/Discussion/Terms, and a fixed personal-value rating/footer. Ordinary
reader selection and its mark-read behavior are unchanged. Entering focused mode
reads saved chats only; it does not create another paid summary. Drafts and request
ownership remain in the shared account-scoped chat controller.

Each completion requires an explicit 1–10 integer and atomically persists read,
rating, read event and an idempotency receipt. Undo targets a concrete receipt and
rejects later revisions. Reload after a lost response replays the same operation.
A previous rating is displayed as previous, without automatically enabling the
next completion under a repeated click. No ranking or inferred ratings were added.

The existing floating chat no longer owns a second copy of message/composer logic:
`ArticleChatContent` is shared with the embedded mode. Reading composition stays
under `app/reading`, respecting the existing frontend dependency guard.

## Verification

- `just check-affected`: passed Rust compilation and TypeScript checks.
- `just check-release`: passed; PostgreSQL Docker acceptance (including schema
  11→12, atomic completion, rollback, retry, undo, ownership and retention),
  backup/restore acceptance, Chromium acceptance, Clippy, formatting, frontend
  build, 193 frontend tests and 80 browser scenarios.
- One pre-existing paragraph-translation Escape-tooltip scenario needed its
  configured retry during the full run. Rechecked translation + focused reading
  three times with `--retries=0`: all 33 executions passed.
- Six new focused-reading browser tests cover rating requirements, immediate busy
  feedback, stable controls, exact retry after response loss/reload, draft/history
  isolation, missing full text, next-page failure, conflicting undo, keyboard,
  mobile, queue completion and previous-rating repeat-click protection.
- Desktop and mobile screenshots inspected; the ordinary reader's mobile absolute
  positioning is explicitly overridden in the focused workspace.

## Persistence and cost

Schema 12 adds current `article_ratings` and retained `reading_completions`.
Foreign keys are checked at commit to allow ingest's identity-preserving row
replacement. They prohibit permanent identity removal instead of silently deleting
ratings/history. A conflicting ingest merge of rated identities therefore fails
explicitly and requires resolution; there is no automatic rating winner policy.
Each completion adds one small transaction, event and receipt; queue fetching is
paged using the current stored timestamp without narrowing it through JavaScript.
No additional LLM call is made merely to open the focused screen. No quantitative
throughput claim is made; this change has no ingestion hot-path computation.

## Deployment

Deployed to `158.160.186.87`; container rebuilt, schema upgraded to 12 and app
restarted healthy. Read-only authenticated production browser checks confirmed
desktop/mobile rendering, a saved summary, ten rating controls and no floating
duplicate. No API write was attempted by the screen; article read/rating state
and saved conversations matched before/after. Temporary test session was removed.
