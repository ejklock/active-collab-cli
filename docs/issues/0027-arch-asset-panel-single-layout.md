---
type: Issue
title: "ARCH — one layout source of truth for the Anexos/Artefatos panel (asset_panel module)"
description: Behavior-preserving deepening; replace the three parallel encodings of the asset-panel row composition (renderer pushes rows, height counts them, model hit-test walks them) with a single pure layout function in a new src/tui/screens/asset_panel.rs, consumed by all three as thin adapters.
status: open
labels: [refactor, tui, detail, assets, layout, maintainability]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## ARCH — asset-panel single layout source of truth

Implements [ADR 0028](/adr/0028-asset-panel-single-layout-source.md). No observable
behavior change → parity is the contract (no new BDR); the behavior stays the one
pinned by [BDR 0018](/bdr/0018-asset-card-breathing-room.md) Sc. 5 (*height matches
geometry*) and [BDR 0021](/bdr/0021-asset-open-hint-in-card.md) Sc. 3–4 (*click maps
after the taller card*). Follows the same parity-gated pattern as the ARCH render
refactor ([issue 0014](/issues/0014-arch-refactor-render-decompose-relocate.md)).
Surfaced by an architecture review (candidate #1: *the asset-panel layout has two
authors*).

### Problem

The card's row composition (top `PANEL_VPAD`, per-asset wrapped rows, blank
separators, the italic Ctrl/Cmd+click footnote, bottom `PANEL_VPAD`, borders) is
encoded three times that must agree row for row: `render_assets_panel` pushes the
rows (`detail.rs`), `asset_panel_render_height` counts them (`detail.rs`), and
`asset_index_at_panel_row` walks them (`model.rs`). Two adapters of one composition
that lives nowhere — V6, D1d, and D1f each hand-edited all three. The model
hit-test also recomputes `content_width` by hand, does not model the
`ASSET_PANEL_MAX_ROWS` cap (latent off-by-one when assets exceed the cap), and does
not model the hint rows (relies on them falling out of the viewport bound).

### Decision

Introduce `src/tui/screens/asset_panel.rs` owning a pure `layout(assets,
content_width) -> Vec<PanelRow>` (`PanelRow = Pad | Asset { idx, text } |
Separator | Hint(text)`) plus `apply_cap`. The renderer maps the capped vector to
`Line`s (styling applied by kind via `theme`), the height is its `.len() + 2`
borders, and the click hit-test indexes it. The cap and hint become modeled rows,
not out-of-range luck. See ADR 0028 for the full decision and rejected
alternatives.

### Scope

Included:

- `src/tui/screens/asset_panel.rs` (new) — `PanelRow`, `layout`, `apply_cap`,
  `render`, `height`, `index_at`; the moved `asset_content_width`,
  `ASSET_PANEL_MAX_ROWS`, `ASSET_HINT_ROWS`.
- `src/tui/screens/mod.rs` — `pub mod asset_panel;` and the `asset_panel_render_height`
  re-export path the callers use stays resolvable.
- `src/tui/screens/detail.rs` — `render_assets_panel` and `asset_panel_render_height`
  delegate to the module (or are replaced by its `render`/`height`).
- `src/tui/model.rs` — `asset_panel_cmd_at` calls `asset_panel::index_at`; remove
  the local `asset_index_at_panel_row` and the duplicated `content_width` formula.
- `tests/unit/model.rs`, `tests/unit/tui_render.rs` — repoint the moved-function
  callers; add a direct `Vec<PanelRow>` layout test. Derive every expected
  row/col/style from the **real** TestBackend buffer, never assumed geometry.

Excluded: any visible change to the card (spacing, padding, hint, cap height are
all reproduced exactly); other screens; the `render.rs` split (architecture-review
candidate #2, its own future issue).

### Acceptance

- The card renders byte-for-byte as today: top/bottom pad, per-link blank
  separators, leading `PANEL_HPAD`, the italic Ctrl/Cmd footnote after the assets,
  the `ASSET_PANEL_MAX_ROWS` ceiling (BDR 0018 Sc. 1–4, BDR 0021 Sc. 2) — asserted
  via TestBackend.
- `asset_panel::height` equals the rows the renderer emits across asset counts and
  a wrapped label, and equals the geometry the body layout / scroll bound / click
  hit-test use (BDR 0018 Sc. 5) — one source, so they cannot disagree.
- Ctrl/Cmd+click on asset *k* still opens asset *k*, including the tenth asset and a
  wrapped label's continuation row; the footnote and pad/separator rows open nothing
  (BDR 0021 Sc. 3–4); empty list → no card, height 0 (BDR 0018 Sc. 6).
- A direct unit test asserts the `Vec<PanelRow>` composition (the layout is the
  test surface); the cap and hint are modeled rows, not out-of-range luck.
- Full suite green; `clippy -D warnings`, `fmt`, comment-policy clean; complexity
  within budget (cyclomatic ≤ 10 / ≤ 8 new); tests mutation-resistant. Verified by
  parity — assertion changes only where they pin the preserved behavior; the rest
  are call-site repoints.

### Plan

1. Create `asset_panel.rs`: `PanelRow`, `layout`, `apply_cap`; move
   `asset_content_width`, `ASSET_PANEL_MAX_ROWS`, `ASSET_HINT_ROWS`. Register in
   `screens/mod.rs`.
2. Add `render`, `height`, `index_at` deriving from `apply_cap(layout(...))`.
3. Repoint `detail.rs` (`render_assets_panel`/`asset_panel_render_height`) to the
   module; keep the `height` signature stable for its four callers.
4. Repoint `model.rs` `asset_panel_cmd_at` to `asset_panel::index_at`; delete the
   local walk and the duplicated width formula.
5. Add the direct `Vec<PanelRow>` test; repoint the moved-function test callers;
   re-derive geometry assertions from the real buffer.
