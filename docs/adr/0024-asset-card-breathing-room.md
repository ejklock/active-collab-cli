---
type: ADR
title: Anexos/Artefatos card breathing room — per-link separators, interior padding, named height ceiling
description: Give the detail asset card visual breathing room — a blank separator row between consecutive links, one row of interior vertical padding top and bottom, and a leading horizontal pad — and raise the panel's magic height cap to a named ceiling so the common multi-link case is not clipped. The renderer and the height function stay in lock-step.
status: Superseded
supersedes:
superseded_by: 0029
tags: [tui, ux, ratatui, detail, assets, layout]
timestamp: 2026-06-27T00:00:00Z
---

# 0024. Anexos/Artefatos card breathing room

> **Superseded by [ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md)
> (2026-06-27).** The fixed asset panel and its named height ceiling
> (`ASSET_PANEL_MAX_ROWS`) were retired when the assets folded into the global
> scroll. The **breathing room** decided here (a blank row between consecutive
> links, interior padding) is **carried forward inline** by ADR 0029 / BDR 0022;
> only the fixed-panel ceiling is reversed. Retained for provenance.

## Context

The detail view renders an **Anexos/Artefatos** card listing the task's assets as
`[n] ↗ <label>` rows ([ADR 0023](/adr/0023-asset-label-derivation.md) derives the
label). The card is a fixed bottom panel whose height is computed by
`asset_panel_render_height` and consumed by the layout split (`draw_detail`) and by
three model geometry functions (`detail_max_offset`, `is_in_body_area`,
`asset_panel_cmd_at`) — so the renderer and the height function **must agree row for
row**.

Today `render_assets_panel` flattens the asset rows directly into the box with **no
blank line between links and no interior padding** — the rows sit flush against the
border and against each other. The operator, dogfooding, asked for two things: *"os
links dentro de artefatos deveriam ter maior espaçamento vertical… e também o padding
total interno do card um pouco maior."*

The height function caps at a **magic `8`** (`(row_count + 2).min(8)`). That cap was
sized for the flush layout. Once per-link spacing and interior padding are added, a
modest card already exceeds it — the operator's own four-link card needs `4 rows + 3
separators + 2 vpad + 2 borders = 11`, which the cap would clip, **hiding links**.

Force: **readability of an actionable list** — the asset rows are clickable links; they
must be legible (spaced) and none may be hidden by an arbitrary ceiling. Volumetria
(number of assets) is naturally small, but the ceiling must clear the common case.

## Decision

### 1. Compose the card with breathing room

`render_assets_panel` builds its content (inside the ratatui border) as, top to bottom:

- **`PANEL_VPAD` (1) blank row** of interior top padding;
- the asset rows, with **one blank separator row between consecutive assets**
  (no leading/trailing separator);
- **`PANEL_VPAD` (1) blank row** of interior bottom padding.

Each non-blank content row is prefixed with **`PANEL_HPAD` (1) leading space**, and the
label wrap width is reduced by `PANEL_HPAD` on each side so text never collides with the
right border. `PANEL_HPAD`/`PANEL_VPAD` are the existing panel constants — the asset card
now uses the same padding vocabulary as the body panels.

### 2. Height function stays in lock-step

`asset_panel_render_height` sums the **same** terms the renderer emits: per-asset wrapped
rows (at the padded width) + `(n − 1)` separators + `2 × PANEL_VPAD` + `2` borders. The
renderer and the height function share one composition so the layout split, scroll bound,
and click hit-test never drift from what is drawn.

### 3. Named height ceiling, not a magic number

The magic `.min(8)` becomes `.min(ASSET_PANEL_MAX_ROWS)`, a named constant sized to clear
the common multi-link card with spacing (the operator's four-link case = 11 rows). A fixed
ceiling keeps the function signature stable and bounds the panel so it cannot dominate the
detail view; the body region absorbs the remainder via the existing `saturating_sub`.

## Alternatives considered

- **Viewport-responsive cap** (`min(natural, available − MIN_BODY_ROWS)`). Correct for
  pathological small terminals (panel never starves the body below a floor), but it
  changes `asset_panel_render_height`'s signature and ripples through four call sites and
  several geometry tests for a benefit the common case never sees. **Deferred (YAGNI)** —
  recorded here as the next refinement if body-starvation on tiny terminals is ever
  reported. The named ceiling is the smaller, sufficient step.
- **Half-line spacing / no separator, only padding.** Rejected: the operator asked
  specifically for spacing *between the links*; a blank separator is the legible TUI idiom.
- **Keep the magic 8, just add padding.** Rejected: it clips the operator's own card.

## Consequences

**Positive:** the asset card reads as a list of distinct, legible links with room to
breathe; none are hidden in the common case; the card uses the same padding constants as
the rest of the detail panels.

**Accepted trade-offs:** the card is taller (more vertical space for the same assets), so
on short terminals the body region shrinks sooner (graceful, pre-existing `saturating_sub`
behavior). A card with many assets still clips at `ASSET_PANEL_MAX_ROWS` — a higher
threshold than before, and the same clipping *class* that already existed at 8; the
viewport-responsive cap above is the documented escape hatch.

## Related

- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (asset panel as a dynamic-height region)
- ADR: [/adr/0023-asset-label-derivation.md](/adr/0023-asset-label-derivation.md) (the link label this card lays out)
- BDR: [/bdr/0018-asset-card-breathing-room.md](/bdr/0018-asset-card-breathing-room.md)
- Issue: [/issues/0023-d1d-asset-card-spacing.md](/issues/0023-d1d-asset-card-spacing.md)
- Architecture: [/architecture.md](/architecture.md)
