# Personalized article summaries: interview notes

Status: specification approved; candidate04 accepted without another questionnaire; deployed and public smoke verified on 2026-09-27.
[deepseek-summary-spec.md](deepseek-summary-spec.md) governs active implementation
and budgeted research. The original research acceptance criteria remain recorded,
but the user waived another blinded author evaluation on 2026-09-27 and authorized
completion with the current candidate04.
The user subsequently approved two sequential calls for initial/regenerated
summaries: draft generation followed by source-grounded factual review. This
two-call amendment alone did not approve a prompt. The later explicit product
acceptance below authorizes use; it does not declare the factual gate passed or
deployment complete.

## Requested outcome

Research the author's channel `https://t.me/s/reading_data_news`, match posts to
source articles, and develop a DeepSeek system prompt for faithful summaries in
the author's style. Add a prominent toolbar button beside Mark read / Read later
which sends the prompt and full article and opens a chat for follow-up questions,
quotations and rephrasing. Keep the provider credential server-side; its local
source was identified by the user as `~/.deepseek` (not read during interview).

## Confirmed decisions

1. Corpus: all of the user's authored article retellings throughout the channel's
   history. Check temporal variation rather than assuming the style is constant.
2. Content: verified explanatory context is allowed. Do not invent the user's
   opinions or personal experience.
3. Human evaluation: the user will review 15–20 blinded candidate summaries and
   identify departures from their style. Keep final evaluation examples separate
   from prompt-development examples; assess factuality separately from style.
4. Research API spending: hard ceiling of USD 10 for DeepSeek prompt experiments,
   separate from ordinary production chat usage. Account for completed and reserved
   in-flight costs and stop before exceeding the budget.
5. Inference deliverable: one fixed author-style generation prompt plus the article;
   fixed examples may be embedded. A separately versioned factual-review prompt
   receives the complete draft and the same complete source in a second call.
   Consider dynamically selected examples only if evaluation shows the fixed
   author prompt is insufficient, not as the initial design.
6. Initial availability: requesting user's profile has a DeepSeek API-key input;
   the feature becomes available only when a key is configured. Keep credentials,
   summaries and conversations isolated by account. Other-user style configuration
   is not part of this initial scope.
7. Show the provider account balance in the profile when available. Verified
   official API: GET https://api.deepseek.com/user/balance with Bearer auth. It
   returns is_available and balance_infos (currency, total_balance, granted_balance,
   topped_up_balance). Preserve returned currencies and decimal strings. This is
   current account balance, not the platform's full historical usage dashboard.
   Source: https://api-docs.deepseek.com/api/get-user-balance/
8. Conversations persist per article and account across closing, reloading and
   devices. Reopening resumes the existing summary/history without another paid
   generation. Regeneration requires an explicit separate action.
9. No live web search in production chat for the first version. Quotes must come
   from the article; general model explanations must be distinguished from the
   article author's claims. External source lookup remains in scope for research.
10. Chat UI: a draggable floating window over the page, like a conventional chat
    widget, rather than a side-by-side pane or article replacement. Opening it
    must not shift the article or surrounding controls. Support collapse/close;
    keep its drag handle and controls reachable within the viewport.
11. Prompt acceptance: at least 18 of 20 blinded summaries rated by the user as
    publishable in their style with at most minor edits, with no invented facts,
    distorted numbers or fabricated quotations. If the USD 10 ceiling is reached
    first, preserve results and report that the quality criterion is unmet.
    This is the original research criterion. On 2026-09-27 the user separately
    accepted candidate04 for use and waived another questionnaire; its failed
    factual result remains unchanged, and no new human score is assigned.
12. Initial `Summarize` and explicit `Regenerate` use **two sequential provider
    calls**: generate the draft, then check/correct it against the pinned source.
    Display stable `Generating` / `Checking` stages and publish the final summary
    only after completed review and server format/quote validation. Do not expose
    the unreviewed draft or partial review as the final answer. Completed model
    review reduces risk; it is not a factual guarantee or an independent audit.
13. Ordinary follow-up questions, explanations, quotation requests and rephrasing
    remain **one-call** chat turns. Server verification of exact quotations still
    applies; there is no automatic second provider call for those turns.
14. Keep the complete draft internally, separate from published conversation
    messages, with its exact source and prompt/model versions. If review fails or
    is stopped, an explicit retry reviews that saved draft using one new call;
    it does not regenerate the draft. An incomplete draft cannot be treated as a
    completed source for review. Errors never silently publish the original draft.
