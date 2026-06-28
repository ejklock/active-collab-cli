---
type: Issue
title: "D1e — open assets via Ctrl/Cmd+click; remove the numeric 1-9 open and d+1-9 download shortcuts"
description: Replace the numeric asset open (1-9) and download (d+1-9) keyboard shortcuts with Ctrl/Cmd+click activation consistent with D1c body links. Plain click does not open (reserved for selection). Remove pending_download, Msg::AssetOpen/TogglePendingDownload, digit_to_asset_index, the Cmd::DownloadAsset variant, and the numeric footer hints.
status: closed
labels: [tui, ux, mouse, assets, keybindings]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## D1e — asset activation via Ctrl/Cmd+click

Implements [ADR 0025](/adr/0025-asset-activation-ctrl-cmd-click.md), observable behavior
pinned by [BDR 0019](/bdr/0019-asset-activation-ctrl-cmd-click.md). Retires the numeric
asset shortcuts from [ADR 0009](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) §5
([issue 0008](/issues/0008-r7-i18n-assets.md)). Detail-polish family (D1a–D1d shipped).

### Problem

Numeric `1`–`9` open and `d`+`1`–`9` download cannot address a tenth asset and hide a
download mode. The operator asked to remove the numeric logic (including the footer) and
open assets by click instead.

### Decision

Open an asset on **Ctrl/Cmd/Super+left-click** (reusing the D1d `asset_index_at_panel_row`
mapping and the D1c modifier gate); a plain click does not open (reserved for V6 selection).
Remove the numeric scheme and its state entirely; drop in-TUI download (numeric-only).

### Scope

Included: `src/tui/events.rs` (remove the digit and `d` key arms), `src/tui/model.rs`
(remove `pending_download`, `Msg::AssetOpen`, `Msg::TogglePendingDownload`,
`handle_asset_open`, `handle_toggle_pending_download`, `digit_to_asset_index`, the
`Cmd::DownloadAsset` variant; gate `asset_panel_cmd_at` on Ctrl/Cmd and always emit
`OpenAsset`; block the unmodified asset-open path in `handle_click_detail`),
`src/tui/mod.rs` (remove the `DownloadAsset` shell arm), `src/tui/view.rs` (reword the
detail footer hint), `locales/pt_BR.json` (reword the footer strings), and the affected
`tests/unit/*`. Excluded: the D1d card layout (unchanged); body links (D1c, unchanged); a
replacement download gesture (deferred per ADR 0025).

### Acceptance

- Ctrl/Cmd/Super+click on asset row [n] emits `Cmd::OpenAsset` with asset n's URL; works for
  the tenth asset (BDR 0019 Sc.1, Sc.6).
- A plain unmodified click on an asset row emits no open/download Cmd (Sc.2).
- Pressing `1`–`9` opens nothing; `d`+digit enters no download mode; `pending_download`,
  `Msg::AssetOpen`, `Msg::TogglePendingDownload`, `digit_to_asset_index`, and
  `Cmd::DownloadAsset` are removed (Sc.3, Sc.4).
- The detail footer no longer shows `1-9 open asset` / `d+1-9 download` (or pt-BR
  equivalents) (Sc.5).
- Full suite green; clippy `-D warnings` (no dead-code/unused-variant), fmt, comment-policy
  clean; complexity within budget; the click/keyboard tests are mutation-resistant.

### Plan

Single slice (D1e). 1) Remove the digit + `d` key arms in events.rs. 2) Remove the numeric
messages, handlers, `digit_to_asset_index`, `pending_download`, and `Cmd::DownloadAsset` in
model.rs; remove the `DownloadAsset` arm in mod.rs. 3) Gate `asset_panel_cmd_at` on
Ctrl/Cmd and always emit `OpenAsset`; block the unmodified asset-open call in
`handle_click_detail`. 4) Reword the footer hint in view.rs + locales/pt_BR.json. 5) Replace
the numeric/pending_download tests with Ctrl/Cmd+click-opens, plain-click-no-open, and a
tenth-asset test.
