---
type: ADR
title: Open assets with Ctrl/Cmd+click; retire the numeric 1-9 open and d+1-9 download shortcuts
description: Replace the numeric keyboard shortcuts for opening (1-9) and downloading (d then 1-9) detail assets with Ctrl/Cmd+click activation, consistent with the D1c body-link gate. The numeric scheme cannot address a tenth asset and its download-prefix mode is invisible; a pointer gesture scales to any count and matches how body links already open.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, crossterm, mouse, assets, keybindings]
timestamp: 2026-06-27T00:00:00Z
---

# 0025. Asset activation via Ctrl/Cmd+click; numeric shortcuts retired

## Context

The detail view's Anexos/Artefatos assets were actionable through numeric keyboard
shortcuts introduced with the visual redesign ([ADR 0009](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) §5):
pressing `1`–`9` opened the n-th asset, and a `d` prefix followed by `1`–`9` downloaded
it (a transient `pending_download` mode on the `Screen::Detail` model).

The operator, dogfooding a task with many assets, hit the structural limit directly:
*"pode remover essa lógica de abrir anexo com número (inclusive do footer) pq se tiver
mais de 9 anexos não vai dar certo."* The numeric scheme **cannot reference a tenth
asset** — there is no `10` key — and the `d`-prefix download mode is invisible (the
operator cannot see they are in download mode). Meanwhile body links already adopted a
clear, count-independent gesture: **Ctrl/Cmd+click** ([ADR 0020](/adr/0020-body-links-inline-url-native-click.md) §2a,
[BDR 0014](/bdr/0014-body-link-inline-url-activation.md)), with plain click reserved for
text selection ([V6](/adr/0021-app-managed-text-selection-clipboard.md)).

Force: **an actionable list must scale to its real cardinality** and stay consistent with
the rest of the detail view's pointer model. A 1-9 keymap fails both; a pointer gesture
satisfies both.

## Decision

Adopt **Ctrl/Cmd+click to open an asset**, retiring the numeric shortcuts. This amends the
numeric-shortcut portion of ADR 0009 §5 (the rest of ADR 0009's redesign stands).

### 1. Open via Ctrl/Cmd+click

A **Ctrl/Cmd/Super+left-click** on an asset row emits `Cmd::OpenAsset` for that asset. The
click→asset mapping reuses the D1d `asset_index_at_panel_row` helper, so it resolves to the
correct asset regardless of count. A **plain (unmodified) click does not open** an asset —
it is reserved for text selection (V6), exactly as a plain click on a body link does not
open it (D1c). This makes the asset card and the body links share **one** pointer model:
plain = select, Ctrl/Cmd = activate.

### 2. Remove the numeric scheme entirely

The `1`–`9` open key, the `d` download prefix, the `pending_download` model field, the
`Msg::AssetOpen` / `Msg::TogglePendingDownload` messages and their update handlers, and the
`digit_to_asset_index` helper are all removed. The numeric footer hints
(`1-9 open asset  d+1-9 download` / `1-9 abrir anexo  d+1-9 baixar`) are reworded to drop
the numeric affordances.

### 3. Download by numeric shortcut is dropped (not replaced)

Because download existed **only** through the `d`+number prefix, removing the numeric scheme
removes the in-TUI download gesture; the `Cmd::DownloadAsset` variant and its shell
interpretation are removed to avoid dead code. Opening an asset in the browser via
Ctrl/Cmd+click covers the operator's stated need. A dedicated download gesture can be added
later if it is requested — recorded here so the omission is a deliberate, visible choice.

## Alternatives considered

- **Keep numeric, page beyond 9 (e.g. `1`–`9` then a "more" key).** Rejected: still a
  keymap that fights its own cardinality and adds a paging mode for a list that is already
  on screen and clickable.
- **Plain click opens the asset.** Rejected: collides with V6 text selection and with the
  D1c rule that plain click does not activate — the detail view would then have two
  contradictory plain-click meanings.
- **Keep download on a new modifier (e.g. Ctrl/Cmd+Shift+click).** Deferred: the operator
  asked to remove numeric download, not to relocate it; adding a second asset gesture now is
  speculative. Left as an explicit future option.

## Consequences

**Positive:** asset activation scales to any number of assets; the asset card and body
links share one pointer model (plain = select, Ctrl/Cmd = activate); the invisible
download-mode and the dead 1-9 keymap are gone; less model state (`pending_download`
removed) and one fewer `Cmd` variant.

**Accepted trade-offs:** in-TUI download is removed (numeric-only, now dropped) until a
gesture is requested; opening requires a modifier the operator must know (the same one D1c
already established, and the footer hint advertises it). Terminals that intercept
Ctrl/Cmd+click are subject to the same caveat already noted for D1c body links.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) (§5 numeric asset shortcuts, retired here)
- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (§2a Ctrl/Cmd+click precedent)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md) (plain click reserved for selection)
- ADR: [/adr/0024-asset-card-breathing-room.md](/adr/0024-asset-card-breathing-room.md) (the card + asset_index_at_panel_row mapping)
- BDR: [/bdr/0019-asset-activation-ctrl-cmd-click.md](/bdr/0019-asset-activation-ctrl-cmd-click.md)
- Issue: [/issues/0024-d1e-asset-activation-ctrl-cmd-click.md](/issues/0024-d1e-asset-activation-ctrl-cmd-click.md)
