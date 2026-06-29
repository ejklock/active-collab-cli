---
type: ADR
title: A reusable centered modal overlay (dimmed backdrop) renders the comment compose and the delete-confirm, replacing their inline-spliced rendering
description: The comment compose field and the delete-confirm prompt are spliced into the Detail screen's scrollable `lines`, so they scroll with the thread, get pushed around, and have no focus framing. Introduce a reusable modal primitive in a new src/tui/widgets/modal.rs — a pure centered-Rect layout + a render helper that dims the backdrop (Modifier::DIM over the content cells), Clears the modal Rect, and draws a bordered box with title, body, and an in-box hint/status line. Migrate BOTH the compose mode and the delete-confirm to it (two adapters = a real seam). Amends the rendering of ADR 0034 (compose stays a mode; its rendering moves out of the scrollable lines), ADR 0036 (confirm stays click-driven and gains Enter/Esc; its rendering moves out of the comment card), and ADR 0038 (while a modal is open, the compose/confirm hint + transient status render inside the modal, not the footer).
status: Accepted
supersedes:
superseded_by:
tags: [tui, modal, overlay, comments, compose, confirm, render, widget]
timestamp: 2026-06-28T00:00:00Z
---

# 0039. A reusable centered modal overlay for compose and confirm

## Context

Two transient interactions on the Detail screen render **inline, inside the single
globally-scrollable `Vec<line>`**:

- **Compose** (new/edit comment): `reflow_detail` (`src/tui/model.rs`) appends
  `compose_block_lines` (a `── Comment ──` label + the buffer) to the end of `lines`/
  `line_styles` whenever `compose.is_some()` ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md)).
- **Delete-confirm**: `build_detail_content` (`src/render.rs`) renders a
  `[confirmar]`/`[cancelar]` prompt **as tokens inside the focused comment's card** when
  `confirm_delete == Some(id)` ([ADR 0036](/adr/0036-permission-aware-comment-targeting.md)).

Because both are spliced into the scrollable content, they **scroll with the thread, are
pushed around by the comments above them, and carry no focus framing** — the compose
field can even be scrolled off screen while typing. The user named this directly: *"não
gostei da forma que é aberto o campo para comentários… melhor criarmos um componente
reutilizável de modal."*

The view layer already proves an **overlay** is feasible: `view()` (`src/tui/view.rs`)
draws the drag-selection highlight *on top of* the content (`draw_selection_highlight`)
after `draw_detail`. ratatui supports the rest natively: a centered `Rect`, the `Clear`
widget to punch a hole in the content, and a per-cell `Modifier::DIM` pass over the
backdrop. What is missing is a **reusable modal primitive** — there is no widget the two
(and future) transient interactions can share.

The user locked the design: **a reusable modal**, used by **both the compose and the
delete-confirm**, rendered **centered with a dimmed backdrop**.

## Decision

Introduce a **reusable modal overlay primitive** and render the compose and the
delete-confirm through it, removing both from the scrollable content.

### 1. A new `widgets` module home

Add `src/tui/widgets/modal.rs` — the first member of a new `src/tui/widgets/` module for
**cross-screen view primitives** (a modal is not a screen, so it does not belong under
`screens/`). The module is pure-layout + render, with colors sourced from `theme.rs`.

### 2. The primitive: pure centered layout + a render helper

- **`modal_area(frame_area: Rect, desired_w: u16, desired_h: u16) -> Rect`** — a **pure**
  function that centers a box of the desired size in the frame, clamped to fit (with a
  margin) so it never exceeds the terminal. Unit-tested headlessly against fixed frame
  sizes — this is the modal's testable layout seam.
- **Target size ≈ 70 % of the frame** (refined 2026-06-29 on user feedback — the modal must
  be *bigger*, not content-hugging). `render_modal` sizes the box to ≈ 70 % of the frame
  width **and** height, with a content-driven minimum (it never shrinks below what the body
  + hint need), then centers + clamps it via `modal_area`. The earlier behavior (full-width,
  height hugging the content) read as a low inline strip — this makes the modal a clearly
  dominant centered panel.
