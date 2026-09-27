# Paragraph translation

The `文` button beside Summarize enables paragraph selection when the account has
an enabled DeepSeek key and the article's full text is available. Clicking a
paragraph queues a translation; hovering never calls the provider. Completed
paragraphs have dotted word underlines. Hover/focus opens the selected dictionary
card design: original word, Mandarin pinyin with tone marks, Russian meaning.
Clicking a word also works on touch devices. Esc dismisses the card and paragraph
result. The button exits selection mode.

The full Russian paragraph translation appears in a fixed overlay, so neither
network completion nor annotations move subsequent paragraphs. Word wrappers
preserve the source text and HTML, including emphasis and hyperlinks. Paragraphs
and leaf list items are eligible; nested list containers and preformatted code
blocks are not. Inline code retains its original formatting.

## Contracts and ownership

- `POST /api/articles/{id}/translations`: `workspaceId`, `operationId`, exact
  `source` text. Authentication and CSRF precede work. The server verifies workspace
  ownership, article access, and exact `textContent` membership in archived HTML.
- `GET /api/articles/{id}/translations?workspace_id=...`: account/workspace-scoped
  persisted results and pending jobs. The UI polls only while work is outstanding.
- `reader-ai::translation` owns the validated source segmentation contract.
  Provider segments must concatenate to the exact original source, without
  normalization, omitted punctuation, whitespace changes or reordering. Empty
  translations, missing Chinese pinyin and malformed/incomplete responses fail.
  Persisted result deserialization repeats the validation.
- `ai_translations` stores account/workspace/article ownership, exact source and
  revision, model, frozen cost rates, job state and reported usage. PostgreSQL
  backup via `pg_dump` includes this table. No existing tables/data are rewritten.
- Concurrent requests for the same account, article revision and paragraph reuse
  one queued/running/completed job under an advisory lock. Exact strings, not
  hashes, determine identity. Different accounts never share cached translations.
- Workers use the existing configured concurrency, deadlines, output/context
  bounds and `max_message_bytes` paragraph size limit. Requests use the configured
  initial-summary model (currently Flash), JSON mode and thinking disabled.
  Translation uses its own prompt, without the author's summary prompt or a
  second fact-check call. Tokens and exact estimated cost are saved when reported.
- The shared outbound HTTP client enforces redirects, DNS/IP checks, byte limits,
  deadlines, credential redaction and completion timing. No browser-held API key,
  direct browser provider request, new transport or third-party service is used.
- A leased job interrupted by restart fails explicitly after lease expiration.
  No uncertain paid request is automatically repeated. Explicit retry creates a
  new attempt, preserving the previous one. Failed POST retries retain their
  operation ID until a definite server response is received.

Tests cover lossless segmentation and deserialization, exact paragraph membership,
provider payload/completion handling, a real PostgreSQL job lifecycle and tenant
isolation, duplicate activation, preserved HTML, immediate pending feedback,
instant hover details and stable article/toolbar geometry in Chromium.
