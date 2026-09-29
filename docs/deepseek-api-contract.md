# DeepSeek UI/API contract (implementation agreement)

All endpoints use current session auth and mutation CSRF. Account ownership is
derived server-side. Strings carry exact money; optional data is omitted, not null.

`AiProfile`: `{configured:boolean, enabled:boolean, balance?:{available:boolean,
balances:{currency:string,total:string,granted:string,toppedUp:string}[],updatedAt:string},
error?:string,availabilityReason?:string}`. enabled means configured and feature allowed for the account;
transient balance failures do not erase credentials. Never return raw keys.

- GET `/api/ai/profile`: saved status and last known balance.
- PUT `/api/ai/profile` body `{apiKey:string}`: validate/store; returns AiProfile.
- DELETE `/api/ai/profile`: remove key, preserve conversations; returns AiProfile.
- POST `/api/ai/balance`: refresh balance; returns AiProfile.

`ChatMessage`: `{id:string,role:"user"|"assistant",content:string,status:"pending"|
"streaming"|"complete"|"interrupted"|"failed",createdAt:string,
phase?:"generating"|"verifying",purpose?:"summary"|"chat"}`. Phase and purpose
are present on assistant attempts, including terminal failures. Content is safe-to-
render Markdown, never raw provider HTML. Publish quoted content only after source
verification; do not display an unverified quote as a verbatim excerpt.

`ArticleChat`: `{id:string,articleId:string,workspaceId:string,title:string,
sourceUrl:string,createdAt:string,model:string,promptVersion:string,
status:"waiting_content"|"queued"|"generating"|"verifying"|"completed"|"failed"|"interrupted"|
"cancelled",messages:ChatMessage[],providerCalls:ProviderCall[],error?:string}`.

`ProviderCall`: `{id:string,assistantId:string,phase:"generating"|"verifying",
status:"started"|"completed"|"failed"|"interrupted"|"cancelled",
usage?:{promptTokens:number,completionTokens:number,promptCacheHitTokens:number,
promptCacheMissTokens:number,estimatedCostUsd?:string}}`. Each entry is one
potentially billable request; absent usage/cost is unknown, never zero. Retain
known costs, unknown-request counts and earlier retry costs in the API; the
compact chat does not display these technical accounting details.

Initial summaries and regeneration have two paid stages. New summaries default to
`deepseek-flash` in non-thinking mode; the background checker remains Pro/low.
After the first complete, transport/quote-validated response, the API projects its
retained draft envelope into the incomplete summary's `content`. Its status stays
non-complete, and the UI labels it as not yet checked. Before a complete draft
exists, `content` is empty. Partial verifier text is never exposed: only successful
final verification replaces the preview. Failure/cancellation keeps the complete
preview available, with its non-complete status and explicit error. GET/list,
retry, stop and idempotent responses use this same projection. No schema change
or deletion of original drafts/older conversations is involved.
Follow-up `purpose:"chat"` uses one request and displays streamed content normally.
New summaries receive the original heading from the application, not the model.
Verification returns atomic segment-scoped corrections. Invalid corrections produce
`ai_review_incomplete` (422), preserve the unchecked preview and require a manual
retry. No rejected check is automatically resubmitted. Exact response/prompt and
reported usage are retained privately. See [targeted review](targeted-review.md).


- POST `/api/articles/{id}/chat` body `{workspaceId:string,operationId:string,
  regenerate?:boolean}`: return existing latest conversation, or create one when
  absent. regenerate explicitly creates a retained new version. Returns ArticleChat.
- GET `/api/articles/{id}/chats?workspace_id=...`: `ArticleChat[]` (versions).
- GET `/api/ai/chats/{id}`: latest ArticleChat including the complete early summary
  preview or streamed follow-up output. Client polls only while pending; browser reconnect does not regenerate.
- POST `/api/ai/chats/{id}/messages` body `{operationId:string,content:string}`:
  append user turn and enqueue one generation; returns ArticleChat.
- POST `/api/ai/chats/{id}/stop`: stop waiting/generation; returns ArticleChat.
- POST `/api/ai/chats/{id}/retry` body `{operationId:string}`: explicitly retry the
  last failed/interrupted assistant attempt using its pinned input and preceding
  user question. A summary with a complete saved draft retries verification only,
  preserving its `purpose:"summary"` and `phase:"verifying"`. No automatic retry
  is made after an unknown provider outcome. Keep earlier attempts/history/costs;
  returns ArticleChat.

Return current project ApiError format; 409 for busy/stale operations, explicit
errors for missing credentials/full text, context limits and provider failures.
Client-generated operation IDs identify retries of a local action. Database
constraints prevent duplicate jobs across concurrent devices. Provider transport
streams into durable state; polling transports progress to the browser.

Automatic summaries and budgeting (schema 8): `AiProfile.spending`, when AI is
configured on the server, returns dailyLimitUsd, today, resetsAt, spentUsd,
reservedUsd, remainingUsd and days[{day,mode,spentUsd,reservedUsd}]. Money is exact
USD decimal strings; dates use Moscow. Every mode shares one daily budget.
`ai_daily_budget` is HTTP 429. Chat list reads promote the most recent conversation;
background scheduling uses private store reads, without interactive priority.
See [the current automatic-summary contract](automatic-summaries.md).

## Account model preferences

`GET /api/ai/profile` includes `models: {summary, verification}`.
`PUT /api/ai/models` accepts exactly those two fields and returns the profile.
Both accept only `deepseek-flash` or `deepseek-v4-pro`; unknown/missing fields
are rejected. Authentication, account enablement and same-origin CSRF apply.
Missing stored preferences mean Flash/Flash. Saving never regenerates a summary.
