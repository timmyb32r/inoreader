# Offline blind author review

Generate one standalone HTML file, with no external runtime or network requests:

```sh
python3 tools/style_research/render_blind_review.py \
  --manifest .inoreader-state/style-research/blind-review/manifest.json \
  --output .inoreader-state/style-research/blind-review/index.html
```

The manifest has exactly this shape (20 items by default; use an explicit
`--expected-count` for a different experiment design):

```json
{
  "schema_version": 1,
  "evaluation_id": "opaque-unique-evaluation-id",
  "items": [
    {
      "id": "opaque-unique-item-id",
      "title": "Exact original article heading",
      "url": "https://primary-publisher.example/article",
      "text": "The complete generated summary, including its heading."
    }
  ]
}
```

Do not encode experiment arms, models, reference IDs, or quality outcomes in the
opaque IDs or text. The generator rejects additional fields instead of embedding
an experiment record in the page. It never loads author reference files. An
existing output file is not overwritten; use a new evaluation ID and artifact for
a changed experiment. The example has one item for clarity and therefore requires
`--expected-count 1` if executed as written.

The UI has no preselected verdict and does not automatically advance after a
vote. Comments and fact/number/quote flags are optional. Export includes all
items, their exact original text, the explicit verdict or null, flags, comments,
and timestamps. It declares `partial` until every item has an explicit verdict;
unchecked issue boxes mean “not flagged,” not “verified correct.”

Local drafts are separated by evaluation ID and checked against the complete
manifest before loading. An incompatible, damaged, concurrently changed, or
unwritable draft is not overwritten. The UI retains current work in memory,
explains the problem, and supports exporting ratings and downloading the previous
draft when one exists. Browser persistence for local files varies; the exported
JSON is the portable result. Clearing browser storage removes browser drafts.

The safe DOM renderer supports headings, bold, emphasis, inline code, and
absolute HTTP(S) links. Unsupported Markdown stays visible text; source HTML is
never executed, and images are never fetched. The raw-text toggle and JSON export
retain the complete supplied string. Only explicitly clicked source/content links
open external pages. A restrictive CSP blocks background connections.

This independent research artifact uses a local shared field factory with opaque
names and autofill/password-manager protections. It does not import the app's
frontend runtime; no application UI modules are changed. This is the narrow
reason for using a separate field primitive outside `web/src`.

Verification:

```sh
python3 -m unittest discover -s tools/style_research -p 'test_render_blind_review.py'
node --test tools/style_research/test_blind_review.mjs
```

The browser tests use the project's installed Playwright and Chromium, test local
files, and do not require a running server. They cover XSS/network isolation, full
text preservation, immediate feedback, stable desktop/mobile controls, export
duplicate protection, explicit incomplete results, and draft isolation/recovery.
