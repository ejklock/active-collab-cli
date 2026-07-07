---
type: ADR
title: Toggle terminal mouse capture so native text selection works on demand
description: Add a keyboard-toggled selection mode that disables crossterm mouse capture (restoring the terminal's native click-drag selection) and shows a footer indicator, then re-enables capture so the V2/V4 click affordances return.
status: Superseded
supersedes:
superseded_by: 0021
tags: [tui, ux, ratatui, crossterm, mouse, selection]
timestamp: 2026-06-26T00:00:00Z
---

# 0012. Toggle terminal mouse capture for native text selection

## Context

The TUI enables crossterm mouse capture for the whole session
(`EnableMouseCapture` in `TerminalGuard::new`, `src/tui/mod.rs`). Capture is what
makes the V2/V4 click affordances possible — list-row click to drill in, asset
click to open/download, `↗ Link N` click to open. The cost is that **while
capture is on, the terminal cannot do its own click-drag text selection**: the
emulator forwards mouse events to the app instead of highlighting text, so the
operator cannot select and copy a task id, a URL, or a snippet of a comment.

The user asked to "permitir e mostrar feedback quando selecionarmos texto" —
allow text selection and show feedback that selection is active.

Force: **usability of a read view** — operators routinely copy text out of a task
(ids, links, log excerpts). This is a presentation/terminal-mode concern; the pure
TEA core (`model.rs` `update`) must stay pure, so the actual mode change is an
effect, not a model mutation.

## Decision

Add a **selection mode**, toggled by the `s` key, delivered as slice **V3**.

### 1. Mode change is a `Cmd` (the shell owns the I/O)

Enabling/disabling mouse capture is a terminal side effect. `update` emits a new
effect `Cmd::SetMouseCapture(bool)`; the shell (`src/tui/mod.rs`) interprets it by
running `execute!(stdout, EnableMouseCapture)` / `DisableMouseCapture`. The pure
core never touches the terminal — it only flips state and emits the `Cmd`,
consistent with [BDR 0005](/bdr/0005-loader-single-flight-refresh.md) (effects as
data).

### 2. Model state + footer feedback

`Model` gains `selection_mode: bool` (default `false`). Pressing `s` toggles it:

- `false → true`: emit `Cmd::SetMouseCapture(false)` — the terminal regains native
  selection. The footer renders a visible indicator (e.g. a highlighted
  `SELEÇÃO` / `SELECTION` segment) so the mode is never silent.
- `true → false`: emit `Cmd::SetMouseCapture(true)` — click/scroll affordances
  return; the indicator clears.

The footer hint gains `s seleção` across the three screens, consistent with the
white-on-blue hint bar.

### 3. Scope while in selection mode

While `selection_mode` is on, mouse `Msg`s are simply not produced (the terminal
is not forwarding them). Keyboard navigation is unaffected. On terminal teardown
`TerminalGuard::restore` still runs `DisableMouseCapture` unconditionally, so the
guard stays correct regardless of the toggle's last value.

## Alternatives considered

- **`Shift`-passthrough only (no toggle).** Many terminals let `Shift`+drag bypass
  application mouse capture. Rejected as the primary mechanism: behavior is
  terminal-dependent and undiscoverable, and gives no on-screen feedback (the user
  explicitly asked for feedback). It remains available to users for free; the
  toggle is the portable, discoverable path.
- **Disable mouse capture permanently.** Rejected: throws away the V2/V4 click
  affordances the prior slices delivered.
- **OSC-52 "copy current selection" command.** A different feature (programmatic
  copy of an app-chosen region), not user-driven free-text selection. Out of scope.
- **Auto-disable capture when no clickable target is under the cursor.** Rejected:
  unpredictable, and the operator cannot tell which mode they are in.

## Consequences

**Positive:** operators can copy any on-screen text on demand, with explicit
feedback, without losing the click affordances. The mode change stays out of the
pure core (Cmd-mediated), so the toggle logic is unit-testable headless (assert
`update(model, ToggleSelection)` flips `selection_mode` and emits the right
`Cmd`).

**Accepted trade-offs:** one more `Cmd` variant and one more model field; the shell
grows a small capture-toggle arm. While selection mode is on, clicks do not drill
in or open links — that is the intended trade and is signalled by the footer.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md)
- ADR: [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md)
- BDR: [/bdr/0006-selection-mode-mouse-capture-toggle.md](/bdr/0006-selection-mode-mouse-capture-toggle.md)
- Issue: [/issues/0010-v3-text-selection-mode.md](/issues/0010-v3-text-selection-mode.md)
- Architecture: [/architecture.md](/architecture.md)
