---
type: ADR
title: Move the asset-open hint into the Anexos card as an italic footnote
description: Remove the "Ctrl/Cmd+click open asset" hint from the Detail footer and render it instead as an italic footnote inside the Anexos/Artefatos card, where the assets it describes actually live. The card geometry stays in lock-step with the click hit-test so asset open mapping is unaffected.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, detail, assets]
timestamp: 2026-06-27T00:00:00Z
---

# 0027. Asset-open hint inside the Anexos card

## Context

[ADR 0025](/adr/0025-asset-activation-ctrl-cmd-click.md) (D1e) made assets open on
Ctrl/Cmd+click and surfaced that affordance as a trailing segment of the Detail **footer**
hint: *"↑/↓ scroll  r refresh  Esc/b back  q quit  Ctrl/Cmd+click open asset"*.

The operator asked to move that hint **off the footer and into the Anexos/Artefatos card
itself, in italic**: *"esse hint de Ctrl/Cmd abrir anexo pode ir dentro ali da seção/card dele
em itálico e sair do footer."*

Force: **proximity** — a hint reads best next to the thing it describes. The footer is a
catch-all for global keys; the "how do I open an attachment" affordance belongs in the
attachments card, where the eye already is when deciding to click one.

## Decision

1. **Footer loses the asset segment.** `hint_for_screen` renders the same hint for the Detail
   screen whether or not it has assets: *"↑/↓ scroll  r refresh  Esc/b back  q quit"*. The two
   Detail arms collapse into one.

2. **The card gains an italic footnote.** `render_assets_panel` renders the hint
   ("Ctrl/Cmd+clique abrir anexo") as the **last interior line of the card**, in an italic,
   dimmed style, after the assets and a blank separator, before the bottom interior padding.
   It is a label, not an asset — it is never a click target.

3. **Geometry stays in lock-step.** The hint adds a named `ASSET_HINT_ROWS` (a blank
   separator + the hint line) to `asset_panel_render_height`, appended **after** the existing
   `ASSET_PANEL_MAX_ROWS` asset cap so the footnote is not itself capped away in the common
   case. Because the footnote sits **after** the asset spans, the click hit-test
   (`asset_index_at_panel_row`) needs **no change** to its asset walk: trailing rows already
   fall through to `None`. Both the renderer and the hit-test keep deriving the panel height
   from the single authoritative `asset_panel_render_height`, so no second divergent count can
   appear (the invariant that has bitten this panel before — V6, D1d).

## Alternatives considered

- **Top-of-card placement.** Rejected: it would shift every asset's row index, forcing a
  change to the fragile `asset_index_at_panel_row` walk (the recurring off-by-one bug site).
  Bottom placement leaves the asset walk untouched.
- **Keep it in the footer.** Rejected by the operator — proximity to the assets wins.
- **A separate hint widget below the panel.** Rejected: more layout surface for one line; the
  card already owns its interior padding and is the natural home.

## Consequences

**Positive:** the affordance sits with the attachments; the footer is shorter and uniform
across Detail states; the click hit-test is unchanged, so asset open mapping carries zero
regression risk from this move.

**Accepted trade-off:** when a task has enough attachments to fill the panel to its
`ASSET_PANEL_MAX_ROWS` cap, the trailing footnote may be clipped (the assets win the space).
This is acceptable — the hint is a nicety, and the common case (a handful of attachments)
shows it. A task with no assets has no card and therefore no hint (and never needed one).

## Related

- ADR: [/adr/0025-asset-activation-ctrl-cmd-click.md](/adr/0025-asset-activation-ctrl-cmd-click.md) (D1e — the hint this relocates)
- ADR: [/adr/0024-asset-card-breathing-room.md](/adr/0024-asset-card-breathing-room.md) (the card's PANEL_VPAD/separator geometry)
- BDR: [/bdr/0021-asset-open-hint-in-card.md](/bdr/0021-asset-open-hint-in-card.md)
- Issue: [/issues/0026-d1f-asset-hint-in-card.md](/issues/0026-d1f-asset-hint-in-card.md)
- Architecture: [/architecture.md](/architecture.md)
