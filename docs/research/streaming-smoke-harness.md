# Production-provider streaming smoke

`cargo run -p reader-ai --example deepseek_smoke -- ...` runs the production
`DeepSeekProvider`, `GenerationInput` framing, `VerifiedSegments` parser and
shared outbound DNS/IP/redirect/deadline/instrumentation boundary. It is a
research tool, not a server entry point: it bypasses the application prompt
approval/profile gate only through the explicit execution arguments below.
It does not test the PostgreSQL worker, browser UI or deployed credentials.
Those have separate integration/deployment checks.

Default invocation validates local frozen inputs only. It reads no credential,
opens no connection and writes no output:

```sh
cargo run -p reader-ai --example deepseek_smoke -- \
  --input /private/path/snapshot.json --stage generating
```

The JSON contains full source data and both immutable stage settings:

```json
{
  "article": {
    "title": "Exact original title",
    "source_url": "https://source.example/article",
    "safe_html": "<p>Exact archived full HTML</p>",
    "text": "Exact full plaintext used in the research snapshot",
    "source_revision": "frozen-research-source-id"
  },
  "generation": {
    "system_prompt": "Exact combined style + newline + transport prompt",
    "prompt_version": "frozen-style-version",
    "model": "deepseek-v4-pro",
    "generation_mode": {"kind": "thinking", "effort": "high"},
    "max_output_tokens": 16384,
    "cost_rates": {
      "input_usd_per_million_tokens": "1.32",
      "cached_input_usd_per_million_tokens": "0.044",
      "output_usd_per_million_tokens": "3.96"
    }
  },
  "review": {
    "system_prompt": "Exact combined review + newline + transport prompt",
    "prompt_version": "frozen-review-version",
    "model": "deepseek-v4-pro",
    "generation_mode": {"kind": "thinking", "effort": "low"},
    "max_output_tokens": 16384,
    "cost_rates": {
      "input_usd_per_million_tokens": "1.32",
      "cached_input_usd_per_million_tokens": "0.044",
      "output_usd_per_million_tokens": "3.96"
    }
  },
  "limits": {
    "context_tokens": 1000000,
    "framing_tokens_per_message": 64,
    "framing_tokens_base": 256,
    "max_input_bytes": 900000,
    "max_response_bytes": 16777216
  },
  "network": {
    "connect_timeout_ms": 10000,
    "request_deadline_ms": 180000,
    "max_redirect_hops": 5,
    "max_response_body_bytes": 16777216,
    "cancellation_poll_ms": 250
  }
}
```

These values illustrate the schema, not approval of a candidate or assertion of
current provider prices. Fill them from the exact frozen candidate/source and
explicit operational limits. Both complete stage settings are checked before
any request. The complete actual source+draft context is checked again before
verification; exceeding a limit fails without truncation or a paid request.
Prompts are supplied combined, with transport exactly once: this harness never
silently appends, strips or changes their bytes.

The root researcher must first reserve **one** request with the external strict
$10 research ledger at its model/output bound. Only after reservation succeeds:

```sh
cargo run -p reader-ai --example deepseek_smoke -- \
  --input /private/path/snapshot.json --stage generating \
  --execute-reserved LEDGER-RESERVATION-UUID \
  --key-file /explicit/path/to/token --output-dir /private/path/new-draft-run
```

One CLI execution makes at most one provider request; it never retries or runs
the other stage automatically. The UUID is provenance, not a replacement for
ledger enforcement: the caller owns reservation, settlement and preventing reuse
across distinct output paths. No default key path exists. Credential-file syntax
is one token with at most one trailing LF or CRLF; embedded/extra whitespace is
rejected rather than stripped silently. The key is absent from all artifacts,
stdout/stderr and shared instrumentation.

For verification, reserve a separate request and explicitly supply the completed
draft's directory:

```sh
cargo run -p reader-ai --example deepseek_smoke -- \
  --input /private/path/snapshot.json --stage verifying \
  --draft-run /private/path/new-draft-run \
  --execute-reserved SECOND-LEDGER-RESERVATION-UUID \
  --key-file /explicit/path/to/token --output-dir /private/path/new-review-run
```

`--stage verifying --draft-run ...` without execution arguments is also a free
local validation. Verification rejects a changed source, prompt, parameters,
tariffs, limits or network settings; it revalidates the exact successful draft
envelope through the production parser before opening any connection.

Every executed run requires a new directory, created with mode0700; files use
mode0600 and `create_new`, so prior evidence is never overwritten:

- `snapshot.json`: exact semantic immutable inputs, including full text/HTML.
- `request.json`: reservation ID, stage, timestamp and exact production messages.
- `events.jsonl`: durably flushed validated publication events, known usage and
  shared outbound completion/elapsed timing. It contains private draft text on
  the generating stage; it must never be served as an accepted UI summary.
- `result.json`: validated raw envelope, rendered content, known token usage and
  exact saved-price estimate, first-publication/total milliseconds and count.
  `finish: validated_stop` means production accepted stop + DONE + complete JSON
  + exact quote checks; verifying additionally passes the same exact-title
  acceptance guard as the application. A generating-stage title error remains
  reviewable. On failure `finish: not_completed`, typed safe error,
  any validated partial content and any known usage remain. Raw provider errors,
  raw invalid response bodies and reasoning deltas are never stored.
  A complete envelope rejected only by the final title guard is retained exactly
  for diagnosis, with its known usage and explicit failure; it is not accepted.

The provider API intentionally does not expose raw failure finish reasons. Do
not infer `length` or a precise HTTP error from `not_completed`. A failed process
or missing result is not zero cost. Settle only verified provider usage; preserve
the full reservation when usage is absent/uncertain. A partial `events.jsonl` may
retain known usage if final result writing failed. Ctrl+C requests cancellation
through the production polling seam; it neither retries nor rolls back charges.

The harness itself never modifies the ledger or imports a Reader profile key.
Compiling, testing and free validation do not call the provider.

Usage uses the existing application DTO names: `promptTokens`,
`completionTokens`, `promptCacheHitTokens`, `promptCacheMissTokens` and
`estimatedCostUsd`. The ledger wrapper must deliberately map those counters to
its provider-usage schema when settling; it must not treat missing counters as
zero. Snapshot price fields retain their existing snake_case configuration names.
