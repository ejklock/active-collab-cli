---
type: Issue
title: "Reusable modal primitive + migrate the comment compose to it (centered overlay, dimmed backdrop)"
description: Slice 1 of the comment modal. Add src/tui/widgets/modal.rs — a pure modal_area(frame, w, h) -> centered/clamped Rect + a render_modal helper that dims the backdrop (Modifier::DIM over the content cells), Clears the modal Rect, and draws a bordered box with title/body/in-box hint+status. Migrate the compose mode: reflow_detail stops appending compose_block_lines to the scrollable lines; view() renders a compose modal when compose.is_some() (title Novo/Editar comentário, the buffer, in-box Ctrl+S/Esc hint + Enviando…/error status). Compose key map + semantics unchanged.
status: open
labels: [tui, modal, overlay, compose, widget, slice]
blocked_by:
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Reusable modal primitive + compose migration

Slice 1 of the comment modal. Implements
[BDR 0026](/bdr/0026-comment-modal-overlay.md) Scenarios 1–6, 9, 10 under
[ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md), amending the
rendering of [ADR 0034](/adr/0034-comment-compose-mode-multiline.md) and
[ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md).

### Problem

The compose field is appended to the Detail screen's scrollable `lines` (`reflow_detail`
→ `compose_block_lines`), so it scrolls with the thread and has no focus framing. The user
asked for a reusable modal component instead.

### Decision (from ADR)

- **New primitive (ADR 0039 §1–2):** `src/tui/widgets/modal.rs` — pure
  `modal_area(frame_area, desired_w, desired_h) -> Rect` (centered + clamped) and
  `render_modal(frame, frame_area, ModalContent)` that (1) applies `Modifier::DIM` to the
  backdrop cells via `frame.buffer_mut()`, (2) `Clear`s the modal Rect, (3) draws the
  bordered box (theme.rs style) with title + body + an optional in-box hint/status line,
  (4) returns the modal Rect + any button spans for click-target registration.
- **Compose via modal (ADR 0039 §3):** `reflow_detail` no longer appends
  `compose_block_lines` to `lines`. `view()` renders a compose modal when
  `compose.is_some()`: title `Novo comentário` (or `Editar comentário` for
  `ComposeKind::Edit`), body = the multi-line buffer, in-box bottom line = the controls
  hint (`Ctrl+S enviar · Esc cancelar`) + the transient status (`Enviando…` / localized
  error). `compose_block_lines` becomes the modal body builder.
- **Footer one-home (ADR 0039 §5):** while the compose modal is open, the footer does not
  also render the compose hint/status (the modal owns them).

### Scope

Included:

- `src/tui/widgets/modal.rs` (new) + `src/tui/widgets/mod.rs` (new) + `mod widgets;` wiring
  in `src/tui/mod.rs`.
- `src/tui/view.rs` — render the compose modal overlay after `draw_detail`; suppress the
  footer compose hint/status while the modal is open.
- `src/render.rs` — repurpose `compose_block_lines` into the modal body builder.
- `src/tui/model.rs` — `reflow_detail` stops appending the compose lines.
- `src/tui/theme.rs` — modal box style + the dim/backdrop handling (named styles).
- Tests: `tests/unit/tui_render.rs`, `tests/unit/model.rs`.

Excluded: the delete-confirm modal (issue 0038); a text cursor glyph in the buffer
(out of scope); relocating `panel_box_rich` (separate refactor).

### Acceptance

- AC1 — `modal_area` (unit, pure): centers a desired box within a large frame (with
  margin); clamps to fit (no overflow) when the frame is narrower/shorter than desired.
- AC2 — render (`TestBackend`): with `compose.is_some()`, a centered titled box renders
  over the content; the backdrop cells carry `Modifier::DIM`; the buffer and the in-box
  hint render inside the box.
- AC3 — unit + render: after `reflow_detail` with `compose.is_some()`, the scrollable
  `lines` do **not** contain the compose label/buffer (it moved to the overlay).
- AC4 — unit + render: `ComposeSubmit` sets `Submitting` and the modal status line shows
  `Enviando…`; `CommentMutationOk` closes the modal and refreshes; `CommentMutationErr`
  keeps the buffer and shows the localized error in the modal.
- AC5 — render (`TestBackend`): `ComposeOpen(Edit{id})` renders the modal titled
  `Editar comentário` pre-filled with the comment body; `New` renders `Novo comentário`.
- AC6 — render (`TestBackend`): with the compose modal open, the footer does **not** render
  the compose hint/status (one home — the modal).
- AC7 — regression: compose semantics unchanged — Enter inserts a newline (not submit),
  Esc cancels/discards, Ctrl+S submits (`update()` assertions stay green).
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  helpers) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ 12) (`verify_by: command`).
- TE — tests assert observable behavior (buffer-derived modal geometry + `update()`
  transitions) and survive the mutation floor on changed lines (`verify_by: command`).

### Plan

1. Create `src/tui/widgets/{mod.rs,modal.rs}`; wire `mod widgets;`. Implement
   `modal_area` (pure) + `render_modal` (DIM backdrop → `Clear` → bordered box with
   title/body/hint+status → return Rect + button spans).
2. `theme.rs`: modal box + backdrop styles.
3. `render.rs`: repurpose `compose_block_lines` into the modal body builder.
4. `model.rs`: `reflow_detail` stops appending compose lines.
5. `view.rs`: render the compose modal after `draw_detail` when `compose.is_some()`;
   suppress the footer compose hint/status while open.
6. Tests: `modal_area` centering/clamp; compose modal render (DIM backdrop, title, buffer,
   in-box hint/status); compose-absent-from-scroll; submit-status-in-modal; edit pre-fill
   title; footer-one-home; compose-semantics regression.

Observable end-to-end: press `c` on a task and a centered `Novo comentário` modal opens
over a dimmed thread; type, Ctrl+S, watch `Enviando…` in the box, and the thread reloads.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
