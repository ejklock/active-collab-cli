---
type: Issue
title: "Migrate the delete-confirm to the reusable modal (buttons + Enter/Esc), out of the comment card"
description: Slice 2 of the comment modal. build_detail_content stops rendering the inline [confirmar]/[cancelar] tokens in the comment card; view() renders a confirm modal via the slice-1 primitive when confirm_delete is Some(id) — title Excluir comentário?, body, and [confirmar]/[cancelar] buttons registered as modal click targets (reusing the Confirm/Cancel handlers). The confirm modal also accepts keys: Enter confirms, Esc cancels. Proves the modal primitive's reuse (second adapter = real seam).
status: open
labels: [tui, modal, overlay, confirm, delete, slice]
blocked_by: [0037]
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Delete-confirm via the reusable modal

Slice 2 of the comment modal. Implements
[BDR 0026](/bdr/0026-comment-modal-overlay.md) Scenarios 7, 8 under
[ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md), amending the
rendering of [ADR 0036](/adr/0036-permission-aware-comment-targeting.md). Reuses the modal
primitive from [issue 0037](/issues/0037-modal-primitive-and-compose.md) — the second
adapter that proves the seam is real.

### Problem

The delete-confirm renders as `[confirmar]`/`[cancelar]` tokens **inside the focused
comment's card** (`build_detail_content` with `confirm_delete`), so it scrolls with the
thread and is mouse-only. It should be a centered modal like the compose, and reachable by
keyboard.

### Decision (from ADR)

- **Confirm via modal (ADR 0039 §4):** `build_detail_content` stops rendering the inline
  confirm tokens. `view()` renders a confirm modal when `confirm_delete == Some(id)`:
  title `Excluir comentário?`, a short body, and `[confirmar]`/`[cancelar]` **buttons**
  registered as modal click targets (modal-relative coordinates). The
  `AffordanceKind::Confirm`/`Cancel` → `handle_confirm_delete`/`handle_cancel_delete` path
  is reused, keyed off the modal buttons.
- **Keyboard path:** the confirm modal accepts **Enter (confirm)** and **Esc (cancel)**,
  closing the prior mouse-only gap (matches the ADR 0038 footer hint).

### Scope

Included:

- `src/tui/view.rs` — render the confirm modal via `render_modal` (slice-1 primitive) when
  `confirm_delete.is_some()`; register the `[confirmar]`/`[cancelar]` button click targets.
- `src/render.rs` — `build_detail_content` stops rendering the inline confirm prompt;
  remove the confirm-token path from the comment card builder.
- `src/tui/model.rs` — the confirm hit-test maps a modal-button click to `Confirm`/`Cancel`
  (modal-relative, not scroll-aware); reuse `handle_confirm_delete`/`handle_cancel_delete`.
- `src/tui/events.rs` — when `confirm_delete.is_some()`, Enter → confirm, Esc → cancel
  (a confirm key sub-mode that does not collide with browse keys).
- Tests: `tests/unit/tui_render.rs`, `tests/unit/model.rs`.

Excluded: the compose modal (issue 0037, primitive reused); any change to the server
delete contract (ADR 0033/0035 unchanged).

### Acceptance

- AC1 — render (`TestBackend`): `confirm_delete=Some(id)` renders the confirm modal
  (title `Excluir comentário?`, `[confirmar]`/`[cancelar]` buttons) over the dimmed thread,
  via the slice-1 `render_modal` primitive.
- AC2 — render/unit: the comment card no longer contains the inline `[confirmar]`/
  `[cancelar]` tokens (removed from `build_detail_content`).
- AC3 — unit + render: a click on the `[confirmar]` button span emits the Confirm path
  (`Cmd::DeleteComment`); a click on `[cancelar]` emits Cancel (clears `confirm_delete`,
  no write).
- AC4 — `update()`: with `confirm_delete.is_some()`, Enter emits the Confirm path and Esc
  emits Cancel; neither acts when no confirm is pending.
- AC5 — regression: the delete still refreshes the thread on success (`CommentMutationOk`
  path unchanged); `[excluir]` still opens the confirm (now as a modal).
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  helpers) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ 12) (`verify_by: command`).
- TE — tests assert observable behavior (buffer-derived modal + button hit-test +
  `update()` transitions) and survive the mutation floor on changed lines (`verify_by: command`).

### Plan

1. `render.rs`: remove the inline confirm-token rendering from `build_detail_content` /
   the comment card builder (drop the `confirm_delete` special-case in the card).
2. `view.rs`: render the confirm modal via `render_modal` when `confirm_delete.is_some()`;
   register the `[confirmar]`/`[cancelar]` button click targets in modal coordinates.
3. `model.rs`: map a modal-button click to `Confirm`/`Cancel` (reuse the handlers).
4. `events.rs`: confirm sub-mode keys — Enter → confirm, Esc → cancel.
5. Tests: confirm modal render (no inline tokens); button click → Confirm/Cancel;
   Enter/Esc keys; delete-refresh regression.

Observable end-to-end: click `[excluir]` on your own comment and a centered
`Excluir comentário?` modal opens over a dimmed thread; press Enter (or click
`[confirmar]`) and the comment is deleted and the thread reloads.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