15. Stop cancels the current stage and prevents the next stage from starting,
    including races between draft completion and review start. Close/collapse
    preserves the existing job and history. Retain completed drafts and previous
    conversation versions; cancellation does not promise a provider refund.
16. Account for usage/cost/outcome per provider call and per attempt, including
    draft, review, review retry and ordinary chat. Unknown provider billing after
    transport failure/Stop is not zero; show the known amount and unknown part
    distinctly, and retain the research-budget reservation. Reserve both initial
    stages before a budgeted chain starts; an explicit retry needs a new reserve
    without erasing an earlier unknown charge. A retry after an uncertain outcome
    can incur an additional charge and must make that fact clear.
17. Use persisted operation/stage/attempt identity to prevent duplicate paid calls
    from repeated clicks, duplicate commands, restarts or multiple devices. Do not
    promise provider exactly-once execution after an uncertain response and do not
    blindly retry it. Publish one final summary after successful review; source
    snapshots, drafts and per-call records remain strictly account-isolated.

## Interview outcome

The user approved the specification and later explicitly approved the two-call
summary flow above. There is no pending approval question for those requirements.
Model/prompt quality and release readiness remain results to establish through
research and verification, not consequences of requirements approval.

### Explicit final acceptance / questionnaire waiver — 2026-09-27

The user stated they could not complete another twenty-item questionnaire, asked
to consider the result accepted and continue to completion. The verbatim message,
owner UUID and exact candidate versions are recorded separately in
[deployment-approval.json](../prompts/reading-data-news/deployment-approval.json).
This authorizes production enablement of candidate04 (draft v17 + review v3) for
owner `541affad-f1b0-4dd1-90de-7f41b689d4e6`. Deployment and public smoke completed
on 2026-09-27; see the [deployment record](deepseek-deployment-2026-09-27.md).
The production API key is intentionally unconfigured until the owner enters it
in Profile. No paid request was made against the deployed application.

This is an explicit product decision, not twenty new positive ratings. Previous
19/20 and 20/20 author scores remain tied to their original outputs. Candidate04
has no new human score; its factual gate remains failed (three detected error
cases, plus one uncertain reviewer and one exact-title failure). Frozen prompts,
manifests, outputs and audit findings are unchanged. No further questionnaire or
quality-acceptance question blocks completion. Deployment verification is recorded
independently and does not change the failed factual gate.

## Approved operational choices

Only the explicit toolbar button starts generation; opening an article does not.
Standalone glossary posts are a separate corpus category. Missing full text waits
or fails explicitly; oversized input is never silently truncated. Chats pin their
source snapshot; explicit regeneration creates a new retained conversation version.
The same snapshot is used by both stages. Only the completed reviewed output is
visible as the new summary; ordinary chat continues with one provider call per turn.
Evaluate the second stage for corrections, newly introduced errors and style/detail
preservation, including already-correct drafts as controls. Fixing known diagnostic
failures is not a substitute for a fresh held-out evaluation of the full pipeline.

## Initial evidence (not a completed style study)

The public preview is readable over HTTP. First sample: 20 posts, IDs 1523–1542,
dated September 22–23, 2026. Source matching is feasible even where the post has no
source link: post 1528 corresponds to AWS's article "Best practices for scaling
large consumer groups on Amazon MSK". That pair illustrates selection of mechanism,
concrete configuration and caveats; it does not yet establish corpus-wide rules.
Some posts are standalone definitions or contextual explanations; avoid treating
them as independent article-summary pairs without checking their relationship.

## Integration findings (read-only survey)

- ArticleReader currently lives in App.tsx; a separate ArticleChat feature avoids
  adding model state to the app shell. Editor controls use shared field primitives.
- PostgreSQL ArticlePresentation loads safe HTML from the latest successful
  content manifest. The browser DTO has no content revision or plain-text snapshot.
- ContentManifestPointer already carries record/source revision, refresh ID,
  fetch time and final URL. Chat input must pin its content version; old content
  cleanup means a retained conversation needs its own snapshot or retention ref.
- Authenticated API and repository boundaries must enforce account/workspace/
  article/thread ownership. Personalized summaries cannot cross account boundaries.
- The shared outbound HTTP client supports authenticated POST but buffers the
  response. Streaming requires extending that shared boundary; no direct SDK client.
- The ordinary request timeout is unsuitable for a long synchronous generation.
  Consider durable jobs plus streaming or polling without holding DB transactions.
- No AI config, prompt management, chat tables or model APIs exist yet. Reuse secret
  file mounts and redacted observability; never log keys, prompt/article/chat bodies.
- docs/architecture.md contains stale YDB assumptions; reconcile during approved work.
