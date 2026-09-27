# Inoreader UI style guide

The product uses a cool-slate neutral palette with teal actions. Components use
semantic tokens only; light and dark themes expose the same meanings.

| Meaning | Light | Dark | Token |
|---|---|---|---|
| App background | `#f5f7f9` | `#0c1117` | `--app-bg` |
| Surface | `#ffffff` | `#151c24` | `--surface` |
| Sidebar | `#edf1f4` | `#10171f` | `--sidebar` |
| Border | `#cfd8de` | `#34404c` | `--line` |
| Primary text | `#0b1220` | `#edf4f7` | `--text-primary` |
| Muted text | `#64717d` | `#9aa8b5` | `--text-muted` |
| Accent | `#0d9488` | `#39b8aa` | `--accent` |
| Accent hover | `#0f7f76` | `#55c9bb` | `--accent-hover` |
| Selected surface | `#e5f2f0` | `#173532` | `--selected` |
| Error | `#b42318` | `#ff8f86` | `--error` |
| Warning | `#9a6700` | `#efc15c` | `--warning` |

Interactive controls keep their dimensions across idle, pressed, pending,
success, and error. Feedback appears on the initiating control or in a reserved
status region. Conditional content never inserts above the active target. Focus
rings use a two-pixel teal outline without changing geometry.

Reader layout uses three stable regions on desktop: navigation, article list,
and article content. Narrow screens show one region at a time with an explicit
back action. Source text is never translated or reformatted silently.


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
