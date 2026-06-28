---
type: ADR
title: App-managed text selection with a drawn highlight and clipboard copy
description: Replace the V3 mouse-capture toggle (ADR 0012) with app-managed selection — keep mouse capture on, track press/drag/release, draw a reverse-video highlight over selected cells, and copy the selected text to the system clipboard via arboard — so the operator gets real visual feedback while selecting.
status: Accepted
supersedes: [0012]
superseded_by:
tags: [tui, ux, ratatui, crossterm, mouse, selection, clipboard]
timestamp: 2026-06-26T00:00:00Z
---

# 0021. App-managed text selection with drawn highlight + clipboard copy

## Context

[ADR 0012](/adr/0012-mouse-capture-toggle-for-text-selection.md) (slice V3) added a
selection mode toggled by `s`: it disables crossterm mouse capture so the *terminal*
performs native click-drag selection, and shows a footer indicator.

The operator reports the real limitation directly: "não tenho feedback de texto
selecionado usando mouse… tem que ter feedback pro user saber que ele ta selecionando
trechos de texto." With the V3 approach this is **structurally impossible**: once
mouse capture is off, the application never receives the drag events — the terminal
owns the selection and its highlight, so the app cannot draw any feedback. The `s`
toggle also confused the operator ("pra que serve?") because its purpose (regain
native selection) is invisible.

Research ([/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md))
identified the alternative: keep mouse capture on and let the **app** own selection —
then the app can draw feedback and copy to the clipboard itself, in any terminal.

Force: **usability of a read view with visible feedback** — the operator must *see*
what is selected. The pure TEA core stays pure: selection is model state mutated by
mouse `Msg`s; the clipboard write is an effect (`Cmd`).

## Decision

Adopt **app-managed text selection**, superseding the V3 capture toggle. Delivered as
slice **V6**.

### 1. Selection is model state driven by mouse Msgs

`Model` gains a `selection: Option<Selection>` where `Selection` holds an anchor and
a cursor cell position (row/col in the rendered body viewport). Mouse capture stays
**on** (the V2/V4 click affordances are unaffected). The pure `update`:

- **Unmodified left button down** on the body → start a selection (anchor = cursor = cell).
- **Drag (move with button held)** → extend the cursor; the selected span is anchor→
  cursor in reading order.
- **Left button up** after a drag → finalize; emit `Cmd::CopyToClipboard(text)` with the
  selected text extracted from the rendered lines.
- A **plain (unmodified) click without drag** is a zero-length selection — not a
  selection. **Reconciled with [D1c](/adr/0020-body-links-inline-url-native-click.md) §2a:**
  it does **not** open a body link (link activation is now Ctrl/Cmd+click only). On the
  detail body it clears any existing selection (no-op otherwise); on list screens it keeps
  the screen's drill-in. A **Ctrl/Cmd/Super-modified press is reserved for activation** and
  does **not** start a selection — selecting and activating are disjoint, discriminated by
  the modifier.

### 2. Drawn feedback (the operator's ask)

While a selection exists, the view renders the selected cells with a reverse-video /
highlighted background, so the operator sees exactly what is selected as they drag.
The footer shows a brief "copiado"/"copied" confirmation after a successful copy.

### 3. Clipboard copy is an effect

`Cmd::CopyToClipboard(String)` is interpreted by the shell (`src/tui/mod.rs`) using
**`arboard`** (`Clipboard::new()?.set_text(...)`). The pure core never touches the
clipboard. A clipboard failure (e.g. headless/no display) degrades silently to a
footer note — selection still works, only the copy is unavailable.

### 4. Retire the `s` toggle

The `s` selection mode, `Cmd::SetMouseCapture`, and `Model::selection_mode` are
removed (ADR 0012 / BDR 0006 superseded). `TerminalGuard::restore` still runs
`DisableMouseCapture` unconditionally on teardown. Native terminal selection remains
available to power users for free via Shift/Alt+drag (terminal-dependent), but is no
longer the app's mechanism.

## Alternatives considered

- **Keep the V3 capture toggle, just improve the indicator.** Rejected: cannot
  satisfy the explicit request — the app structurally cannot draw selection feedback
  while capture is off.
- **OSC 52 clipboard write.** Considered for the copy step. `arboard` is more
  portable and does not depend on terminal OSC 52 support; OSC 52 remains a possible
  fallback for remote/no-display sessions but is not the primary path.
- **Whole-line selection only (simpler).** Rejected: operators copy partial strings
  (an id, a URL fragment); cell-granular selection is the point.

## Consequences

**Positive:** the operator sees a live highlight while selecting and gets a copy
confirmation — the exact feedback requested; selection works identically across
terminals; the confusing `s` mode is gone; click affordances are preserved.

**Accepted trade-offs:** new model state (`selection`) and a `Cmd::CopyToClipboard`
effect; the view gains a highlight pass; a new dependency (`arboard`). Selection
geometry must map rendered cells back to text (bounded to the body viewport). A
headless session cannot copy (degrades to a footer note). The change supersedes a
shipped slice (V3) — its tests are replaced, not amended.

## Related

- ADR: [/adr/0012-mouse-capture-toggle-for-text-selection.md](/adr/0012-mouse-capture-toggle-for-text-selection.md) (superseded by this)
- ADR: [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md)
- BDR: [/bdr/0015-app-managed-text-selection.md](/bdr/0015-app-managed-text-selection.md) (supersedes BDR 0006)
- Research: [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md)
- Issue: [/issues/0021-v6-app-managed-selection.md](/issues/0021-v6-app-managed-selection.md)
- Architecture: [/architecture.md](/architecture.md)