- **`render_modal(frame, frame_area, ModalContent)`** — the render helper that, in order:
  1. **strongly dims the backdrop** ("DIM mais forte", refined 2026-06-29 — the thread must
     *not* read as transparent) — over every cell of `frame_area` (via `frame.buffer_mut()`)
     it applies `Modifier::DIM` **and** resets the cell to a dark backdrop background
     (a `theme.rs` style), so the thread reads as clearly *behind* the modal — still faintly
     perceptible, not painted fully opaque;
  2. sizes the box to the ≈ 70 % target, computes `modal_area` to center + clamp it, and
     **`Clear`s** it (so the box is opaque over the dimmed content);
  3. draws the **bordered box** (a `theme.rs` modal style; rounded border to match the
     comment cards) with a **title**, the **body lines**, and an optional **bottom
     hint/status line** inside the box;
  4. returns the modal `Rect` (and any button spans) so the caller can **register click
     targets** in modal-relative coordinates.

`ModalContent` is a small struct: `title`, `body: Vec<(String, Vec<StyleRun>)>` (or the
existing line+style channel), an optional `hint`, an optional `status`, and an optional
set of **buttons** (label + a `ModalButton` id) for the confirm case.

### 3. Compose renders through the modal (amends ADR 0034 rendering)

`reflow_detail` **stops appending `compose_block_lines`** to `lines`; the scrollable
content no longer contains the compose field. Instead, when `compose.is_some()`, `view()`
renders a compose modal **after** `draw_detail`: title `Novo comentário` (or
`Editar comentário` for `ComposeKind::Edit`), body = the buffer (multi-line), and an
**in-box bottom line** carrying the controls hint (`Ctrl+S enviar · Esc cancelar`) and the
transient status (`Enviando…` / the localized error). The compose **mode** and its
mode-aware key map (Ctrl+S/Esc/Enter/Backspace/char) are **unchanged** — only the
rendering moves. `compose_block_lines` is repurposed into the modal body builder.

### 4. Delete-confirm renders through the modal (amends ADR 0036 rendering)

`build_detail_content` **stops rendering the inline `[confirmar]`/`[cancelar]` tokens** in
the comment card. When `confirm_delete == Some(id)`, `view()` renders a confirm modal:
title `Excluir comentário?`, a short body, and **two buttons** `[confirmar]`/`[cancelar]`
registered as modal click targets (the `AffordanceKind::Confirm`/`Cancel` →
`handle_confirm_delete`/`handle_cancel_delete` path is reused, now keyed off the modal
buttons instead of in-card tokens). The confirm modal **also accepts keys**: **Enter
confirms, Esc cancels** — closing the prior mouse-only gap (and matching the footer hint
`Enter/clique confirmar · Esc cancelar` from ADR 0038).

### 5. Footer interaction while a modal is open (amends ADR 0038)

While a modal is open the **modal owns its hint + transient status** (rendered in the box,
per the user's chosen layout). The main footer therefore **does not** also show the
compose/confirm contextual hint or the compose status for the modal case — it shows the
browse hint behind the dimmed backdrop. The footer's `Copiado ✓` clipboard status (a
browse/selection-time event) is unaffected. This keeps **one home** for the compose
hint/status: inside the modal while open.

### Guard / fitness function

- **Centered layout (unit, pure):** `modal_area` centers and clamps for representative
  frame sizes (small, large, narrower-than-desired) — the box stays within the frame with
  the margin; a desired size larger than the frame clamps, never overflows.
- **Size ≈ 70 % of the frame (unit/render):** for a representative large frame the rendered
  modal occupies ≈ 70 % of the width and height (within a tolerance), never the full frame
  and never a thin content-hugging strip.
- **Strong backdrop (render, cell-derived):** a backdrop cell carries `Modifier::DIM` **and**
  the dark backdrop background — proving the thread is strongly dimmed, not merely
  fg-dimmed.
