# DeepSeek operations

`reader-ai` owns the provider protocol, prompt gate, encrypted credentials and
conversation orchestration. `PostgresAiStore` owns account-scoped storage and
fenced jobs. The UI only receives public profile/chat DTOs; raw keys, the master
key, prompt snapshots and source snapshots never appear in those responses.

The optional `ai` configuration section enables the server integration. Its
`enabled_accounts` is an explicit UUID allowlist; the shipped example is empty.
An empty allowlist leaves the integration disabled without requiring a master
key or prompt artifact; adding the first account makes both relevant startup
contracts apply (the prompt file is needed only when approved).
`prompt_approved: false` permits configured accounts to manage their credentials
and balance but prevents every paid generation. Approval is separate from key
availability. Set approval only after the author evaluation described in
`deepseek-summary-spec.md` succeeds or the owner explicitly accepts the current
candidate for use with documented unmet criteria. This operational switch does
not certify factual correctness or replace the server's format/quote validation.

Current candidate04 has passed engineering gates but failed the fresh factual
gate (3 detected error cases among18 completed outputs from20 planned sources).
On 2026-09-27 the owner explicitly accepted this candidate for use and waived
another questionnaire. The separate
[deployment approval](../prompts/reading-data-news/deployment-approval.json)
authorizes enablement only for owner `541affad-f1b0-4dd1-90de-7f41b689d4e6`,
using candidate04 and review v3 on the standing authorized host. Deployment and
public smoke completed on 2026-09-27; see the
[deployment record](deepseek-deployment-2026-09-27.md). The shipped example remains
`prompt_approved: false` with an empty
allowlist. No new human score is recorded. See
[research outcome](research/summary-report.md); do not equate a successful
streaming smoke or the previous candidate's20/20 style votes with this separate
explicit operator acceptance. The failed factual gate remains failed.

Production now runs image
`sha256:24929210d1617d58f2e9630ae8573f51c7f84c5777c0961ea9b9a851e4ecbdc6`
with owner-only access. The owner has configured their provider key; the latest
public smoke confirmed it remained enabled and both saved chats were readable
after restart. New summaries default to Flash with background Pro verification.
See [the compact-chat deployment and verification record](tasks/flash-chat-2026-09-27.md).
The earlier deployment report records the original, then-keyless state.

`prompt_path` points to **style.md**, the editable style instruction. The compiled
`prompts/reading-data-news/transport.md` instruction is appended with one newline.
The standalone research deliverable `system.md` already combines those parts and
must not be supplied as `prompt_path`. `review.prompt_path` points to the separate
factual-review instruction (`review.md`), with the same transport appended once.
Both artifacts may be absent while approval is false. Enabling approval requires
both files; no single-request fallback is used for initial summaries.

For an explicitly budgeted pre-release streaming check, use the research-only
[`deepseek_smoke` harness](research/streaming-smoke-harness.md). It uses this same
production provider and outbound boundary; its default invocation validates local
inputs without reading a key or opening a connection. Paid execution is one
explicitly reserved request per invocation and remains the research caller's
responsibility, outside the production approval gate.

`generation_mode` is an explicit tagged choice: `{kind: standard, temperature:
0.3}` or `{kind: thinking, effort: high}`. Thinking supports `low`, `high`, and
`max`; it rejects a temperature rather than silently ignoring it. Standard
temperature must be finite and in `0..=2`. The selected mode is saved with each
conversation's model, prompt and output limit. Subsequent turns and retries use
that snapshot even after deployment configuration changes; explicit regeneration
creates a new version with the new configuration. Reasoning deltas are never
shown, persisted or logged. Provider completion-token usage includes reasoning
tokens, so cost estimates keep the full reported completion count. The nested
`review` configuration independently selects the review model/mode/output bound
and tariffs. Both prompt bodies/versions and both sets of parameters are frozen
in the conversation, along with context/input/response/framing limits.

Each conversation also freezes the exact USD input, cached-input and output
tariffs for that model. Retries and later turns estimate cost with those saved
rates, even when the worker configuration changes models or tariffs. Explicit
regeneration creates a new snapshot with the newly configured rates. These are
estimates at the saved prices, not a claim about the provider's current billing.
The required persisted `cost_rates` field is validated during deserialization;
missing or invalid rates fail explicitly instead of substituting current prices.

The server reads exactly 32 random master-key bytes from the file identified by
`AI_ENCRYPTION_KEY_FILE`. Compose mounts `secrets/ai-encryption-key` at
`/run/secrets/ai-encryption-key`; generate this once using a cryptographic random
source, keep it out of Git, and make it readable by application UID 10001. Do not
regenerate it during deployments: existing AES-256-GCM credentials cannot be
decrypted without the original key. Account UUID is authenticated encryption
data, preventing ciphertext reuse under another account.

