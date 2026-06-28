---
type: Issue
title: "V3 — selection mode: toggle mouse capture for native text selection"
description: An `s` key toggles a selection mode that disables mouse capture (terminal regains native selection) with a footer indicator, then re-enables it.
status: closed
labels: [tui, ux, mouse, selection]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## V3 — selection mode (mouse-capture toggle)

Let the operator select and copy on-screen text on demand. Implements
[ADR 0012](/adr/0012-mouse-capture-toggle-for-text-selection.md); pins
[BDR 0006](/bdr/0006-selection-mode-mouse-capture-toggle.md).

### Scope

Included: a `Cmd::SetMouseCapture(bool)` effect interpreted in the shell
(`src/tui/mod.rs`); a `selection_mode: bool` on `Model`; an `s` keybind in
`events.rs`/`update`; a footer indicator + `s` hint on all three screens. Excluded:
OSC-52 programmatic copy; any change to the click affordances themselves.

### Acceptance

- `s` toggles `selection_mode`; entering emits one `SetMouseCapture(false)`,
  leaving emits one `SetMouseCapture(true)` (BDR 0006 S1–S2).
- Footer shows a visible selection indicator only while the mode is on (S3).
- Keyboard navigation behaves identically in both modes; no input but Quit exits
  (S4).
- Toggling twice returns to the starting mode (S5).
- Terminal teardown still disables capture unconditionally.
- Pure update/view unit tests for the toggle, the emitted Cmd, and the indicator.

### Plan

Per ADR 0012: thread the Cmd through `dispatch_cmds`; keep `update` pure; add the
footer indicator in `view.rs`; cover with headless tests. The
`Enable/DisableMouseCapture` execution is the only shell seam.
