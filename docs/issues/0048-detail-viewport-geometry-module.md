---
type: Issue
title: "Detail viewport↔content geometry becomes one pure src/tui/detail_geometry.rs; the row→line_idx mapping and text_top=2 stop being copied across hit_test, is_in_body_area, and extract_selected_text"
description: Extract the detail content viewport mapping (DETAIL_TEXT_TOP, content_height, is_in_content, row_to_line_idx) into a pure src/tui/detail_geometry.rs taking primitives (offset, viewport_rows, row) — no Model. hit_test::viewport_to_line_col, is_in_body_area, and extract_selected_text all derive from it, so the mapping lives once and the text_top=2 magic literal is single-homed. Behavior (click resolution, in-body predicate, selection, copy) is unchanged.
status: closed
labels: [tui, geometry, viewport, refactor, locality, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Detail viewport geometry → one pure module (ADR 0045)

### Problem

After [ADR 0044](/adr/0044-detail-click-resolution-as-hit-test-module.md), the detail
viewport→`line_idx` mapping is still copied three times, with `text_top = 2` written as a
magic literal in each:

- `hit_test::viewport_to_line_col` (`src/tui/hit_test.rs:29-36`).
- `is_in_body_area` (`src/tui/model.rs:1125`).
- `extract_selected_text` (`src/tui/model.rs:1194`, the V6 copy path).

A change to the detail chrome height or the content's top row must be made identically in
three functions across two modules, or hit-testing, selection, and copy disagree about which
content line a row maps to (the drift obs 53 named).

### Decision (ADR 0045)

Extract a pure `src/tui/detail_geometry.rs` owning the mapping; all three consumers derive
from it.

### Scope

- `src/tui/detail_geometry.rs` (new, pure, primitive-typed — no `Model`):
  - `pub(crate) const DETAIL_TEXT_TOP: u16 = 2;`
  - `pub(crate) fn content_height(viewport_rows: u16) -> u16` → `viewport_rows.saturating_sub(DETAIL_CHROME_ROWS)`.
  - `pub(crate) fn is_in_content(viewport_rows: u16, row: u16) -> bool` → `row >= DETAIL_TEXT_TOP && row < DETAIL_TEXT_TOP + content_height(viewport_rows)`.
  - `pub(crate) fn row_to_line_idx(offset: usize, viewport_rows: u16, row: u16) -> Option<usize>` → `None` when `!is_in_content`, else `Some(offset + (row - DETAIL_TEXT_TOP) as usize)`.
- `src/tui/mod.rs`: declare `pub(crate) mod detail_geometry;` (DETAIL_CHROME_ROWS is already `pub(crate)`).
- `src/tui/hit_test.rs`: `viewport_to_line_col` becomes `let line_idx = detail_geometry::row_to_line_idx(offset, viewport_rows, row)?; if line_idx >= lines.len() { return None; } Some((line_idx, column as usize))`. No local `text_top`/`content_text_height`.
- `src/tui/model.rs`: `is_in_body_area` keeps its `Screen::Detail` guard and returns `detail_geometry::is_in_content(viewport_rows, row)`; `extract_selected_text` replaces its per-row bounds+mapping with `let Some(line_idx) = detail_geometry::row_to_line_idx(*offset, viewport_rows, vp_row) else { continue }; let Some(line) = lines.get(line_idx) else { continue };`.
- `tests/unit/model.rs`: keep every existing click + V6 selection/copy spec green; add `detail_geometry` unit tests (boundary `is_in_content`; `row_to_line_idx` in-range mapping, out-of-range `None`, the `offset` shift).

### Out of scope

- Folding the offset-clamp height (`model.rs:355`, `:993`,
  `(viewport_rows.saturating_sub(DETAIL_CHROME_ROWS) as usize).max(1)`) — a distinct quantity,
  deferred per ADR 0045.
- Any change to click resolution, the in-body predicate, selection, or copy behavior. Pure
  restructuring.

### Acceptance criteria

- **AC1** (constraint, inspection): `src/tui/detail_geometry.rs` exists and owns
  `DETAIL_TEXT_TOP`, `content_height`, `is_in_content`, `row_to_line_idx`; it is pure (takes
  `offset`/`viewport_rows`/`row`, not `&Model`; no mutation, no `Cmd`, no I/O). No
  `let text_top: u16 = 2` and no `viewport_rows.saturating_sub(DETAIL_CHROME_ROWS)` for the
  row→line mapping remain in `hit_test.rs` or in `is_in_body_area`/`extract_selected_text`.
- **AC2** (behavior, test): click resolution is unchanged — the existing buffer-derived
  `hit_test`/`handle_click_detail` specs (every kind, the out-of-range guard) stay green, now
  routing through `detail_geometry::row_to_line_idx`.
- **AC3** (behavior, test): text selection and clipboard copy are unchanged — the existing V6
  selection/copy specs (multi-row selection, out-of-viewport rows skipped, chrome-stripped
  slices) stay green.
- **AC4** (constraint, test): `detail_geometry` is unit-tested through its interface from
  primitives alone — `is_in_content` at both boundaries (just below `DETAIL_TEXT_TOP`, the
  last in-content row, the first row past it) and `row_to_line_idx` (in-range value with the
  `offset` shift, out-of-range `None`).
- **CC** (constraint, inspection): clean code — `DETAIL_TEXT_TOP`, `content_height`,
  `is_in_content`, `row_to_line_idx` self-describing; no banners/commented-out code; only
  non-obvious why-comments; comment-policy gate green.
- **CX** (constraint, command): complexity within budget — cyclomatic ≤ 10 (≤ 8 for new
  functions), cognitive ≤ 12 (quality-gate arborist).
- **TE** (constraint, command): tests assert observable behavior (the mapping/predicate and
  the unchanged click/selection/copy) and survive the mutation floor — changing the
  `DETAIL_TEXT_TOP` offset or the bounds comparison fails a test.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0045-detail-viewport-geometry-module.md](/adr/0045-detail-viewport-geometry-module.md)
- ADR: [/adr/0044-detail-click-resolution-as-hit-test-module.md](/adr/0044-detail-click-resolution-as-hit-test-module.md) (single-homed the click translation this generalizes)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md) (the selection/copy path sharing the mapping)
