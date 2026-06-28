---
type: ADR
title: Give the Anexos/Artefatos panel one layout source of truth (asset_panel module)
description: A behavior-preserving deepening — replace the three parallel encodings of the asset-panel row composition (the renderer pushes rows, the height function counts them, the model hit-test walks them) with a single pure layout function in a new src/tui/screens/asset_panel.rs module. The renderer maps the layout to lines, the height is its length, and the click hit-test indexes it, so the renderer/height/hit-test can no longer drift.
status: Accepted
supersedes:
superseded_by:
tags: [refactor, tui, ratatui, detail, assets, layout, maintainability, complexity]
timestamp: 2026-06-27T00:00:00Z
---

# 0028. One layout source of truth for the Anexos/Artefatos panel

> **Amended by [ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md)
> (2026-06-27).** The core decision — one composition source (`asset_panel::layout`
> → `Vec<PanelRow>`) that render and the click hit-test both derive from — **still
> holds**. ADR 0029 repurposes the module: `layout`/`PanelRow` are retained and now
> feed **inline scrollable content** plus a scroll-aware line→asset-index map, while
> the fixed-panel adapters (`apply_cap`, `height`, the block `render`, the panel
> `index_at`, and the `ASSET_PANEL_MAX_ROWS` cap) are **retired** with the fixed
> panel. The deferred "Note 1" `apply_cap` refinement is thereby subsumed (no cap,
> nothing to truncate).

## Context

The detail view's **Anexos/Artefatos** card is laid out by three functions that
must agree on the same row composition — top `PANEL_VPAD`, per-asset wrapped rows,
a blank separator between consecutive assets, the italic Ctrl/Cmd+click footnote,
bottom `PANEL_VPAD`, and the two borders:

1. **The renderer** `render_assets_panel` (`src/tui/screens/detail.rs`) *pushes*
   the rows into a `Vec<Line>`.
2. **The height function** `asset_panel_render_height` (`src/tui/screens/detail.rs`)
   *counts* the same rows: per-asset `asset_row_lines().len()` + `(n − 1)`
   separators + `2 × PANEL_VPAD` + `2` borders, capped at `ASSET_PANEL_MAX_ROWS`,
   then `+ ASSET_HINT_ROWS`.
3. **The hit-test** `asset_index_at_panel_row` (`src/tui/model.rs`) *walks* the
   rows to map a clicked screen row back to an asset index.

In the `/codebase-design` vocabulary the renderer and the hit-test are two
**adapters** of one composition — *"one adapter is a hypothetical seam, two is a
real one."* The seam is real, yet the composition it adapts **lives nowhere**:
each function re-encodes the layout by hand. Three slices paid the tax — V6, D1d
([ADR 0024](/adr/0024-asset-card-breathing-room.md)), and D1f
([ADR 0027](/adr/0027-asset-open-hint-in-card.md)) each hand-edited all three
sites and reasoned carefully about keeping them in lock-step.

The coupling is also **leaky in three concrete ways** a single source of truth
removes:

- **Duplicated width formula.** `asset_panel_cmd_at` (`src/tui/model.rs`) recomputes
  `content_width = inner_width − 2 × PANEL_HPAD` by hand, mirroring
  `asset_content_width` in `detail.rs` — a second copy of one formula.
- **The hit-test does not model the cap.** `asset_index_at_panel_row` walks every
  asset's rows with no `ASSET_PANEL_MAX_ROWS` ceiling, while the renderer and the
  height function cap. When assets exceed the cap, the walk can resolve a row the
  renderer clipped away — a latent off-by-one.
- **The hit-test does not model the hint rows.** It relies on the trailing
  `ASSET_HINT_ROWS` and bottom pad falling outside the `viewport_rows` bound rather
  than representing them — exactly the fragile reasoning D1f had to do by hand.

Force: **maintainability and locality** — a layout with two real consumers should
be decided once, in one place, next to the adapters that consume it; not a code
force solved with an architecture hammer, just a shallow seam made deep. The
arborist complexity gate passes today, so this is a **structure/right-place**
deepening, not a gate violation.

## Decision

A **behavior-preserving** deepening, delivered as slice **ARCH** (no observable
behavior change → no new BDR; the contract is parity with the behavior already
pinned by [BDR 0018](/bdr/0018-asset-card-breathing-room.md) Sc. 5 *height matches
geometry* and [BDR 0021](/bdr/0021-asset-open-hint-in-card.md) Sc. 3–4 *click maps
after the taller card*).

### 1. A new `asset_panel` module owns the composition

Create `src/tui/screens/asset_panel.rs` (registered in
`src/tui/screens/mod.rs`). It owns the row composition and the three thin adapters
that today live split across `detail.rs` and `model.rs`. The constants
`ASSET_PANEL_MAX_ROWS`, `ASSET_HINT_ROWS`, `asset_content_width`, and the asset
geometry move here; `detail.rs` and `model.rs` call into the module.

### 2. The layout is a pure value: `Vec<PanelRow>`

```rust
pub enum PanelRow {
    Pad,
    Asset { idx: usize, text: String },
    Separator,
    Hint(String),
}

/// Pure interior composition, top to bottom, BEFORE the cap and borders.
pub fn layout(assets: &[Asset], content_width: usize) -> Vec<PanelRow>;

/// Trim the interior to ASSET_PANEL_MAX_ROWS, then re-append the hint rows.
pub fn apply_cap(rows: Vec<PanelRow>) -> Vec<PanelRow>;
```

