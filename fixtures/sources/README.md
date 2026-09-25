# Source fixture corpus

This directory intentionally contains no invented website responses. The legacy
checkout provides source recipes and historical result summaries, but no
byte-preserving HTTP response corpus for the current 42 sources. Those summaries
are retained in `source-inventory/inventory.json` as observations, not fixtures.
The per-source absence and current runtime-support gap are machine-readable in
`source-inventory/coverage-matrix.json`.

For each source, a future offline fixture bundle should contain:

- the unmodified response body and relevant response metadata;
- source ID, original URL, capture time and content type;
- the adapter/recipe version used for the capture;
- reviewed expected records, preserving URL, title, description absence versus
  empty values, publication data and pagination boundaries;
- separately labelled malformed or blocked responses where available.

Secrets, cookies, authorization headers and private user data must never enter
the corpus. A blocked response can test diagnostics but cannot establish working
extraction coverage.
