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