Each row carries its **kind tag and its already-wrapped text** — *not* a styled
`ratatui` span. `layout` is the single place that calls `asset_row_lines`, so the
"how many rows does this asset wrap into" computation exists exactly once. Styling
stays a render concern: the renderer maps a row's *kind* to its theme style
(`theme::asset_style` / `theme::asset_hint_style`), so `asset_panel` carries no
`ratatui::Style` dependency and stays pure data.

### 3. The three adapters derive from the one vector

- **`render(frame, area, assets)`** — `apply_cap(layout(...))`, then maps each
  `PanelRow` to a `Line` (Pad/Separator → blank; Asset/Hint → `PANEL_HPAD` lead +
  styled text). Borders stay with the ratatui `Block`.
- **`height(assets, inner_width) -> u16`** — `apply_cap(layout(...)).len() + 2`
  borders. Same signature as today's `asset_panel_render_height`, so the four
  existing callers (`draw_detail`, `detail_max_offset`, `is_in_body_area`,
  `asset_panel_cmd_at`) are unchanged.
- **`index_at(assets, content_width, panel_top, row, viewport) -> Option<usize>`**
  — maps a screen row to an interior index into `apply_cap(layout(...))` and
  returns `Some(idx)` iff that row is an `Asset { idx, .. }`. The cap and the hint
  rows are now *modeled*, not relied upon to fall out of range.

### Guard

The slice is gated by **parity**: the full suite stays green
(`docker compose run --rm dev cargo test -- --test-threads=1`), `clippy -D
warnings` and `fmt` clean, the comment-policy test passes, complexity within
budget. The new module also gets a **direct unit test on the `Vec<PanelRow>`** —
the layout becomes the test surface, so a layout change is asserted on the vector
instead of by reading a `TestBackend` buffer. Any test that changes is a call-site
repoint (functions moved modules), never an assertion of new behavior.

## Alternatives considered

- **Structural-only `Vec<RowKind>` (no text in the rows).** The renderer would
  re-call `asset_row_lines` to get the text, leaving the *wrap composition*
  computed in two places again — the very double-encoding this deepening removes.
  Rejected; carrying the wrapped text in the row is what makes the source single.
- **Carry styled `Vec<Span>` in the rows.** Deeper than needed and it leaks
  `ratatui::Style`/`theme` into the otherwise-pure layout. Rejected in favor of
  *kind + text*, with styling applied by kind at render time.
- **Keep the code in `detail.rs`, only move the hit-test in from `model.rs`.**
  Smaller diff, but `detail.rs` is already the large detail-screen file and the
  seam stays buried. Rejected: the point of the deepening is to give the layout a
  visible home with its adapters co-located.
- **Bake the cap, hint, and borders into the layout vector** (a `Border` row kind,
  cap applied inline using viewport height). Rejected: it mixes *composition*
  (content-driven) with *fitting* (viewport-driven) and makes `layout` depend on
  viewport size. Keeping `layout` pure and `apply_cap` separate isolates the
  peculiar "hint after the cap" rule in one named, testable step.
- **Leave it as three encodings.** Rejected: the seam is real (two adapters), the
  deletion test passes (deriving height + hit-test from the shared vector
  *concentrates* all panel geometry into one pure function), and the same class of
  off-by-one has already cost three slices.

## Consequences

**Positive:** the asset-panel layout is decided once; the renderer, the height,
and the hit-test become thin adapters that *cannot* disagree. The
"renderer/hit-test drifted" bug class becomes unrepresentable; the duplicated
width formula and the unmodeled cap/hint are gone. The layout gains a direct
unit-test surface (`Vec<PanelRow>`), reducing reliance on `TestBackend`
buffer-reading. `model.rs` sheds panel geometry it never should have owned.

**Accepted trade-offs:** a churny diff across `detail.rs`, `model.rs`,
`screens/mod.rs`, and the affected tests, verified purely by parity (green tests +
clean gates) since there is no new behavior to assert. A new small module is added
to the screens tree (documented in [architecture.md](/architecture.md)). Carrying
the wrapped text in `PanelRow` allocates a `String` per row — negligible for the
small asset counts the panel ever holds, and the cost the single-source-of-truth
buys back.

## Related

- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md) (the prior behavior-preserving ARCH refactor; same parity-is-the-contract pattern)
- ADR: [/adr/0024-asset-card-breathing-room.md](/adr/0024-asset-card-breathing-room.md) (D1d — the spacing this layout reproduces)
- ADR: [/adr/0027-asset-open-hint-in-card.md](/adr/0027-asset-open-hint-in-card.md) (D1f — the in-card hint this layout reproduces)
- BDR: [/bdr/0018-asset-card-breathing-room.md](/bdr/0018-asset-card-breathing-room.md) (Sc. 5 height-matches-geometry — the preserved contract)
- BDR: [/bdr/0021-asset-open-hint-in-card.md](/bdr/0021-asset-open-hint-in-card.md) (Sc. 3–4 click-maps-after-the-taller-card — the preserved contract)
- Issue: [/issues/0027-arch-asset-panel-single-layout.md](/issues/0027-arch-asset-panel-single-layout.md)
- Architecture: [/architecture.md](/architecture.md)
