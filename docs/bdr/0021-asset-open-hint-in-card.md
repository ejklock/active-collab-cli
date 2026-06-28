---
type: BDR
title: The asset-open hint lives inside the Anexos card in italic, not in the footer
description: On the Detail screen, the "Ctrl/Cmd+clique abrir anexo" hint is rendered as an italic footnote inside the Anexos/Artefatos card and no longer appears in the footer. The asset click-to-open mapping is unchanged by the card growing one footnote.
status: Accepted
superseded_by:
supersedes:
tags: [tui, ux, detail, assets]
timestamp: 2026-06-27T00:00:00Z
---

# 0021. Asset-open hint inside the Anexos card

## Context

Implements [ADR 0027](/adr/0027-asset-open-hint-in-card.md) (D1f). The Ctrl/Cmd+click
open-asset affordance moves from the Detail footer into the Anexos/Artefatos card as an
italic footnote.

## Textual Description

On the **Detail** screen:

- The **footer** shows *"↑/↓ scroll  r refresh  Esc/b back  q quit"* — the **same** whether or
  not the task has attachments. It no longer carries the "Ctrl/Cmd+click open asset" segment.
- When the task **has attachments**, the Anexos/Artefatos card shows, as its **last interior
  line** (after the assets and a blank separator, before the bottom padding), an **italic,
  dimmed** hint: **"Ctrl/Cmd+clique abrir anexo"**.
- The hint is a **label, not a click target**: clicking it opens nothing.
- **Clicking an asset** still opens that asset on Ctrl/Cmd+click exactly as before — the card
  growing by the footnote does not shift which asset a click resolves to.
- A task with **no attachments** has no card and shows no hint.

## Scenarios

**Scenario 1: hint leaves the footer** — Given a Detail screen with attachments, When the
footer renders, Then it reads "↑/↓ scroll  r refresh  Esc/b back  q quit" and does NOT contain
"Ctrl/Cmd" or "abrir anexo".

**Scenario 2: hint appears in the card, italic** — Given a Detail screen with attachments,
When the Anexos card renders, Then its last interior line contains "Ctrl/Cmd+clique abrir
anexo" and those cells carry the italic modifier.

**Scenario 3: asset click mapping unchanged** — Given a card with N attachments and the
italic footnote, When the operator Ctrl/Cmd+clicks the row of asset *k*, Then asset *k* opens
(the footnote at the bottom does not shift the asset hit-test).

**Scenario 4: footnote is not clickable** — Given the card's footnote line, When the operator
Ctrl/Cmd+clicks it, Then nothing opens (it resolves to no asset).

**Scenario 5: no attachments, no hint** — Given a Detail screen with no attachments, When it
renders, Then there is no Anexos card and no asset hint anywhere.

## Test Design

The footer string, the in-card italic hint, and the click hit-test are all deterministic and
asserted against the real TestBackend buffer / `asset_panel_cmd_at`. Geometry expectations are
derived from the **real rendered buffer** (the recurring-bug guard), never from assumed rows.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Footer drops asset hint | unit | 1 | hint_for_screen(Detail+assets) has no "Ctrl/Cmd"/"abrir anexo" | footer relocation |
| Card shows italic hint | render (TestBackend) | 2 | last interior card line contains the hint; cells have ITALIC | in-card footnote |
| Asset click still maps | unit (real buffer) | 3 | Ctrl/Cmd+click on asset k's row → Cmd::OpenAsset for k | no hit-test regression |
| Footnote not clickable | unit | 4 | Ctrl/Cmd+click on the footnote row → None | label, not target |
| Height includes hint | unit | 2 | asset_panel_render_height grew by ASSET_HINT_ROWS | geometry lock-step |

## Related

- ADR: [/adr/0027-asset-open-hint-in-card.md](/adr/0027-asset-open-hint-in-card.md)
- BDR: [/bdr/0019-asset-activation-ctrl-cmd-click.md](/bdr/0019-asset-activation-ctrl-cmd-click.md) (D1e — the prior footer hint)
- Issue: [/issues/0026-d1f-asset-hint-in-card.md](/issues/0026-d1f-asset-hint-in-card.md)
