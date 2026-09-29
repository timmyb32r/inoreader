# Reader improvements — verification 2026-09-29

All seven requested changes are deployed to inoreader.duckdns.org.

- Selected article and workspace are represented in the URL. Regression tests cover browser history, direct reload, read articles outside the current feed, and workspace authorization.
- Summary and Terms share a draggable two-column desktop frame, with tabs on narrow screens and matching typography. Closing either preserves the other.
- Paragraph translation has pointer and keyboard dragging, verified against a cached translation in production.
- A permission-scoped wiki shortcut sits beside the subscription link. Its reserved slot prevents asynchronous layout shifts.
- Huawei publication extraction preserves raw evidence and distinguishes precise desktop bylines from date-only mobile duplicates.
- The toolbar exposes Copy article; production smoke copied 15,384 characters without changing article state.
- The arrivals card opens source analytics: uniform-color treemap, exact proportional areas, complete ranked list, today default, arbitrary inclusive dates. Unique article totals and source contributions are distinguished explicitly.

## Gates

`just check-affected`, `just check-release`, and `git diff --check` passed on the final implementation. Release includes Rust tests/Clippy/formatting, 193 frontend unit tests, 71 fixture-browser scenarios, real-backend browser acceptance, PostgreSQL backup/restore, and Chromium acceptance. The focused assistant-window suite also passed (11 scenarios).

Final image: `sha256:add078ab59fd0254e73c6de8456bf28c6d1d3a3a2383f8be237d83f83b961b41`.

Production smoke verified 70 treemap sources and 287 arrivals matching the home card, custom dates, shared assistant movement, full-text copying, article reload, and cached paragraph translation movement; zero browser render errors. Those counts are a point-in-time snapshot. Mutating API calls were blocked in browser smoke to avoid paid generations or changes to read state. Invalid periods/timezones and inaccessible workspaces were rejected by the production API.

## Date repair and preservation

A protected PostgreSQL backup was restored into a separate database before repair. Rehearsal changed 208 publication metadata records, preserved every article document exactly, and a second pass changed zero records. Production repair updated 196 records; concurrent ingestion had already populated some dates. Final API coverage:

| Source | Dated articles | Undated | Conflicting |
|---|---:|---:|---:|
| Huawei Cloud News | 8 | 0 | 0 |
| Huawei Cloud Blog | 200 | 0 | 0 |

The temporary database and its configuration were removed. The protected pre-repair backup remains on the server. No dates were invented; unresolved dates elsewhere remain explicitly unresolved. Existing unrelated working-tree changes were preserved. No commit was created by this task.
