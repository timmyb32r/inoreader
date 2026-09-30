# Optional fact-check

The account's DeepSeek settings expose Off in the fact-check model selector.
The API requires both preference fields: a model selects verification, explicit
`verification: null` disables it. Missing/unknown values remain errors. Accounts
without preferences retain Flash for both stages.

Workers admit each next phase against current account preferences. Off skips
review context validation and the paid verifier. A fenced storage transaction
publishes only an exact retained draft backed by a completed generation, validating
its transport and quotations against the pinned article again. A queued retry can
reuse a previously paid draft without another call. Existing responses, calls,
usage, and uncertain billing reservations are preserved. Already admitted requests
may finish; turning verification back on affects future work.

Completed unchecked summaries keep the generating phase and show “not fact-checked”.
They support normal follow-up chat. Checked results keep their verification phase.

Regression coverage includes explicit-null preference round trips and omission
rejection, one-call summaries, no verification reservation, account isolation,
switching off during generation, retained-draft retry, lease fencing, missing
paid generation rejection, re-enabling, chat continuation, and selector persistence,
pending feedback, duplicate-save protection and stable control geometry.

The requested account override is $6 for 2026-09-30 (Europe/Moscow), with the
configured $3 daily default unchanged for following days.

Validation: `just check-affected` and `just check-release` passed, including real
PostgreSQL/Docker acceptance, 193 frontend unit tests and 83 browser tests. The
container was rebuilt and deployed to the authorized server. Authenticated API
and production browser checks confirmed Flash / Off and today's $6 limit.
The subsequent queue sample contained 59 summary calls and no verification calls;
57 unchecked summaries were completed. Temporary verification sessions were
revoked. The production browser check suppressed its automatic balance refresh
and made no paid request.
