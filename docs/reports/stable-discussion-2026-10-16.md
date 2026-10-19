# Stable article discussion — 2026-10-16

## Symptom and reproduction

The owner reported repeated “Response interrupted” errors during article discussion on October 15–16. Inspection of the account's retained chat completions found five whitespace-only replies among the latest twelve discussion calls. The latest failed conversation had two consecutive whitespace-only completions (546 and 457 characters). Its stored failure was `The provider returned an invalid or incomplete response`, rather than a browser disconnection.

## Root cause

The observed failure is empty output from DeepSeek JSON mode, a limitation explicitly documented by the provider: <https://api-docs.deepseek.com/guides/json_mode/>. The request history also supplied previous assistant answers as rendered Markdown while requiring subsequent output to be a JSON segments envelope. That inconsistency is removed; it was a plausible aggravating factor, not proven to be the sole cause.

## Fix

Discussion calls use documented chat prefix completion (<https://api-docs.deepseek.com/guides/chat_prefix_completion/>): `/beta/chat/completions`, final assistant prefix `{"segments":[`, and no JSON-mode response format. The parser starts with the same exact prefix and still requires a complete validated envelope, final stream markers, usage metadata and source-verified quotations. Prior assistant presentation text is losslessly JSON-encoded in the request history. Incomplete assistant responses remain excluded.

Initial summaries, factual review, extraction and ranking retain their existing protocols. No schema/configuration migration or automatic paid retries are introduced. Prefix and encoded history are included in the normal input/context validation and budget reservation. Every request remains on the shared direct HTTPS transport with normal cancellation, observation, and account-scoped persistence.

## Regression coverage

Dedicated tests exercise multi-turn history encoding and exact text round-trip, exclusion of unfinished messages, prefix isolation from summary/review, streamed continuation reconstruction, UTF-8 chunk splitting, verified quotations, rejection of fabricated quotes and whitespace-only replies, and one outbound call per attempt.

## References

- `crates/reader-ai/src/model.rs`: `GenerationInput::discussion_prefix` and `messages_json`.
- `crates/reader-ai/src/provider.rs`: request format/path selection and seeded stream parser.
- `crates/reader-ai/src/tests/chat_protocol.rs`: protocol regression cases.

## Verification and deployment

`just check-release` passed: full Rust tests/Clippy/formatting, real PostgreSQL and Chromium/backup-restore acceptance, 218 frontend tests and 121 browser tests. `just check-affected` also passed. The first compile caught a missing enum equality implementation, corrected with a pattern match; the new test's missing namespace was also corrected before the successful final gate.

Deployed `inoreader-app:stable-chat-20261016`, image `sha256:d28b10027707e344dfb79327aa15df816fc789c2ce83ab350e0397f573d3f62a`. Database-backed public readiness returned 204; container health is healthy. No schema/configuration/budget changes. Existing running image was preserved for rollback.

Live verification retried the owner's existing failed PostgreSQL discussion question once through the normal authenticated retry API. It completed successfully, with no error and a 1,138-character substantive answer in the saved conversation. The verification session was removed afterward. An initial camelCase request-shape validation error was rejected before generation and corrected; no extra provider attempt was sent for it. The actual completion validates the beta endpoint, prefix reconstruction, conversation history, worker/publication path, persistence and normal accounting together.

The confirmed whitespace-output failure is bypassed. Network outages or malformed provider responses can still fail explicitly; no claim of universal provider availability or automatic replay of uncertain paid requests is made.
