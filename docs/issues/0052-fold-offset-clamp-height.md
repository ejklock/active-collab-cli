---
type: Issue
title: "Single-home the detail offset-clamp height in detail_geometry::content_height_clamped — the ADR 0045 deferred fold"
description: The clamped body-viewport height (viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1) is written inline in two model.rs functions (detail_max_offset, scroll_offset_for_card). ADR 0045 single-homed the row→line geometry in src/tui/detail_geometry.rs but explicitly deferred this distinct quantity. Add detail_geometry::content_height_clamped(viewport_rows) -> usize = (content_height(viewport_rows) as usize).max(1) and route both call sites through it, so the clamp height lives once next to content_height. Behavior unchanged.
status: closed
labels: [tui, geometry, viewport, refactor, locality, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Fold the detail offset-clamp height into detail_geometry (ADR 0045 deferral)

### Problem

The scroll math computes the body-text viewport height — clamped to a minimum of one row —
inline in two places in `src/tui/model.rs`:

- `detail_max_offset` (`model.rs:397`):
  `let text_viewport_height = (viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1);`
- `scroll_offset_for_card` (`model.rs:1035`):
  `let text_vh = (viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1);`

This is `(content_height(viewport_rows) as usize).max(1)` — `detail_geometry::content_height`
already owns the `viewport_rows.saturating_sub(DETAIL_CHROME_ROWS)` half.
[ADR 0045](/adr/0045-detail-viewport-geometry-module.md) single-homed the detail viewport↔line
geometry in `src/tui/detail_geometry.rs` but **explicitly deferred** this quantity:

> *Folding the offset-clamp height (`model.rs:355`, `:993`,
> `(viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1)`) — a distinct quantity,
> deferred per ADR 0045.*

It is distinct from the row→line mapping (`content_height` can be 0; the clamp floors at 1 so a
degenerate viewport does not break the offset arithmetic), but it is still detail viewport
geometry and belongs in the same module as `content_height`, not copied across two model.rs
functions.

### Decision

Execute ADR 0045's deferred fold (no new architectural decision): add
`detail_geometry::content_height_clamped(viewport_rows: u16) -> usize` returning
`(content_height(viewport_rows) as usize).max(1)` and route both `model.rs` call sites through
it. The `.max(1)` floor — the "distinct quantity" — is now named and documented in one place.

### Scope

- `src/tui/detail_geometry.rs`: add
  `pub(crate) fn content_height_clamped(viewport_rows: u16) -> usize` =
  `(content_height(viewport_rows) as usize).max(1)`, with a doc-comment stating why the floor is
  1 (a zero-height body viewport would make the scroll/offset arithmetic — `viewport_end`,
  `lines_len.saturating_sub(height)` — degenerate; the floor keeps model-only tests at
  viewport=(0,0) consistent with render behavior).
- `src/tui/model.rs`: `detail_max_offset` and `scroll_offset_for_card` call
  `detail_geometry::content_height_clamped(viewport_rows)` instead of the inline expression. No
  other change.
- `tests/unit/detail_geometry.rs`: add a unit spec for `content_height_clamped` from primitives —
  viewport smaller than the chrome → 1 (the floor), a viewport with N body rows → N, and the
  exact-chrome boundary.

### Out of scope

- Any change to scroll, offset-clamp, or card-scroll behavior. Pure single-homing.
- Touching `content_height` / `is_in_content` / `row_to_line_idx` (already single-homed by
  ADR 0045).

### Acceptance criteria

- **AC1** (constraint, inspection): `detail_geometry::content_height_clamped(viewport_rows) ->
  usize` exists (= `(content_height(viewport_rows) as usize).max(1)`); both `detail_max_offset`
  and `scroll_offset_for_card` call it; no
  `(viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1)` literal remains in
  `model.rs`.
- **AC2** (behavior, test): scroll/offset behavior is unchanged — the existing `detail_max_offset`,
  `scroll_offset_for_card`, and detail-scroll specs stay green.
- **AC3** (constraint, test): `content_height_clamped` is unit-tested from primitives — the `max(1)`
  floor when `viewport_rows <= DETAIL_CHROME_ROWS`, the N-row value above the chrome, and the
  exact-chrome boundary.
- **CC** (constraint, inspection): clean code — no banners/commented-out code; only non-obvious
  why-comments; comment-policy gate green.
- **CX** (constraint, command): complexity within budget — cyclomatic ≤ 10 (≤ 8 for the new
  function); the fold removes a duplicated literal and cannot raise any function's complexity.
- **TE** (constraint, command): tests assert observable behavior and survive the mutation floor —
  dropping the `.max(1)` floor or the chrome subtraction must fail a `content_height_clamped` spec.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0045-detail-viewport-geometry-module.md](/adr/0045-detail-viewport-geometry-module.md) (single-homed the geometry; deferred this fold)
