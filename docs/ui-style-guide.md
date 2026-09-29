# Inoreader UI style guide

The product uses a cool-slate neutral palette with teal actions. Components use
semantic tokens only; light and dark themes expose the same meanings.

Tokens are owned by `web/src/styles.css` (`:root` and `.theme-dark`).
Components must not define competing neutral palettes. The architecture test
rejects references to undefined tokens.

| Meaning | Token |
|---|---|
| App background | `--bg` |
| Surface / raised / muted | `--surface` / `--surface-raised` / `--surface-muted` |
| Border / strong border | `--line` / `--line-strong` |
| Primary / muted text | `--text` / `--text-muted` |
| Accent / hover / selected surface | `--accent` / `--accent-deep` / `--accent-soft` |
| Warning / warning surface | `--warning` / `--warning-soft` |
| Error / error surface | `--danger` / `--danger-soft` |
| Focus / elevation | `--focus` / `--shadow` |

`ModalDialog` owns focus trapping/restoration and modal dismissal. `FloatingPanel`
and `useFloatingPanel` own movable nonmodal overlays. `StatusRegion` stays mounted
inside the panel's reserved row. `AsyncButton` locks on activation and replaces
its visual content without removing the label's layout footprint.

Interactive controls keep their dimensions across idle, pressed, pending,
success, and error. Feedback appears on the initiating control or in a reserved
status region. Conditional content never inserts above the active target. Focus
rings use a two-pixel teal outline without changing geometry.

Reader layout uses three stable regions on desktop: navigation, article list,
and article content. Narrow screens show one region at a time with an explicit
back action. Source text is never translated or reformatted silently.

Article toolbar controls share a 36px height, 12px semibold labels, 16px icons
and the same neutral surface and border. Icon-only controls are square; the
translation character is an icon, not a separate label style. Only enabled
toggle states use the selected teal surface; Summarize and Terms are ordinary
actions. Hover and press feedback changes color without scaling the control.
Loading spinners occupy the same space as the icons they replace.

The article chat reserves its header, 36px status row and 80px composer. Copy and
send use icon buttons; request accounting and version selection are not part of
the reading surface. A complete first-pass summary can be shown as unchecked
while fact-checking runs. Replacement preserves scroll position and waits for
active presses, text selections and scrolling to finish.

Paragraph translation is an explicit `文` mode next to Summarize. A paragraph
click creates one cached account-owned translation. Inline word wrappers keep
source text, fonts, spacing, links and emphasis unchanged. The chosen dictionary
card shows the exact word, Mandarin pinyin and Russian meaning on hover/focus.
The whole-paragraph translation uses a fixed overlay with reserved header,
content and footer dimensions; network completion never pushes the article.
Annotation waits until text selection, pointer presses and active scrolling end.

The `Terms` action opens a fixed, movable glossary panel. Its header, status
row and footer keep fixed heights. Only the content pane scrolls. New definitions
precede a divider and exact known channel paragraphs with source links. Async
results defer during a press, selection or scrolling. Copy buttons write safe
rendered HTML plus plain text; unavailable rich clipboard uses plain text.

The session countdown reserves a fixed-width digital display and two fixed
control slots in the top bar. Running, paused and finished states never resize
these slots. Duration editing uses a modal with a reserved validation region.
Completion pulses only color/shadow; reduced motion uses a static highlight.

Source posts with no authored title keep an empty title in storage and the API.
Lists show a separate “Untitled post” absence label in the existing heading slot;
the caption remains the excerpt and is never promoted into a generated title.

The Zhihu profile section uses a masked, autofill-resistant multiline editor for
Cookie headers or DevTools cookie tables. Its action row, removal confirmation
row and feedback region reserve fixed geometry before interaction. Save validates
remotely before replacement; checking never mutates the saved session.

Wiki is a full-page area with namespace navigation and an independent reading or
editing panel. The editor reserves a 36px status row and a fixed-height two-pane
workspace; narrow screens switch Write/Preview in that same space. Publication
locks its fields and controls until the response, while private-draft saving
leaves editing available. Namespace access never inherits Reader admin rights.

Unified search uses a full page with a shared SearchField, All/News/Wiki controls,
a fixed-height status row, independent result/preview scroll areas and fixed
pagination. Query, scope and selection are URL-addressable. At narrow widths the
same two panes stack with explicit fixed viewport fractions. Trigram matches are
highlighted with the semantic accent tokens; server text never becomes markup.

Home's reading calendar measures distinct articles marked read per local day, not
elapsed time. Its server-backed summary, calendar and reserved status line retain
fixed geometry while loading or failing; unknown loading values use an em dash,
not a fabricated zero. Daily counts include explicit bulk marking.

The Home flow chart uses adjacent slate (`--text-muted`) arrivals and teal
(`--accent`) reads on one integer scale. Tooltip overlays never move either chart
or the reading calendar. The plot reserves 180px height during loading and zero days.

Feed exposes its unread total in the browser title. Subscription category tabs
show category totals independent of search/filter results, with reserved numeric
width. Copy full article uses the shared icon-only clipboard control and copies
the title, excerpt and full rendered body once full text is ready. Its pending,
success and error states preserve toolbar geometry. On narrow screens Summarize
and Terms retain their accessible names but show icons to keep all actions visible.

Article selection opens its persisted DeepSeek chat automatically. Chat remains a
fixed overlay and is hidden beneath settings and outside Reader. Every article
selection reopens its own conversation; no regeneration control is offered.
DeepSeek spending reserves a fixed-height 30-day plot and summary region; mode
segments use semantic colors, and exact-value hover cards are overlays. Unknown
billing is a separate reservation, never rendered as known zero spending.

Reading history reuses Feed. Home's read card, Read series and calendar days are
links into the selected local day, with immediate pending feedback. The Feed
filter uses the shared protected date field, a clear action and Back to Home.
Once displayed, its 128px region remains reserved when cleared or loading;
article rows show the matching read time separately from source dates.

DeepSeek settings reserve two native shared model selectors and a fixed-size Save
models control. Both start at Flash. Saving immediately locks both selectors and
shows a spinner within the existing button; success/error use the existing fixed
status area. Refreshing usage must not overwrite unsaved selections.

Reader assistants may dock into one movable frame when summary and terms belong
to the same article. `PanelDock` owns the shared position and proportional columns;
on narrow screens explicit Summary/Terms tabs retain both pane states. Pane headers
move the entire dock. Standalone panes retain their own positions. Paragraph
translation uses the same floating-position hook and a dedicated drag handle.
Source analytics uses a single teal treemap palette: area represents contribution,
not color. Hover details, chart height and date controls reserve their footprints.

Failed or interrupted factual checks keep the summary preview and the fixed status
row. The recovery icon is labelled “Retry verification”; it locks immediately with
aria-busy on activation. No automatic paid retry follows a failed check.

Focused reading (`/reading`) is a separate Feed-launched workspace: the original
occupies roughly two thirds of desktop width and the assistant has its own fixed
column. Summary (the summary and its complete conversation) and Terms share that column; the chat's content and
controller are reused without a floating shell or a new generation request.
On narrow screens Article/Assistant tabs share one body viewport. The article
must use static positioning here, overriding the ordinary mobile reader overlay.
A fixed footer holds the explicit personal-value rating (1–10), Read & next and
a permanently reserved status/recovery row. Selecting a score changes only
color/border inside a stable box. Pending completion disables repeat activation;
next article starts without a selected score unless it already has a saved rating.