- **Compose not in scroll content (unit):** after `reflow_detail` with `compose.is_some()`,
  the scrollable `lines` do **not** contain the compose label/buffer (it moved to the
  overlay) — inverts the prior inline assertion.
- **Compose modal render (buffer-derived):** a `TestBackend` render with an open compose
  shows the centered titled box over the dimmed content; the buffer text and the in-box
  hint/status appear inside the box; the backdrop cells carry the DIM modifier.
- **Confirm modal render + buttons (buffer-derived):** a `TestBackend` render with
  `confirm_delete = Some(id)` shows the confirm modal with `[confirmar]`/`[cancelar]`;
  a click on each registered button span emits `Confirm`/`Cancel`; Enter emits Confirm,
  Esc emits Cancel.
- **No inline confirm (unit/render):** the comment card no longer contains the
  `[confirmar]`/`[cancelar]` tokens.
- **Reuse (deletion test):** the compose and confirm both call the **same** `render_modal`
  / `modal_area`; deleting the primitive would force re-implementing the centered-dim-Clear
  dance in two places — a real seam, not a shallow wrapper.

## Alternatives considered

- **Keep inline, just stop it scrolling** (pin the compose block to the bottom of the
  content area). Rejected: it still has no focus framing, competes with the thread, and
  does nothing for the confirm; the user explicitly asked for a modal component.
- **Solid (opaque) backdrop instead of DIM.** Rejected: painting a fully opaque panel over
  the content hides the thread entirely; the user chose "fundo escurecido" (dimmed, not
  hidden). The 2026-06-29 refinement keeps DIM but **strengthens** it (DIM + a dark backdrop
  background) so the thread no longer reads as transparent — the middle ground between a
  weak fg-only dim and a fully opaque fill.
- **Put the modal under `screens/`** (next to `asset_panel.rs`). Rejected: a modal is a
  cross-screen primitive, not a screen; a dedicated `widgets/` home keeps the seam honest
  and gives future reusable primitives (e.g. a toast) a place to live.
- **A full generic modal stack / z-index manager.** Rejected as over-engineering for two
  call sites: a single active modal derived from `compose` / `confirm_delete` covers every
  current need; a stack can come if a third, layerable modal ever appears (YAGNI).
- **Move `panel_box_rich` into the new widget now.** Deferred: the modal draws its own box
  via ratatui `Block`; relocating the string-based `panel_box_rich` is a separate refactor
  out of this feature's scope.

## Consequences

**Positive:** the compose field and the delete-confirm become focus-framed centered
overlays over a dimmed thread — the interaction the user asked for. The two share **one**
reusable primitive (a real, deletion-test-passing seam), so a future transient dialog has
a ready home. The confirm gains a keyboard path (Enter/Esc). The scrollable content gets
simpler — `reflow_detail` no longer appends compose lines, and `build_detail_content` no
longer special-cases the confirm prompt.

**Accepted trade-offs:** a new `widgets/` module and a modal primitive are added; `view()`
gains an overlay branch and modal-relative click-target registration (the compose/confirm
hit-tests move from scroll-aware content coordinates to modal-relative coordinates — a net
simplification, since modal coordinates are not scrolled). The compose hint/status move
from the footer (ADR 0038) into the modal for the open-modal case, so ADR 0038's
compose/confirm footer branches are partially superseded (documented here). The DIM
backdrop pass iterates the content area's cells each frame a modal is open — bounded by the
viewport, negligible.

## Related

- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md) (compose stays a mode; its rendering moves here)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (confirm stays click-driven, gains Enter/Esc; rendering moves here)
- ADR: [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md) (compose hint/status move into the modal while open)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (the scrollable content the compose/confirm leave)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md) (the existing overlay precedent — selection highlight drawn over content)
- BDR: [/bdr/0026-comment-modal-overlay.md](/bdr/0026-comment-modal-overlay.md)
- Issues: [/issues/0037-modal-primitive-and-compose.md](/issues/0037-modal-primitive-and-compose.md), [/issues/0038-confirm-delete-modal.md](/issues/0038-confirm-delete-modal.md)