Back up this master key securely and separately from the database. A complete
PostgreSQL `pg_dump` includes `ai_profiles`, `ai_chats` and `ai_operations`;
selective table backups must explicitly include all three. Restore
both the database and its matching encryption key before enabling the service.
Deleting an account's provider key cancels its pending generation and preserves
all conversations and immutable article snapshots.

Chats retain full extracted safe HTML and a plaintext snapshot whose text-node
boundaries are newline separators. Quotes are exact contiguous matches against
that plaintext. PostgreSQL captures the selected content manifest and all chunks
in one statement's MVCC snapshot. Source refresh and old chunk cleanup cannot
change an existing conversation. New versions are explicit; existing output is
never replaced by regeneration.

Generation is a durable job, not a long-lived API request. New initial summaries
and explicit regeneration use `deepseek-flash`, standard/non-thinking mode at
configured temperature0.3, then Pro/low checks the identical full article and
retained draft. The owner explicitly requested this faster flow on2026-09-27;
the candidate04 style/review text is unchanged, but the old Pro/high research
scores do not evaluate this new model configuration. Configured Flash tariffs
are peak-price estimates (USD0.3 input,0.006 cached input,1.2 output per million;
[provider prices](https://api-docs.deepseek.com/quick_start/pricing/) checked
2026-09-27); actual off-peak billing can be lower. Existing chats retain their
original model/mode/rate snapshots; New summary explicitly uses current defaults.

Once the first complete response passes transport and exact-quote validation,
the public API projects the retained draft as an early preview. It is labelled
not yet checked while verification continues in the background. Failure or Stop
keeps the preview; Retry runs only the missing check. Partial verifier output
never replaces it. Successful verification atomically replaces the displayed
body with the complete checked response. The original envelope remains retained.
The first standalone bold
heading must preserve every title byte, including Unicode whitespace. The worker
checks after recording known usage; the repository repeats the same invariant
at the final accepted-state transition. A mismatch fails explicitly and permits
review-only retry from the saved draft. The draft itself is allowed to contain a
title error that review can correct; chat replies do not require a title.
This second model pass reduces factual errors; it is not a
mathematical guarantee of truth. Follow-up questions use one streaming request.


The compact widget has no version selector, profile shortcut or request-cost
panel. Copy, new summary, send and recovery use labelled icons. Stop/retry/reconnect
appear only when applicable, in fixed reserved positions. Profile/key management
remains under the account menu. Older versions and per-call usage remain persisted
and available through the account-scoped API. The message viewport keeps its
scroll position/minimum reading surface when the checked result is shorter;
remote replacement waits until pointer/selection interaction and scrolling end
(180ms scroll-idle threshold). The fixed header/status/composer footprint prevents
asynchronous answer changes from moving controls.

Workers claim jobs
with leases; no database transaction stays open during provider I/O. The client
polls the account-scoped public projection. The shared outbound boundary streams real
provider bytes, enforces an overall deadline/body bound, validates each redirect
and IP, and never retries an ambiguous POST against another address. Complete
JSON segments of follow-up answers are published progressively; exact quotation
segments are checked before publication. Summary verification partials remain
internal and are replaced by the completed draft in every public API response. The lease must exceed twice the configured
provider request deadline so one claim can cover both stages. Context/input
limits fail explicitly, without truncation or
automatic conversation compression. Before any paid call, a conservative upper
bound uses the exact messages' UTF-8 byte count plus explicit
`framing_tokens_per_message`, `framing_tokens_base` and `max_output_tokens`
reservations against `context_tokens`. This may reject an article whose exact
tokenization would fit; it never truncates it or pretends this is an exact token
estimate. `max_input_bytes` remains an additional explicit byte budget. The
provider also enforces its exact token limit at the request boundary.

An expired job lease is marked interrupted, preserving verified partial content.
It is never automatically submitted again because provider billing may already
have occurred. Retry is an explicit new operation, retaining the original source
snapshot and question. If a complete draft was saved, retry resumes verification
only; it does not charge for another draft. Each request has an independent ID,
assistant attempt, phase, terminal outcome and optional reported usage. Completed
first-stage billing survives second-stage failure, cancellation and process
restart. Missing usage remains unknown. Late usage from an old request is saved
without publishing into or reviving a newer attempt. Client operation UUIDs and database locks prevent duplicate
paid attempts from reconnects or concurrent devices. Stop cancels reception; it
does not promise reversal of charges already incurred.

Technical logs contain stable operation names, status/duration and first-delta
latency. No key, article text, chat body, provider error body or request headers
are logged. The account-owned `ai_chats.document` retains request provenance,
token counts and exact decimal estimated costs atomically with draft transitions.
The unshipped standalone `ai_usage` table was removed to avoid two sources of
truth. Known costs and unknown request counts are displayed separately. Those
are application estimates at each saved model's rates; provider account balance is independent
and can include unrelated usage. Research spending has its own $10 ledger and
is not performed by these production workers.

Profile balance errors retain the last successful balance and its timestamp.
An invalid DeepSeek key produces HTTP 422, never Reader's session-expiry 401.
