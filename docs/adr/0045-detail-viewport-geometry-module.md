---
type: ADR
title: Detail viewport↔content geometry is one pure module — the row→line_idx mapping and text_top live once, shared by hit-test, selection, and copy
description: ADR 0044 single-homed the click-resolution coordinate translation in hit_test, but the same detail viewport→line_idx mapping (the magic literal text_top=2, content_height = viewport_rows - DETAIL_CHROME_ROWS, the in-content bounds check, line_idx = offset + (row - text_top)) is still copied in is_in_body_area and extract_selected_text (the V6 selection/copy paths). Extract a pure src/tui/detail_geometry.rs that owns this mapping; hit_test, is_in_body_area, and extract_selected_text all consume it, so the geometry lives once and text_top stops being a scattered magic number.
status: Accepted
supersedes:
superseded_by:
tags: [tui, geometry, viewport, refactor, locality, depth, ratatui]
timestamp: 2026-06-30T00:00:00Z
---

# 0045. Detail viewport↔content geometry is one pure module

## Context

[ADR 0044](/adr/0044-detail-click-resolution-as-hit-test-module.md) collapsed detail click
resolution into `src/tui/hit_test.rs`, single-homing the click path's viewport→`line_idx`
translation in `viewport_to_line_col`. But that translation is one instance of a more general
fact — **how a terminal row maps to a content line in the scrollable Detail body** — and the
same arithmetic is still copied across the screen's other row-consuming paths:

- `hit_test::viewport_to_line_col` (`hit_test.rs:29-36`) — `text_top = 2`,
  `content_text_height = viewport_rows.saturating_sub(DETAIL_CHROME_ROWS)`, the
  `row < text_top || row >= text_top + content_text_height` bounds check, and
  `line_idx = offset + (row - text_top)`.
- `is_in_body_area` (`model.rs:1125`) — the **same** `text_top = 2` + `content_text_height`
  formula, as the in-content predicate `row >= text_top && row < text_top + content_text_height`.
- `extract_selected_text` (`model.rs:1194`) — the **same** formula again, looped over the
  selected rows: the bounds `continue` and `line_idx = offset + (vp_row - text_top)` (the V6
  copy path, [ADR 0021](/adr/0021-app-managed-text-selection-clipboard.md)).

So `text_top = 2` is a **magic literal written in three places**, and the row→`line_idx`
mapping has three copies kept in agreement only by convention. The ADR 0043 review and obs 53
flagged the two non-click copies as the remaining drift risk after ADR 0044: a change to the
detail chrome height or the content's top row must be made — correctly — in three functions
across two modules, or hit-testing, selection, and copy silently disagree about which content
line a click/drag landed on.

## Decision

Extract a **pure** module `src/tui/detail_geometry.rs` that owns the detail content
viewport↔line mapping, and have every row-consuming path derive from it.

1. **One home for the constant and the formulas.** The module declares the content's top row
   once (`DETAIL_TEXT_TOP = 2`, replacing the scattered literal) and exposes:
   - `content_height(viewport_rows: u16) -> u16` — `viewport_rows.saturating_sub(DETAIL_CHROME_ROWS)`.
   - `is_in_content(viewport_rows: u16, row: u16) -> bool` — the in-content predicate.
   - `row_to_line_idx(offset: usize, viewport_rows: u16, row: u16) -> Option<usize>` — the
     viewport-bounded mapping: `None` when `row` is outside the content viewport, else
     `offset + (row - DETAIL_TEXT_TOP)`. (The `lines.len()` guard stays with the caller that
     holds `lines`, applied via `?`/`lines.get`.)

2. **Pure, primitive-typed, no `Model` dependency.** The functions take `offset`,
   `viewport_rows`, `row` — not `&Model` — so the interface is the test surface: feed the
   geometry primitives, assert the mapping/predicate, with no screen state to construct.

3. **All three consumers derive from it.** `hit_test::viewport_to_line_col` becomes
   `row_to_line_idx(...)?` then its `lines.len()` guard; `is_in_body_area` keeps its
   `Screen::Detail` guard and delegates to `is_in_content`; `extract_selected_text` replaces
   its per-row bounds+mapping with `row_to_line_idx` + `lines.get`. No `text_top`/
   `content_text_height` arithmetic remains in `model.rs`'s body/selection paths or in
   `hit_test`.

### Guard / fitness function

- **Behavior preserved — invisible to the user.** Click resolution, the in-body predicate,
  text selection, and clipboard copy are unchanged; all existing buffer-derived click and
  V6 selection/copy specs stay green.
- **One mapping, one constant.** `DETAIL_TEXT_TOP` and the row→`line_idx` mapping exist in
  exactly one place; grep finds no remaining `let text_top: u16 = 2` in `model.rs`/`hit_test.rs`.
- **The interface is the test surface.** `detail_geometry` unit tests assert
  `is_in_content` at the boundaries (just below `text_top`, just past the last content row)
  and `row_to_line_idx` (in-range mapping, out-of-range `None`, the `offset` shift) from
  primitives alone — no `Model`.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean;
  complexity within budget.

## Alternatives considered

- **A `DetailViewport { offset, viewport_rows }` struct with methods.** Rejected: it adds a
  constructor and ties the geometry to a value assembled from the model; free functions over
  primitives are simpler, equally local, and trivially pure-testable.
- **Also fold the offset-clamp height (`model.rs:355`, `:993`,
  `(viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1)`).** Deferred: that is a
  related but distinct quantity (the `usize` text-viewport height with a floor of 1, used for
  scroll-offset clamping, not row→line hit mapping). It can adopt `content_height` in a later
  tidy; pulling it in now widens the slice for a different concern.
- **Leave the two non-click copies (status quo after ADR 0044).** Rejected: it keeps
  `text_top = 2` as a three-place magic literal and the mapping as a convention-kept
  triplicate — the exact drift obs 53 named.

## Consequences

**Positive:** one home for "how a Detail row maps to a content line"; `text_top` is named and
single-homed; hit-test, selection, and copy share the mapping so they cannot drift; the module
is pure and directly unit-testable through its interface, with no `Model`. A chrome-height or
top-row change is now a one-line edit in one module.

**Accepted trade-offs:** a third small TUI submodule (`detail_geometry`) beside `hit_test` —
a deliberate seam; the `lines.len()` guard stays caller-side (the geometry is viewport-only and
does not know `lines`), which keeps the module free of a `lines` dependency at the cost of one
guard line at each line-resolving caller. The offset-clamp height copies remain for now (noted
above).

## Related

- ADR: [/adr/0044-detail-click-resolution-as-hit-test-module.md](/adr/0044-detail-click-resolution-as-hit-test-module.md) (single-homed the click translation this generalizes)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md) (the selection/copy path that shares the mapping)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (the scrollable content the geometry maps into)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md) (the `src/tui/` module tree `detail_geometry` joins)
- Issue: [/issues/0048-detail-viewport-geometry-module.md](/issues/0048-detail-viewport-geometry-module.md)
