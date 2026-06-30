---
type: ADR
title: The Detail screen's mutually-exclusive overlays become one typed DetailOverlay enum — making "composing and confirming-delete at once" unrepresentable
description: Screen::Detail today carries two independent Option fields — compose: Option<Compose> and confirm_delete: Option<i64> — for two mutually-exclusive overlay modes. Nothing structural prevents both being Some at once (an invalid state); the mutual exclusion is upheld only by convention and a single manual two-field clear in handle_comment_mutation_ok. Replace the two Options with one overlay: DetailOverlay { None, Compose(Compose), ConfirmDelete { comment_id } } field so the illegal combined state cannot be constructed, the key-routing guards and modal rendering read one field, and clearing the overlay is one assignment. focused_comment stays a separate orthogonal field.
status: Accepted
supersedes:
superseded_by:
tags: [tui, model, state-machine, refactor, make-illegal-states-unrepresentable, tea]
timestamp: 2026-06-30T00:00:00Z
---

# 0047. The Detail screen's overlays become one typed `DetailOverlay`

## Context

The Detail screen has two **mutually-exclusive overlay modes** over the read view:

- **comment compose** — multi-line text entry for a new comment or an edit
  ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md),
  [ADR 0036](/adr/0036-permission-aware-comment-targeting.md)), rendered in the reusable modal
  ([ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md)).
- **delete-confirm** — a Sim/Não prompt for a target comment
  ([ADR 0041](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md)), in the same
  modal.

Today they are encoded as **two independent `Option` fields** on `Screen::Detail`
(`src/tui/model.rs`):

```rust
compose: Option<Compose>,        // None when compose is inactive
confirm_delete: Option<i64>,     // Some(id) when the delete prompt is shown
```

`architecture.md` even calls this "the compose state machine (`Screen::Detail.compose`,
`confirm_delete`)". But two independent `Option`s encode **four** states, one of which is
illegal: `compose: Some(_)` **and** `confirm_delete: Some(_)` at the same time — composing a
comment while a delete prompt is open. Nothing structural forbids it:

- The key-routing guards (`src/tui/mod.rs`) check the two fields **independently**, with a
  priority order (`confirm_delete.is_some()` → confirm keys; else `compose.is_some()` → compose
  keys; else browse keys). The priority *masks* the conflict at the input layer but does not
  prevent the state.
- The modal renderer and the footer (ADR 0038/0039) each branch on `compose.is_some()` /
  `confirm_delete.is_some()` independently.
- The only place the two are coupled is `handle_comment_mutation_ok`, which manually clears
  **both** (`*compose = None; *confirm_delete = None;`) — a convention a reader must notice,
  not an invariant the type enforces.

The invalid state has not bitten in practice only because every handler that sets one happens
to leave the other `None`. That is a latent correctness hazard maintained by discipline, not by
the compiler — exactly the shape "make illegal states unrepresentable" removes.

## Decision

Replace the two independent `Option` fields with **one typed field**:

```rust
/// The active modal overlay on the Detail read view. Compose and the delete
/// prompt are mutually exclusive by construction — only one overlay at a time.
#[derive(Debug, Clone, PartialEq)]
pub enum DetailOverlay {
    None,
    Compose(Compose),
    ConfirmDelete { comment_id: i64 },
}
```

`Screen::Detail` carries `overlay: DetailOverlay` in place of `compose` and `confirm_delete`.
The combined state is now **unconstructible** — the enum holds at most one overlay.

`focused_comment: Option<usize>` is **not** folded in: it is an orthogonal navigation cursor
(which comment card is selected), legitimately `Some` alongside any overlay; it stays its own
field.

**Accessors keep call sites honest and terse** (on `DetailOverlay`):

- `fn compose(&self) -> Option<&Compose>` / `fn compose_mut(&mut self) -> Option<&mut Compose>`
- `fn confirm_delete_id(&self) -> Option<i64>`
- `fn is_compose(&self) -> bool` / `fn is_confirm(&self) -> bool`

The transitions map one-to-one onto the existing handlers, now writing one field:

- open compose / edit → `*overlay = DetailOverlay::Compose(Compose { … })`
- open delete prompt → `*overlay = DetailOverlay::ConfirmDelete { comment_id }`
- cancel compose, cancel delete, confirm delete, mutation success, fresh detail load →
  `*overlay = DetailOverlay::None` (one assignment; the two-field manual clear disappears)
- mutation error → `if let DetailOverlay::Compose(c) = overlay { c.status = Error(msg) }`

The key-routing guards become a single match each: `is_confirm()` then `is_compose()` then
browse. The modal renderer and footer read `overlay` (via the accessors) instead of two
`Option`s.

### Guard / fitness function

- **Behavior preserved — pure representation change.** The exact compose lifecycle (New/Edit,
  Editing/Submitting/Error), the delete-confirm flow (open → Sim confirms → Não/Esc cancels),
  the `confirm > compose > browse` key-routing priority, and the modal/footer rendering are all
  unchanged. Every existing buffer-derived and dispatch spec in `tests/unit/model.rs` stays
  green (rewritten only to construct/inspect `overlay` instead of the two `Option`s).
- **The illegal state is gone by construction.** No code path can hold an active compose and an
  active delete prompt simultaneously; the manual two-field clear in
  `handle_comment_mutation_ok` is replaced by a single `DetailOverlay::None`.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean; complexity
  within budget; mutation floor (Reviewer backstop): a routing or transition mutant must fail a
  test.

## Alternatives considered

- **Keep two `Option`s, add a debug-assert that not both are `Some`.** Rejected: a runtime
  assert is strictly weaker than making the state unrepresentable, adds a check to maintain, and
  still lets the illegal state exist between mutations. The type should carry the invariant.
- **Fold `focused_comment` into the enum too.** Rejected: focus is orthogonal — a user focuses a
  card *then* opens compose/confirm; the focus cursor is valid in every overlay state. Merging it
  would re-introduce the very product-of-states problem this ADR removes, in the other direction.
- **A boxed/`Option<DetailOverlay>` instead of a `None` variant.** Rejected: an explicit `None`
  variant reads better at the call sites (`overlay = DetailOverlay::None`) than
  `Option<DetailOverlay>` with its `Some(Compose(_))` double-wrap, and avoids a needless
  allocation decision; the Detail variant is already `#[allow(clippy::large_enum_variant)]`.

## Consequences

**Positive:** the Detail overlay state is one typed value with three legal states and no illegal
fourth. Clearing the overlay is one assignment; the key-routing guards and modal/footer reads go
through named accessors; the "remember to clear both fields" convention is deleted. The Model
shrinks by one field. New overlay modes (if ever added) extend the enum, with the compiler
forcing every match to handle them.

**Accepted trade-offs:** the rewrite touches ~32 `tests/unit/model.rs` construction/inspection
sites and the modal/footer/guard read sites — a wide but mechanical diff (two `Option`s → one
enum field + accessors). `architecture.md`'s state description and the modal/key-routing prose
are updated in the same change (maintenance rule).

## Related

- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md) (compose as a Detail mode; the `compose` field)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (edit reuses compose via ComposeKind::Edit)
- ADR: [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md) (footer branches on the overlay state)
- ADR: [/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md) (both overlays render in the modal)
- ADR: [/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md) (the delete-confirm prompt)
- Issue: [/issues/0051-detail-overlay-typed-state.md](/issues/0051-detail-overlay-typed-state.md)
