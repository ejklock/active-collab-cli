---
type: ADR
title: detail_geometry absorbs the selection column math — one deep selected_text(...) interface owns row→line and column→text, retiring the box_inner_content_pub reach-in
description: ADR 0045 single-homed the detail viewport row→line_idx mapping in the pure detail_geometry module, but only the ROW half moved. The COLUMN half — mapping a Selection's frame columns to inner-content display columns and slicing the boxed line — stayed in model.rs::extract_selected_text/extract_line_slice, which reaches across the seam into render internals (box_inner_content_pub, BODY_LEFT_CHROME_COLS, display_width, slice_by_display_cols) and mixes model's own DETAIL_CONTENT_BLOCK_BORDER_COLS into the offset. box_inner_content_pub is a pass-through adapter that exists solely for this one caller. Deepen detail_geometry with a single selected_text(offset, viewport_rows, sel, lines) -> String that owns both halves — the row loop, the column offset, the box unwrap, and the display-width slice — over the text_measure primitives (ADR 0049); move the Selection type and DETAIL_CHROME_ROWS into detail_geometry so it is self-contained; delete model's two extract functions and the box_inner_content_pub wrapper. No behavior change — the existing V6 selection/copy specs are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [tui, geometry, viewport, selection, refactor, locality, depth, ratatui]
timestamp: 2026-06-30T00:00:00Z
---

# 0050. detail_geometry absorbs the selection column math

## Context

[ADR 0045](/adr/0045-detail-viewport-geometry-module.md) extracted a pure `src/tui/detail_geometry.rs`
that owns **how a terminal row maps to a content line** — `DETAIL_TEXT_TOP`, `content_height`,
`is_in_content`, `row_to_line_idx`. Hit-test, the in-body predicate, and text selection all
consume it, so the row→`line_idx` mapping lives once.

But only the **row** half of the detail viewport moved. The **column** half — mapping a
`Selection`'s frame columns to inner-content display columns, and extracting the text — stayed in
`model.rs`, and it **reaches across the seam** into render internals:

- `extract_selected_text` (`model.rs:~1210`) loops the selected rows via
  `detail_geometry::row_to_line_idx` (the row half, correctly delegated) but then calls…
- `extract_line_slice` (`model.rs:1272-1300`), which:
  - unwraps the box with `render::box_inner_content_pub(line)` — a **pass-through adapter**
    (`render.rs:130`, `box_inner_content(s)`) exposed publicly *solely* for this caller;
  - measures with `render::display_width`;
  - computes `left_offset = DETAIL_CONTENT_BLOCK_BORDER_COLS + BODY_LEFT_CHROME_COLS`, **mixing
    model's own constant** (`DETAIL_CONTENT_BLOCK_BORDER_COLS`, `model.rs:374`) with render's
    (`BODY_LEFT_CHROME_COLS`);
  - slices with `render::slice_by_display_cols`.

So the column-offset math lives in `model.rs`, borrowing four render internals and a magic offset
composed from two modules' constants. `box_inner_content_pub` is an adapter with exactly one
caller — the classic sign of a seam in the wrong place. And `detail_geometry::content_height`
still reaches *up* into `model::DETAIL_CHROME_ROWS` (a backwards dependency: geometry is
lower-level than the model). The row math is single-homed; the column math is not.

This lands after [ADR 0049](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md),
which makes the width primitives a named, pure, `richtext`-free `text_measure` interface —
so `detail_geometry` can own the column math without dragging `richtext`.

## Decision

Deepen `detail_geometry` so it owns the **whole** detail viewport — row *and* column — behind one
interface, and make it self-contained.

1. **One deep extraction interface.** Add
   `selected_text(offset: usize, viewport_rows: u16, sel: Selection, lines: &[String]) -> String`.
   Behind this small interface sits the full implementation: the selected-row loop (already using
   `row_to_line_idx`), the per-row column-offset computation, the box unwrap
   (`text_measure::box_inner_content`), the display-width slice
   (`text_measure::slice_by_display_cols`), and the `\n` join. `model` calls it in one line; it no
   longer knows how a selection maps to text.

2. **The column constants and offset move in.** `DETAIL_CONTENT_BLOCK_BORDER_COLS` moves from
   `model.rs` into `detail_geometry`; the left offset
   (`border_cols + BODY_LEFT_CHROME_COLS`, `BODY_LEFT_CHROME_COLS` now from `text_measure`) is
   computed once, inside the module. No column-offset arithmetic remains in `model.rs`.

