# Dropbox archive ingestion

The explicitly selected `BuiltInAdapter::Dropbox` reads the publisher-linked
`https://dropbox.tech/all-stories` page through the shared outbound boundary.
The homepage's Latest section has only six articles; the archive currently
contains 409 distinct article cards in its initial static HTML. No fabricated
page offsets, arbitrary item cap, browser, or hidden load-more click is needed.
This is a publisher window, not a guarantee that deleted historical posts remain
available. Existing records absent from a later response are never deleted.

Each card must provide exactly one absolute HTTPS Dropbox article URL, a
nonempty title and a valid publication date. The exact authored URL is the
upstream identity, matching the former RSS GUIDs. Duplicate identities,
ambiguous links, malformed dates, unexpected hosts and empty/challenge pages
fail the entire collection before commit.

The approved RSS-to-archive change is a **partial metadata update**, selected
only by this adapter. Listing HTML does not carry RSS descriptions, full content,
or timestamp precision. For an existing identity, retain those fields verbatim;
the title/location may update. Retain an existing publication value, including
its precision and offset, only when the listing's day agrees. Conflicting days
fail collection rather than choosing a winner. If no previous publication date
exists, store the authored day without inventing a time or timezone. New records
have the listing title/date and no fabricated body/description.

The engine applies this policy before the existing transactional poll boundary.
Other collectors retain their existing complete-record update semantics. Initial
archive delivery uses the configured initial-feed depth and persists any
remainder under the ordinary durable backlog mechanism.

Regression fixtures include the real 409-card archive and the ten previously
stored RSS identities/dates. Tests verify complete coverage, byte-for-byte
retention of descriptions/full HTML and exact timestamps, identity preservation,
serialization, duplicate/malformed rejection and contradictory-day failure.
