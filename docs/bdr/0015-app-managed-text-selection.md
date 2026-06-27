---
type: BDR
title: "App-managed text selection: drag highlights text and copies it to the clipboard with feedback"
description: Mouse press-drag-release selects text in the detail body; the app draws a highlight over the selection and copies it to the system clipboard on release, showing a copied confirmation. Supersedes the V3 mouse-capture toggle behavior.
status: Accepted
superseded_by:
supersedes: [6]
tags: [tui, ux, mouse, selection, clipboard]
timestamp: 2026-06-26T00:00:00Z
---

# 0015. App-managed text selection with drawn highlight + clipboard copy

<!-- Status lives in frontmatter. Observable behavior delivered by slice V6. -->

## Context

[BDR 0006](/bdr/0006-selection-mode-mouse-capture-toggle.md) (slice V3) specified a
key that toggles mouse capture off so the *terminal* selects text — which means the
app cannot draw any feedback. This BDR replaces that with app-managed selection: the
app keeps capture on, draws the highlight itself, and copies to the clipboard.
Delivered by slice V6 ([Issue 0021](/issues/0021-v6-app-managed-selection.md)) under
[ADR 0021](/adr/0021-app-managed-text-selection-clipboard.md), superseding BDR 0006.

## Behavior

```mermaid
sequenceDiagram
    participant U as User (mouse)
    participant M as model.update (pure)
    participant V as view
    participant Sh as shell (arboard)
    U->>M: LeftDown(cell)
    M->>M: selection = anchor..cursor
    U->>M: Drag(cell)
    M->>V: render highlight over selection
    U->>M: LeftUp
    M->>Sh: Cmd::CopyToClipboard(text)
    Sh->>V: footer "copied"
```

## Textual Description

In the **detail view**, mouse capture stays on. **Amended (reconciled with D1c): the
keyboard modifier is the discriminator between selecting and activating.** The unified
pointer model is:

- **Unmodified left button down** on the body starts a selection (anchor = cursor = cell).
- **Drag** (move with button held) extends the cursor; the selection spans anchor→
  cursor in reading order.
- While a selection exists, the selected cells render with a **reverse-video /
  highlighted background** — visible feedback as the operator drags.
- **Left button up** after a drag finalizes and emits `Cmd::CopyToClipboard(text)`; the
  footer shows a brief **copied** confirmation.
- A **plain (unmodified) click with no drag** is **not** a selection and — per D1c — does
  **not** open a body link. On the detail body it simply **clears any existing selection**
  (a no-op otherwise, leaving room for a future caret). On list screens the click keeps its
  existing drill-in semantics (those screens have no body links to gate).
- A **Ctrl/Cmd/Super+click** opens the link/asset under the pointer ([D1c](/adr/0020-body-links-inline-url-native-click.md) §2a); a **modified press does not start a
  selection** — activation and selection stay disjoint.
- Clipboard failure (headless/no display) degrades to a footer note; selection and
  highlight still work.

The `s` selection mode, the mouse-capture toggle, and the `selection_mode` indicator
from V3 are **removed**.

## Scenarios

**Scenario 1: drag highlights** — Given the body is visible, When the operator
presses and drags across text, Then the covered cells render highlighted.

**Scenario 2: release copies** — Given an active selection, When the button is
released, Then `Cmd::CopyToClipboard` is emitted with the selected text.

**Scenario 3: plain click is not selection and does not open** — Given a single
unmodified click with no movement on the detail body, When released, Then no copy
occurs, no link/asset opens (D1c), and any existing selection is cleared.

**Scenario 4: reading-order span** — Given a drag from a later cell back to an earlier
cell, When extended, Then the selection text is in reading order (anchor/cursor
normalized).

**Scenario 5: copy failure degrades** — Given the clipboard is unavailable, When a
selection is released, Then the app does not panic and shows a footer note; selection
state is unaffected.

**Scenario 6: old toggle gone** — Given the `s` key, When pressed, Then no
mouse-capture toggle occurs (the V3 behavior is removed).

**Scenario 7: modifier press does not select** — Given a Ctrl/Cmd/Super+left-button
down on the body, When pressed (and optionally dragged), Then no selection is started
(the modified gesture is reserved for D1c link activation), so selecting and activating
never collide.

## Test Design

Selection state transitions and text extraction are pure and unit-tested on
`update`; the clipboard write is asserted as an emitted `Cmd` (the effect itself is
the shell's, not unit-tested). Highlight rendering is asserted via the TestBackend
buffer. Each row names what it proves.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Drag selects | unit | 1 | model selection spans dragged cells | press/drag state |
| Highlight drawn | render (TestBackend) | 1 | selected cells styled reversed | drawn feedback |
| Release copies | unit | 2 | `Cmd::CopyToClipboard(text)` emitted | copy effect as data |
| Plain click clears, no open | unit | 3 | no copy Cmd; no OpenAsset; selection cleared | click vs drag, D1c gate |
| Reading order | unit | 4 | normalized anchor/cursor text | order normalization |
| Copy failure safe | unit/integration | 5 | no panic; footer note | graceful degrade |
| Toggle removed | unit | 6 | `s` emits no capture Cmd | V3 retired |
| Modifier press no select | unit | 7 | Ctrl/Cmd press starts no selection | select/activate disjoint |

## Related

- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md)
- BDR: [/bdr/0006-selection-mode-mouse-capture-toggle.md](/bdr/0006-selection-mode-mouse-capture-toggle.md) (superseded by this)
- Research: [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md)
- Issue: [/issues/0021-v6-app-managed-selection.md](/issues/0021-v6-app-managed-selection.md)
