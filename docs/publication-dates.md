# Publication and saved dates

Each article exposes two independent dates. `saved_at` is its immutable first
arrival in the workspace. `published_at` is an optional publisher declaration,
never an ingestion time. Feed ordering remains newest **saved** first; the
publication calendar uses publication dates only.

The validated `PublicationDate` contract in `reader-core` preserves three forms:
a date, a local timestamp without an asserted timezone, or an RFC3339 timestamp.
A date-only declaration never acquires midnight or a timezone. Source evidence
retains the original text and extraction label. Zoned timestamps are displayed
and grouped in UTC (the existing reader convention); local timestamps and dates
retain the declared calendar. The full source timestamp remains in the tooltip.

RSS/Atom/JSON Feed publication declarations take precedence over page metadata.
Atom `updated`, JSON Feed `date_modified`, HTTP Last-Modified, copyright years,
and dates guessed from URLs are not publication dates. Page extraction runs on
raw HTML before sanitization and stores its evidence with the content manifest.
It reads Open Graph publication metadata, Schema.org `datePublished`, explicit
publication elements, and an unambiguous article-scoped time. Verified byline
selectors for TapData, Alibaba Cloud, Oxide, Meituan, Postgres Pro, Yandex
Research and GitHub release pages are restricted to their domains and headers.
Different declarations must agree; conflicting declarations remain unresolved.
A date-only declaration may corroborate a supplied timestamp on the same day.

The projection batches origin metadata with the existing article query. It does
not fetch websites during a reader request. Publication-history aggregation streams
metadata rows and never loads article bodies. Refreshes naturally re-extract page
metadata through the existing outbound HTTP/security and content-commit boundary.

## Historical enrichment

`inoreader --config CONFIG backfill-publication-dates` performs a read-only audit.
`--apply` explicitly enables publication-metadata updates. It uses stored raw HTML,
configured ingest batch size and input byte limit, and reports per-source coverage,
missing content, decoding failures, conflicts and concurrent refreshes.

Apply replaces only the manifest's `publication` field, with a compare-and-swap
against the observed manifest. It cannot overwrite a concurrent content refresh.
It does not rewrite article text, source identities, saved timestamps or reading
state. Chunk corruption is an error, not a silently skipped record. Repeating an
unchanged run is idempotent. Before production apply, take a PostgreSQL backup,
restore it into a separate database, inspect the dry-run report, then run apply
and repeat the audit. Missing publication evidence remains explicitly unknown.

Standards: [Schema.org datePublished](https://schema.org/datePublished),
[Open Graph article metadata](https://ogp.me/),
[RSS item pubDate](https://www.rssboard.org/rss-specification).

Huawei publisher extraction: `bbs.huaweicloud.com/blogs/*` uses the unique desktop
`.article-write-time.isPc` publication byline. Its full text remains evidence;
only the exact `发表于` label is removed when parsing. The local clock retains
seconds without inventing a timezone. Huawei international news pages use their
unique direct-child `body > time[datetime]`, retaining its explicit offset and
precision. Duplicate candidate bylines stay unresolved. Retained raw HTML can be
reprocessed with the existing publication-date backfill command.