3. **`Selection` moves into `detail_geometry`.** `Selection { anchor, cursor }` with `is_drag` /
   `normalized` is a pure viewport-coordinate concept; it moves from `model.rs` into
   `detail_geometry`, and `model` imports it. This removes the geometry→model type dependency the
   `selected_text` signature would otherwise create.

4. **`DETAIL_CHROME_ROWS` moves into `detail_geometry`.** The chrome-row count is a geometry fact;
   moving it down inverts the current backwards `content_height → model::DETAIL_CHROME_ROWS`
   reach, so `detail_geometry` depends on nothing above it and `model` reads the constant from the
   geometry module.

5. **Delete the reach-in.** `model.rs::extract_selected_text` and `extract_line_slice` are removed
   (subsumed by `selected_text`), and the `box_inner_content_pub` pass-through is deleted from
   `render`/`text_measure` — its one caller is gone, so `box_inner_content` need only be `pub` at
   its `text_measure` home.

### Guard / fitness function

- **Behavior preserved — invisible to the user.** Drag-selection and clipboard copy produce the
  identical string; all existing V6 selection/copy specs
  ([ADR 0021](/adr/0021-app-managed-text-selection-clipboard.md)) stay green.
- **The interface is the test surface.** `detail_geometry` unit tests exercise `selected_text`
  from primitives — `offset`, `viewport_rows`, a `Selection`, a `&[String]` of boxed lines —
  asserting the extracted text across single-row, multi-row, partial-column, and double-width
  (CJK/emoji) selections, with **no `Model`**. The column math is now tested through the seam it
  lives behind, not past it.
- **One home, dependency direction corrected.** No `box_inner_content_pub`, no
  `DETAIL_CONTENT_BLOCK_BORDER_COLS`, and no `extract_line_slice` column arithmetic remain in
  `model.rs`; `detail_geometry` no longer reaches up into `model::DETAIL_CHROME_ROWS`. Grep
  confirms `Selection` and `DETAIL_CHROME_ROWS` have one home each.
- **The deletion test passes.** Deleting `selected_text` would scatter the row loop + column
  offset + box unwrap + slice back into `model` — it concentrates complexity, not merely moves it.
- Full suite green; `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, `comment_policy`
  clean; complexity within budget.

## Alternatives considered

- **Expose only a column helper (`inner_col(frame_col) -> usize`), keep model's loop.** Rejected
  in the design grilling: it leaves `model` owning the row loop, the box unwrap, and the join —
  the module stays shallow and `model` still knows the boxed-line structure. The deep
  `selected_text` puts the whole extraction behind one interface.
- **Keep the width primitives in `render` and have `detail_geometry` depend on `render`.**
  Rejected: it creates a geometry→render edge and would reopen the same question when
  [ADR 0049](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md) splits `render`.
  Depending on the pure `text_measure` core keeps the edge minimal and `richtext`-free.
- **Leave the column math in `model` (status quo after ADR 0045).** Rejected: it keeps
  `box_inner_content_pub` alive for one caller, keeps the two-module offset mix, and leaves the
  detail viewport half-homed — the exact drift ADR 0045 set out to end, finished on the column axis.

## Consequences

**Positive:** the detail viewport is fully single-homed — row *and* column — behind one deep,
pure, directly-testable interface; `box_inner_content_pub` (an adapter for one caller) is deleted;
the offset stops mixing two modules' constants; `Selection` and `DETAIL_CHROME_ROWS` live where
they belong and the geometry→model backwards edge is removed; `model.rs` sheds two functions and
four render reach-ins.

**Accepted trade-offs:** `detail_geometry` gains a dependency on `text_measure` (deliberate, and
`richtext`-free by [ADR 0049](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md)'s
design) and takes a `&[String]` in `selected_text` — so it now knows about `lines`, unlike the
viewport-only row helpers, which stay `lines`-free. This is the right trade: extraction inherently
needs the line contents, and putting it here is what removes the reach-in.

## Related

- ADR: [/adr/0045-detail-viewport-geometry-module.md](/adr/0045-detail-viewport-geometry-module.md) (single-homed the row half this completes on the column axis)
- ADR: [/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md) (creates the pure `text_measure` core `selected_text` slices over)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md) (the V6 selection/copy behavior preserved)
- ADR: [/adr/0044-detail-click-resolution-as-hit-test-module.md](/adr/0044-detail-click-resolution-as-hit-test-module.md) (the sibling hit-test module that already consumes `detail_geometry`)
