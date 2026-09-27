# Telegram glossary — interview decisions

Requested 2026-09-27. Scope: design/specification only; implementation requires
explicit approval of the resulting specification (deep-interview workflow).
Proposed specification: [telegram-glossary-spec.md](telegram-glossary-spec.md).

## Explicit requirements

- Ingest historical posts from @reading_data_news and keep collecting new posts.
- Article action extracts companies, products, technologies, abbreviations and
  protocols, and presents their explanations as a list.
- Distinguish terms already explained in the channel from terms not explained.
- A channel definition is an article paragraph beginning with a **bold term**,
  followed by ` - ` and the explanation. Exact punctuation variants, paragraph
  boundaries and bold spans still need agreed semantics; do not silently broaden.
- Existing account/workspace isolation and shared secure outbound HTTP boundaries
  remain applicable. No public Telegram posting was requested.

## Discovered project evidence

- `docs/research/channel-inventory.md` documents 1,529 observed public-preview posts
  across IDs 1–1542, 77 archived pages and 13 unobserved IDs. This is not proof of
  complete authenticated Telegram history.
- `.inoreader-state/style-research/corpus/posts.jsonl` retains formatted HTML,
  raw post HTML, timestamps, permalinks and fetch provenance.
- Existing collector: `tools/style_research/collect_channel.py`; research utility,
  not an established production ingestion pipeline.
- Preliminary structural scan found bold-at-line-start definition candidates using
  both hyphen and dash variants, including some whole bold sentences. These are
  candidates only; no extraction policy or accepted glossary has been established.

## Decisions

- Known/unknown classification applies to all requested entity types, not only
  abbreviations (explicit user answer).

- Results layout: new definitions first, then a divider, then known definitions
  with links to their channel posts (explicit user answer). The user did not
  request an additional AI explanation in the known-definition section.

- Repeated definitions: show only the newest definition for a term and its post
  link (explicit user answer: “достаточно ограничиться самым свежим”). No expansion
  listing older definitions is needed. This is a presentation decision, not
  authorization to delete imported source posts.

- No semantic alias matching by DeepSeek (user answered “не надо” to allowing
  contextual equivalence between CDC and Change Data Capture). Do not automatically
  merge distinct names based on model knowledge. No separate alias mechanism has
  been approved.

- New definitions use the current article and DeepSeek's knowledge without
  external search or verification (explicit user answer). Do not label these
  definitions externally verified.

- Generated definitions are publication-ready paragraphs in the author's
  @reading_data_news style, not merely reference notes (explicit user answer).
  Reuse the established style research; do not imitate an assistant persona.

- Definition separators: accept hyphen (`-`), en dash (`–`) and em dash (`—`)
  after a bold term at the beginning of a paragraph (explicit “ок” to the proposal).

- Telegram ingestion: channel bot selected explicitly after the user raised the
  absence of a spare phone number. Create it through BotFather from the existing
  Telegram account; no dedicated user account/session is part of this design.
- Initial history: import the existing 1,529-post public-preview archive (explicit
  user answer), and also set up ongoing ingestion of fresh posts. Do not require a
  fresh Telegram Desktop export. Existing archive coverage remains limited; this
  decision does not establish that all historical posts have been observed.
- The time interval between the archived history and bot activation must be
  covered by the ingestion design or explicitly reported as unverified; starting
  the bot alone does not backfill it.
- New posts and edited posts known to the bot arrive through Bot API. Pending
  updates are retained for at most 24 hours; a bot alone cannot guarantee recovery
  after longer outages. Gap detection and recovery policy remain unresolved.
- Official references: https://core.telegram.org/bots#how-do-i-create-a-bot and
  https://core.telegram.org/bots/api#getting-updates.

## Proposed contracts awaiting specification approval

The specification makes the remaining choices concrete: exact name matching,
paragraph boundaries, verbatim known definitions, ordering by publication rather
than old-post edit time, bot polling and public-page reconciliation, explicit
coverage limitations, account-owned channel configuration and a floating results
panel. These are proposals, not retrospectively recorded interview answers.
Implementation has not started. Bot provisioning/access remains an operational
prerequisite for live ingestion, not a reason to invent working connectivity.

## Provisioning constraint raised during interview

The user asked whether a separate Telegram user account can be created without
an available phone number. Do not assume that a dedicated account is provisionable.
This constraint superseded the proposed dedicated MTProto account; the user
subsequently selected a channel bot. Telegram ties separate accounts to separate numbers; official no-SIM
registration uses an anonymous Fragment number, not numberless registration.
Sources: https://telegram.org/faq#q-i-have-a-new-phone-number-what-do-i-do and
https://telegram.org/blog/ultimate-privacy-topics-2-0#sign-up-without-a-sim-card.
Accepted replacement: channel bot plus separately imported history. This changes
the autonomous historical gap-recovery guarantee; recovery policy is unresolved.
