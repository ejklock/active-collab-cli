---
type: Issue
title: "Replace Screen::Detail's two independent overlay Options (compose, confirm_delete) with one typed DetailOverlay enum so the composing-and-confirming-delete state is unrepresentable"
description: Screen::Detail carries compose: Option<Compose> and confirm_delete: Option<i64> as two independent fields for two mutually-exclusive overlays; both can be Some at once (an illegal state) and mutual exclusion is upheld only by a manual two-field clear. Introduce enum DetailOverlay { None, Compose(Compose), ConfirmDelete { comment_id } } and a single overlay field, with accessors (compose/compose_mut/confirm_delete_id/is_compose/is_confirm). focused_comment stays separate. Pure representation change — key routing, compose lifecycle, delete-confirm flow, and modal/footer rendering are unchanged.
status: closed
labels: [tui, model, state-machine, refactor, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Detail overlay as one typed state (ADR 0047)

### Problem

`Screen::Detail` (`src/tui/model.rs:241-282`) holds two independent `Option` overlay fields:

- `compose: Option<Compose>` (line 263)
- `confirm_delete: Option<i64>` (line 273)

Two `Option`s encode four states; `compose: Some(_)` **and** `confirm_delete: Some(_)` together
is illegal (composing while a delete prompt is open). Nothing structural prevents it — the
key-routing guards (`src/tui/mod.rs`) check the fields independently with a `confirm > compose >
browse` priority, the modal/footer read each independently, and only
`handle_comment_mutation_ok` couples them via a manual `*compose = None; *confirm_delete = None;`.

### Decision (ADR 0047)

Replace the two `Option`s with one `overlay: DetailOverlay` field:

```rust
pub enum DetailOverlay { None, Compose(Compose), ConfirmDelete { comment_id: i64 } }
```

`focused_comment: Option<usize>` stays a separate orthogonal field.

### Scope

- `src/tui/model.rs`:
  - Add `enum DetailOverlay` (near `Compose`) with accessors `compose(&self) -> Option<&Compose>`,
    `compose_mut(&mut self) -> Option<&mut Compose>`, `confirm_delete_id(&self) -> Option<i64>`,
    `is_compose(&self) -> bool`, `is_confirm(&self) -> bool`.
  - Replace `compose` + `confirm_delete` on `Screen::Detail` with `overlay: DetailOverlay`.
  - Update the ~10 handlers (handle_loaded_detail, handle_compose_open, handle_compose_input,
    handle_compose_submit, handle_compose_cancel, handle_edit_comment_request,
    handle_delete_comment_request, handle_confirm_delete, handle_cancel_delete,
    handle_comment_mutation_ok, handle_comment_mutation_err) to read/write the one field. The
    two-field clear becomes `*overlay = DetailOverlay::None`; mutation error mutates the inner
    Compose via `compose_mut`.
- `src/tui/mod.rs`: the `compose_active` / `confirm_active` guards become `overlay.is_compose()`
  / `overlay.is_confirm()`; routing priority unchanged.
- `src/tui/view.rs` (and any modal/footer reader): read `overlay` via the accessors instead of
  the two `Option`s.
- `tests/unit/model.rs`: rewrite the ~32 construction/inspection sites to build/inspect
  `overlay: DetailOverlay::{None,Compose(..),ConfirmDelete{..}}` instead of the two `Option`s;
  assertion values are unchanged. If a Detail-screen test constructor/helper exists, route the
  change through it.

### Out of scope

- Folding `focused_comment` into the enum (orthogonal navigation cursor).
- Any change to compose lifecycle, delete-confirm flow, key-routing priority, or modal/footer
  behavior. Pure representation change.

### Acceptance criteria

- **AC1** (constraint, inspection): `DetailOverlay { None, Compose(Compose), ConfirmDelete {
  comment_id } }` exists with the five accessors; `Screen::Detail` has one `overlay` field and no
  longer has `compose` or `confirm_delete`; the simultaneous-compose-and-confirm state is
  structurally unconstructible. `focused_comment` remains a separate field.
- **AC2** (behavior, test): key-routing priority is preserved — the existing dispatch specs
  (confirm keys when confirming, compose keys when composing, browse keys otherwise) stay green
  via `is_confirm()`/`is_compose()`.
- **AC3** (behavior, test): compose behaviors unchanged — open New, edit pre-fill (ComposeKind::
  Edit), input/newline/backspace, submit (New→POST, Edit→PUT), cancel, and the
  Editing/Submitting/Error status transitions all stay green.
- **AC4** (behavior, test): delete-confirm behaviors unchanged — open prompt sets
  `ConfirmDelete`, Sim confirms (DELETE), Não/Esc cancels; mutation success clears the overlay in
  one assignment.
- **AC5** (behavior, test): footer + modal rendering derive from `overlay` (ADR 0038/0039) — the
  buffer-derived modal/footer specs stay green.
- **CC** (constraint, inspection): clean code — no banners/commented-out code; only non-obvious
  why-comments; comment-policy gate green.
- **CX** (constraint, command): complexity within budget — cyclomatic ≤ 10 (≤ 8 for new
  functions), cognitive ≤ 12 (quality-gate arborist). The accessors are trivial matches; folding
  two field-clears into one should not raise any handler's complexity.
- **TE** (constraint, command): tests assert observable behavior (routing, compose lifecycle,
  delete flow, rendering) and survive the mutation floor; swapping a routing branch or a
  transition target must fail a test.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0047-detail-overlay-as-one-typed-state.md](/adr/0047-detail-overlay-as-one-typed-state.md)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md)
- ADR: [/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md)
