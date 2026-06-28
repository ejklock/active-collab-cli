---
type: ADR
title: Organize the TUI as a layered module tree under src/tui/
description: Extract the flat view/draw code from tui/mod.rs into a layered module tree — view.rs, screens/, drawer.rs, theme.rs — restoring the screen-oriented structure lost in the Rust rewrite.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, tui, module-structure, ratatui]
timestamp: 2026-06-25T00:00:00Z
---

# 0007. Organize the TUI as a layered module tree under src/tui/

## Context

The Python TUI had a screen-oriented module layout (`tui/screens/*`) where each
screen owned its handle and render functions. The Rust rewrite
([ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md)) flattened all of this
into a 721-line `app.rs` + `tui.rs`, and a subsequent refactor
([S2a](/issues/index.md)) moved it to `src/tui/mod.rs` + `src/tui/model.rs`.
The result was still a single large file containing both the pure model logic and
all the view/draw code, making the TUI hard to navigate and extend: adding a new
screen required editing one giant match arm inside `model.rs`.

Force: maintainability and navigability of the TUI as screens grow. The TEA
core ([ADR 0006](/adr/0006-promote-crate-to-repo-root.md), S2a) gave us a clean
model/update layer; the view layer deserved an equivalent structure.

## Decision

Organize the TUI as a **layered module tree** under `src/tui/`:

| Layer | Module | Responsibility |
|---|---|---|
| Model (pure TEA) | `model.rs` | `Model` / `Msg` / `Cmd` / `Screen` / `update` / `mine_model` |
| Events | `events.rs` | crossterm `Event` → `Msg` mapping |
| Shell | `mod.rs` | runtime wiring, `setup_terminal`, `run_app` / `run_app_blocking`, `browse_blocking`, `run_mine` |
| Frame | `view.rs` | frame `Layout` split + footer; dispatches to each screen |
| Screens | `screens/{projects,tasks,detail}.rs` | each screen owns its `draw_*` fn (responsive `Table`; `detail` wraps text + renders a dedicated assets panel) |
| Shared widgets | `drawer.rs` | shared widget builders (`render_table`) |
| Styles | `theme.rs` | all ratatui `Style`/`Color` constants (`header_style`, `selection_style`, `asset_style`, `footer_style`, `SELECTION_SYMBOL`) |

`render.rs` is unchanged — it owns domain string rendering reused by the
`get`/`current` CLI commands and is not a TUI-layer concern.

## Alternatives considered

**(a) Keep the flat `app.rs` + `tui.rs` / single `model.rs`** — rejected: poor
navigability; a screen change required editing one large file; the CC-56 update
monolith problem.

**(b) Trait-object `Screen` abstraction** mirroring the Python OO design — each
screen a `Box<dyn Screen>` — rejected for now as heavier than the enum + match
the pure TEA core already uses. No present force requires dynamic dispatch;
adding a screen today means adding an enum variant and a new file, not a trait
impl. This can be revisited if the screen count grows substantially.

## Consequences

**Positive:**

- Each screen is independently readable, searchable, and testable — finding
  `draw_projects` takes one navigation step.
- Styles change in one place (`theme.rs`) with no risk of scattered
  `Color::Cyan` literals diverging across files.
- Clear seam for new screens: add `screens/foo.rs`, add variant, add match arm
  in `view.rs`.
- `mod.rs` is reduced to the runtime shell — the only stateful, async, terminal
  code.

**Accepted trade-offs:**

- More files: 7 modules instead of 2.
- A render change may touch a screen file and `theme.rs` — two files instead of
  one. Considered acceptable: the two files have distinct, non-overlapping
  responsibilities.

## Related

- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md)
- ADR: [/adr/0006-promote-crate-to-repo-root.md](/adr/0006-promote-crate-to-repo-root.md)
