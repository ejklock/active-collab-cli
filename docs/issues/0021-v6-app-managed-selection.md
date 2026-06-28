---
type: Issue
title: "V6 — app-managed text selection: drag to highlight, copy to clipboard with feedback"
description: Replace the V3 mouse-capture toggle with app-managed selection — keep capture on, track press/drag/release, draw a reverse-video highlight over the selection, and copy it to the system clipboard via arboard with a footer confirmation. Supersedes ADR 0012 / BDR 0006.
status: closed
labels: [tui, ux, mouse, selection, clipboard]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## V6 — app-managed text selection

Implements [ADR 0021](/adr/0021-app-managed-text-selection-clipboard.md), observable
behavior pinned by [BDR 0015](/bdr/0015-app-managed-text-selection.md). Supersedes the
V3 selection mode ([ADR 0012](/adr/0012-mouse-capture-toggle-for-text-selection.md) /
[BDR 0006](/bdr/0006-selection-mode-mouse-capture-toggle.md)). Traces to research
[/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md).

### Problem

V3 toggles mouse capture off so the terminal selects text — which means the app cannot
draw feedback. The operator explicitly needs to *see* what is selected. With capture off
that is structurally impossible.

### Decision

Keep mouse capture on; the app owns selection. `Model` gains `selection: Option<Selection>`
mutated by mouse `Msg`s (down=anchor, drag=extend, up=finalize). The view draws a
reverse-video highlight over selected cells. On release, emit `Cmd::CopyToClipboard(text)`
interpreted by the shell via **arboard**; footer shows a copied confirmation. A click with
no drag keeps existing click semantics. Remove the `s` toggle, `Cmd::SetMouseCapture`, and
`selection_mode`.

### Scope

Included: `Cargo.toml` (add `arboard`), `src/tui/model.rs` (selection state + transitions
+ text extraction; remove `selection_mode`/`SetMouseCapture`), `src/tui/view.rs` (highlight
pass + copied footer; drop the selection-mode indicator), `src/tui/mod.rs` (interpret
`Cmd::CopyToClipboard` via arboard; drop the capture-toggle arm), and the affected
`tests/unit/*` + a TestBackend highlight test. Excluded: rich-text (R4), links (V5);
non-detail screens' click behavior (unchanged).

### Acceptance

- Press+drag sets a selection spanning the dragged cells; selected cells render
  reverse-video (BDR 0015 Sc. 1); highlight asserted via TestBackend.
- Release emits `Cmd::CopyToClipboard(text)` with reading-order-normalized text
  (Sc. 2, 4).
- A plain (unmodified) click with no movement does not copy, does not open a link (D1c),
  and clears any existing selection (Sc. 3). Selection only starts on an **unmodified**
  press; a Ctrl/Cmd/Super press starts no selection — reserved for D1c activation (Sc. 7).
- Clipboard failure degrades to a footer note, no panic (Sc. 5); `s` emits no capture
  Cmd — V3 removed (Sc. 6).
- Full suite green; clippy `-D warnings`, fmt, comment-policy clean; complexity within
  budget; selection transitions + copy-Cmd tests are mutation-resistant.

### Plan

Single slice (V6). 1) Add `arboard` to `Cargo.toml`. 2) Add `Selection` + state machine
to `model.rs`, remove `selection_mode`/`SetMouseCapture`. 3) Add `Cmd::CopyToClipboard`
+ text extraction from rendered lines. 4) Highlight pass + copied footer in `view.rs`;
drop the indicator. 5) Interpret the Cmd via arboard in `mod.rs`; drop the toggle arm.
6) Replace V3 tests; add selection/highlight/copy tests.
