# Source inventory

`inventory.json` is the checked, lossless import of the 42 source mappings in the
legacy `personal_feed/config.yaml`. Each `configuration` object preserves every
decoded YAML field and source ordering. Historical observations are evidence of
past discovery runs only: they are not raw HTTP responses and are deliberately
not labelled parser fixtures.

Regenerate it from an available checkout of the legacy service:

```sh
ruby tools/import_personal_feed.rb \
  /path/to/personal_feed/config.yaml \
  source-inventory/inventory.json \
  /path/to/personal_feed/docs/*-check.jsonl \
  /path/to/personal_feed/docs/discovery-retries.jsonl
ruby tools/validate_source_inventory.rb \
  source-inventory/inventory.json \
  /path/to/personal_feed/config.yaml
```

The nine-page raster PDF `00_29_30` exists at the path recorded in `SPEC.md`.
`tools/extract_inoreader_pdf.py` renders it and writes untrusted row evidence to
`pdf-ocr-candidates.json`. The detector uses the repeated enabled-toggle geometry,
so all 214 logical rows are represented even when OCR misses a URL. It finds 215
physical marker fragments and explicitly joins the single row split across pages
2 and 3. Each row retains its page image size, marker and cell-crop coordinates,
raw OCR tokens and lines, confidence and uncertainty flags. OCR never joins or
repairs a URL. Every row remains `needs_visual_review`; `pdf_inventory.rows` stays
empty until a human checks the exact name and URL against the rendered page.
Validators must never treat complete geometry as a successful source import.

Regenerate candidates with Poppler and Tesseract:

```sh
python3 tools/extract_inoreader_pdf.py \
  /path/to/screencapture-inoreader-preferences-content-feeds-2026-09-25-00_29_30.pdf \
  source-inventory/pdf-ocr-candidates.json
```

## Coverage contract

`coverage-matrix.json` is the explicit acceptance matrix. It joins every source
ID to its runtime implementation state, fixture provenance, independently
reviewed expectations and latest permitted test result. All 42 entries now map
to a compiled runtime path, but this is not source coverage: the matrix must
remain explicit about missing raw fixtures, reviewed expectations and executed
regressions. Inventory presence and successful compilation are not extraction
evidence.

- `configuration_only`: only the source recipe is locally available.
- `historical_observation_only`: a local historical check records a result, but
  no original response body is available.
- `raw_response`: a byte-preserving response plus independently reviewed
  expectations exists. No current source meets this bar yet.

Do not manufacture successful fixtures from selectors, titles, or current parser
output. Captured responses must include provenance, capture time, media type and
independently checked expectations before changing coverage to `raw_response`.

## Evidence-preservation regression corpus

`fixtures/contracts/` contains one deterministic contract for each of the 42
known sources. Each contract copies the exact imported configuration and the
historical observation fields, including explicit nulls and zero-result runs.
The seven specialized adapters also identify their ported legacy inline fixture
provenance. These files prevent configuration and evidence drift, but are not
raw HTTP bodies and do not claim that a source still returns the observed data.
Regenerate and validate them with:

```sh
python3 tools/build_source_contract_fixtures.py
python3 tools/validate_source_contract_fixtures.py
```

For the 214 PDF rows, build a self-contained crop review form from the original
PDF and the geometry candidates:

```sh
python3 tools/build_pdf_visual_review.py ORIGINAL.pdf \
  source-inventory/pdf-ocr-candidates.json /tmp/inoreader-pdf-review.html
```

The form shows the original pixel crop beside editable OCR values and exports
only rows whose reviewer explicitly checked them. It does not modify inventory
or promote OCR automatically.
