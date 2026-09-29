# Subscription descriptions move to wiki

Personal note is removed from the subscription model, API, generated contracts,
editor, navigation guards, catalog search, column picker and sorting. Existing
catalog layouts discard the retired column; wiki unsaved-change protection remains.

The owner's two nonempty notes were copied verbatim into private namespace `DE`,
which has no shared members, and each page was bound to its subscription:

- Marc Brooker's Blog: `b7c5d0fb-d57b-4649-9196-af9447d774e3`.
- Stories by Lauren Balik on Medium: `d36957a6-6e33-473a-b8a5-e6aca45869d9`.

The offline retirement script refuses missing, changed, deleted, shared or
wrong-owner wiki copies. It removes only the retired JSON property. Startup and
subscription deserialization reject unmigrated data instead of dropping text.
A narrow operational script remains because existing installations need an
explicit, verified data transfer; no legacy application reader/API is retained.

Verification includes rejected deserialization, PostgreSQL rollback on missing or
changed copies, exact text preservation, idempotent retirement, absent API field,
and catalog/detail UI coverage. All release checks passed (193 frontend tests,
73 browser scenarios, Rust/Clippy and Docker PostgreSQL/Chromium/backup acceptance).
The frontend gate was rerun after correcting its localStorage mock; the earlier
passing backend gate covered the same production tree.

Deployed to 158.160.186.87. Authenticated production browser checks verified both
wiki links, absence of the editor/API field/catalog column, and the new search
hint. Database verification confirmed exact original text in both wiki pages and
no note property in their subscription documents. A protected pre-retirement
backup is retained on the server.
