---
type: Issue
title: "D1f — move the asset-open hint into the Anexos card (italic), out of the footer"
description: Render the "Ctrl/Cmd+clique abrir anexo" hint as an italic footnote inside the Anexos/Artefatos card and remove it from the Detail footer, keeping the asset click hit-test in lock-step with the taller card.
status: closed
labels: [tui, ux, detail, assets]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## D1f — asset-open hint inside the Anexos card

Implements [ADR 0027](/adr/0027-asset-open-hint-in-card.md), behavior pinned by
[BDR 0021](/bdr/0021-asset-open-hint-in-card.md).

### Problem

The "Ctrl/Cmd+click open asset" affordance lives in the Detail footer, far from the
attachments it describes. The operator wants it inside the Anexos/Artefatos card, in italic.

### Decision

Move the hint into the card as an **italic, dimmed footnote** (last interior line, after the
assets + a blank separator, before the bottom padding) and drop it from the footer. Place it
at the **bottom** so the asset click hit-test (`asset_index_at_panel_row`) needs no change —
the new rows fall after the asset spans. Grow `asset_panel_render_height` by a named
`ASSET_HINT_ROWS`, appended after the asset cap so the footnote is not capped away in the
common case.

### Scope

- `src/tui/screens/detail.rs` — `render_assets_panel` (push blank + italic hint after assets)
  and `asset_panel_render_height` (+`ASSET_HINT_ROWS`, after the cap).
- `src/tui/view.rs` — `hint_for_screen` (collapse the two Detail arms; drop the asset segment).
- `src/tui/theme.rs` — an italic/dim `asset_hint_style`.
- `locales/pt_BR.json` — add the standalone card-hint key; remove the now-unused combined
  footer key.
- `tests/unit/model.rs` + `tests/unit/tui_render.rs` — height includes the hint; footer drops
  it; card shows the italic hint; asset click still maps (derive from the real buffer).

### Acceptance

- The Detail footer reads "↑/↓ scroll  r refresh  Esc/b back  q quit" with no "Ctrl/Cmd"
  segment, with or without assets (Sc.1).
- The Anexos card's last interior line shows "Ctrl/Cmd+clique abrir anexo" in italic (Sc.2).
- Ctrl/Cmd+click on asset *k* still opens asset *k*; the footnote row opens nothing (Sc.3, Sc.4).
- No card and no hint when there are no attachments (Sc.5).
- `asset_panel_render_height` grew by `ASSET_HINT_ROWS`; full suite green; clippy `-D
  warnings`, fmt, comment-policy clean; complexity within budget; tests mutation-resistant.

### Notes

When attachments fill the panel to `ASSET_PANEL_MAX_ROWS`, the trailing footnote may be
clipped — accepted (the assets win the space; the hint is a nicety).
