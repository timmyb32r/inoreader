# Reader improvements — autopilot obligation ledger

Authoritative scope: user's initial six-item request plus the additive source-analytics request on 2026-09-29. All existing changes
are preserved. Standing authorization includes deployment to 158.160.186.87.

| Obligation | Status | Acceptance |
|---|---|---|
| Selected article URL and browser history | verified | Article selection has a durable URL; reload/direct opening selects the same article; Back/Forward restores article and reader context without unwanted extra history entries. |
| Harmonious summary and terms window | verified | When both are open they share one movable, visually unified frame with two columns on wide screens and an accessible narrow-screen layout; either remains usable alone; independent content/state is retained and controls remain stable. |
| Draggable paragraph translation | verified | Drag header moves the translation window; text/controls remain usable; bounds, close, resize and mobile work. |
| One-click subscription wiki link from article | verified | Wikipedia-style icon beside subscription link opens its bound wiki page directly; permissions enforced; absent/inaccessible links don't create dead controls; multiple sources handled clearly. |
| Huawei Cloud publishing rhythm | verified | Investigate live data and extraction; fix any defect and repair dates where evidence supports them; no invented dates; history chart reflects available publication dates. |
| Copy entire article toolbar button | verified | Existing behavior inspected; a clear button beside summary tools copies the complete article, with immediate feedback and no shift; regression verifies full content. |
| Source contribution analytics page | verified | Arrivals card opens a dedicated permission-scoped page; today is selected initially; uniform-color treemap with rectangle areas proportional to article contribution and ranked volume/share; today by default and arbitrary inclusive calendar date ranges; clear multi-source attribution semantics; links back to sources/articles; immediate feedback and stable layout. |
| Release and live verification | verified | Required checks pass; deploy; smoke-test all seven outcomes; note any material limitation. |

Plan: inspect navigation, panels, article metadata and date extraction; implement
navigation and article actions first, then shared panels/dragging, then the
Huawei investigation/fix. Add targeted regressions, run release gates, deploy
and validate against live data. No additional user interview is needed.

Follow-up 2026-09-29 18:10: added source contribution analytics; no original obligation cancelled or replaced. Implement analytics after the original slices, before the shared release gate.

Follow-up: user replaced the analytics heatmap with a uniform-color treemap (area = article contribution), and fixed periods with arbitrary date ranges, default today. Only the analytics visualization/range requirements changed; all other obligations remain active.

Final checkpoint: all seven obligations verified and deployed. See
[verification report](../reports/reader-polish-2026-09-29.md). No obligation
cancelled or blocked. The requested visualization is a uniform-color treemap;
rectangle area represents article contributions, with today as the default and
arbitrary inclusive calendar date ranges.
