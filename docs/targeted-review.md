# Targeted factual review

Owner approved 2026-09-29 after deep-interview: targeted changes, no automatic
failure retries, retain Flash thinking/low for verification. Daily default $3 and
existing date-specific overrides are unchanged. No bulk historical regeneration.

The summary request returns body segments. The application prepends the exact
source title as its own heading segment (empty source titles remain absent).
The first complete draft is shown unchecked while a second request checks facts.
The checker receives the full unchanged source and exact saved draft. Its current
approved prompt asks for factual corrections without stylistic rewriting.

Protocol: `{"verdict":"unchanged"}`, `{"verdict":"unable","reason":"..."}`,
or `{"verdict":"corrections","changes":[{"segment":1,"before":"exact old
text","after":"replacement","reason":"factual reason"}]}`. Indices are zero
based; nonempty anchors must occur exactly once in the selected original segment.
Overlapping edits, unknown fields, invalid indices, contradictory/empty verdicts
and modifications of the source heading fail. All edits address original offsets
and apply atomically, never sequentially against mutated anchors. Explicit empty
replacements delete the selected text. Unchanged segments retain exact bytes and
order. Resulting quote segments must still be exact source substrings. These
checks prove safe application, not the truth of a model's factual judgment.

No partial review is displayed. A successfully validated patch atomically replaces
the preview. Failure keeps the draft with “Verification not completed” and the
explicit “Retry verification” action. Existing fixed status/control geometry and
interaction deferral remain. Operation IDs and row locks prevent duplicate calls.
Manual retries reuse the draft, source and reasoning/output settings. They use
the currently approved correction protocol, including for old saved drafts;
existing stored prompts, responses and completed summaries are never rewritten.

`ai_call_responses` privately stores exact returned draft, chat and review text plus its actual
system prompt, including rejected/incomplete responses within the configured byte
limit. It is separate from public chat DTOs and has no cascading deletion. Known
usage is settled even when a correction is rejected. Missing usage stays unknown;
no invented refund releases uncertain spending. Reasoning text is never retained.
Schema 11 requires explicit offline upgrade after backup. Startup never migrates.

Scheduler no longer retries failed or interrupted paid calls. Restart, reopening,
priority changes and midnight cannot revive those states. Only a budget-deferred
request that has not yet been sent can resume automatically. Candidate setup can
retry before creating a chat; its existing configured limits still apply.

Acceptance covers exact Unicode titles, unchanged/corrected/deleted fragments,
ambiguous and overlapping anchors, invalid JSON/verdicts, quote protection, atomic
failure, retained usage/raw response, stage-only manual retry, duplicate clicks,
lease expiry/restart and budget deferral. PostgreSQL and real provider-transport
fixtures exercise storage and SSE. Browser checks cover immediate busy feedback,
no double submission and stable composer/toolbar positions.

No additional paid evaluation or human questionnaire is required for deployment.
Savings are not asserted until observed: source input and thinking tokens remain
billable, and the model may miss factual errors. Historical results remain intact.
