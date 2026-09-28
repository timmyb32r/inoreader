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
