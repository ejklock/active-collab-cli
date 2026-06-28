---
type: Issue
title: "D1d — Anexos/Artefatos card breathing room: blank line between links + interior padding"
description: Give the detail asset card visual breathing room — a blank separator row between consecutive links, one row of interior vertical padding top and bottom, and a leading horizontal pad — keeping the renderer and asset_panel_render_height in lock-step, and raise the magic height cap to a named ceiling so the common multi-link card is not clipped.
status: closed
labels: [tui, ux, detail, assets, layout]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## D1d — Anexos/Artefatos card breathing room

Implements [ADR 0024](/adr/0024-asset-card-breathing-room.md), observable behavior pinned
by [BDR 0018](/bdr/0018-asset-card-breathing-room.md). Detail-polish family (D1a–D1c
shipped). Traces to [PRD 0001](/prd/0001-rust-tui-cli-parity.md).

### Problem

`render_assets_panel` flattens the asset rows flush — no blank line between links, no
interior padding — so the Anexos/Artefatos card reads as a dense block pressed against the
border. The operator asked for vertical spacing between the links and a bit more interior
padding. The height function's magic `.min(8)` cap clips a spaced multi-link card (the
operator's four-link card needs 11 rows).

### Decision

Compose the card as `top vpad + asset rows with a blank separator between consecutive
assets + bottom vpad`, each non-blank row prefixed with `PANEL_HPAD` and wrapped at the
padded width. `asset_panel_render_height` sums the same terms so it stays in lock-step with
the renderer. Replace the magic `8` with a named `ASSET_PANEL_MAX_ROWS` ceiling sized to
clear the common card.

### Scope

Included: `src/tui/screens/detail.rs` (`render_assets_panel` composition +
`asset_panel_render_height` formula + `ASSET_PANEL_MAX_ROWS`), `src/render.rs` (padded
wrap width for asset rows if the width term lives there), and the affected
`tests/unit/*` (height tests + TestBackend render tests for the card). Excluded: the label
derivation (D1b, unchanged); the responsive viewport cap (deferred per ADR 0024); body
text / selection (V6, unaffected — the card is a separate widget).

### Acceptance

- A blank row separates consecutive links; one blank row of interior padding sits above
  the first link and below the last (BDR 0018 Sc. 1, 2); asserted via TestBackend.
- Each link row is inset from the left border by `PANEL_HPAD`; wrapped labels stay clear of
  the right border (Sc. 3).
- A four single-line-label asset card shows all four links — no clip (Sc. 4).
- `asset_panel_render_height` equals the rows the renderer emits across asset counts and a
  wrapped label (Sc. 5); empty list → height 0, no card (Sc. 6).
- Full suite green; clippy `-D warnings`, fmt, comment-policy clean; complexity within
  budget; the card render/height tests are mutation-resistant.

### Plan

Single slice (D1d). 1) Add `ASSET_PANEL_MAX_ROWS` and the padded-wrap-width term.
2) Recompose `render_assets_panel` with top/bottom vpad, per-link blank separators, and
`PANEL_HPAD` leading pad. 3) Update `asset_panel_render_height` to sum the same terms,
capped at `ASSET_PANEL_MAX_ROWS`. 4) Update the height/geometry tests and add/adjust
TestBackend render assertions for spacing + padding + no-clip.
