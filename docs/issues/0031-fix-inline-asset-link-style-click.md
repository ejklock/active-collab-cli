---
type: Issue
title: "Fix inline Anexos/Artefatos rows rendering as plain text (no link style) and reading as not clickable"
description: Regression from the derived asset label (ADR 0023, commit 3a961d6). The inline asset row `[n] ↗ {label}` no longer contains a URL token, so link_segments() finds nothing and styled_line_with_runs applies no link style — asset rows render plain and read as not clickable. Fix structurally per ADR 0032 (emit a RichStyle::Link run from the layout); add the missing end-to-end Ctrl/Cmd+click open test.
status: closed
labels: [tui, render, assets, link, regression, bug]
blocked_by:
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Fix inline asset rows: restore link styling + prove click-to-open

Implements [ADR 0032](/adr/0032-asset-row-link-style-structural.md). Restores the
behavior specified by [BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md)
Scenario 1 ("the asset token is visibly link-styled" + "Ctrl/Cmd+click opens it").

### Problem (reported)

In the detail view, the Anexos/Artefatos items "estão todos sem formato de link e não
clicáveis" — they render as plain text (no muted-green/underline link affordance) and read
as not clickable.

### Root cause

Asset-row link styling was implicit, derived from the rendered text by
`link_segments()` (`src/render.rs:304`) → `styled_line_with_runs`
(`src/tui/screens/detail.rs:96`) applying `theme::link_style()` to any detected URL/email
token. [ADR 0023](/adr/0023-asset-label-derivation.md) (commit `3a961d6`) made the asset
label derived (anchor text / filename / host), so the row is now `[1] ↗ report.pdf` with
**no** URL token and the real `asset.url` absent from the visible text. `link_segments`
finds nothing → no link style. The Ctrl/Cmd+click hit-test (`asset_panel_cmd_at`,
`src/tui/model.rs`) is structurally intact; the row only *reads* as unclickable because it
carries no visible link affordance.

### Decision

Style asset rows structurally (ADR 0032): add `RichStyle::Link`, map it to
`theme::link_style()`, and emit a `Link` `StyleRun` over the asset token in
`section_lines()` (`src/tui/screens/asset_panel.rs`) — mirroring how the `Hint` row emits
an `Italic` run. Body-link text-detection (ADR 0020) is unchanged.

### Scope

Included:

- `src/richtext.rs` — add `RichStyle::Link`.
- `src/render.rs` (or wherever `RichStyle` → ratatui `Style` for content runs lives) — map
  `RichStyle::Link → theme::link_style()`; ensure `split_segment_by_runs` applies it.
- `src/tui/screens/asset_panel.rs` — `section_lines()` emits a `Link` `StyleRun` over each
  `PanelRow::Asset` token; remove the empty-run vec and the inaccurate "applied at render
  time" doc comments.
- `tests/unit/tui_render.rs` — assert an inline asset row's cells carry link_style
  (fg muted green + UNDERLINED) over the asset token (buffer-derived).
- `tests/unit/model.rs` — assert Ctrl/Cmd+click on an inline asset content row emits
  `Cmd::OpenAsset` with the correct url (the missing end-to-end test).

Excluded: body-link styling/click (ADR 0020, unchanged); the label-derivation logic
(ADR 0023, correct — only its styling consequence is fixed); any change to the click
hit-test logic (verified, not modified).

### Acceptance

- An inline asset row renders with `theme::link_style()` (muted green + underline) over the
  `[n] ↗ {label}` token, for any label (filename, host, anchor text) — not dependent on a
  URL appearing in the visible text.
- A Ctrl/Cmd+click on an inline asset content row emits `Cmd::OpenAsset` with that asset's
  url (end-to-end test, previously absent).
- The italic "Ctrl/Cmd+click to open" hint and the asset/header/separator geometry are
  unchanged; body-link styling and click stay green.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean;
  complexity within budget; tests mutation-resistant (buffer/value-derived assertions).

### Plan

Single vertical slice: add the variant + mapping, emit the run, prove style + click with
the two new tests. Observable end-to-end (open the detail view, the attachment reads as a
link and opens on Ctrl/Cmd+click).

### Resolution

Closed by commit `eff0b97` (implementation) on top of `1c0eb37` (docs trail). Pipeline:
Coder → quality-gate → Reviewer (opus), approved.

- **Fix.** `RichStyle::Link` added (`src/richtext.rs`), mapped to `theme::link_style()` at
  the single emphasis-style site (`src/tui/screens/detail.rs` `emphasis_style()`).
  `section_lines()` (`src/tui/screens/asset_panel.rs`) now emits a `Link` `StyleRun`
  (`start = PANEL_HPAD`, `len = display_width(text)`) per `PanelRow::Asset`, mirroring the
  `Hint` italic run; the inaccurate "styling applied at render time" doc comments were
  replaced with the real mechanism. `link_segments` / body-link detection unchanged.
- **Tests.** `tests/unit/tui_render.rs` asserts (buffer-derived) the asset-token cells
  carry muted-green fg + UNDERLINED for a non-URL label (`report.pdf`), pinning the
  structural regression; hint stays italic-not-green, separator unstyled.
  `tests/unit/model.rs` drives `update()` with a Ctrl+click on an asset content row →
  `Cmd::OpenAsset{url}`, and an unmodified click on the same row → no open (the Ctrl/Cmd
  gate). Two pre-ADR-0032 tests asserting empty asset runs were updated to assert the Link
  run.
- **Diagnosis correction.** The reported "not clickable" was the missing visual affordance,
  not a broken hit-test: `asset_panel_cmd_at` was structurally intact (now proven by the
  end-to-end test).
- **Gates.** Dev container authoritative: 875 tests + 31 comment_policy green, clippy
  `--all-targets -D warnings` clean, fmt clean. Reviewer confirmed all three mutants
  (Link mapping, the emitted run, the Ctrl/Cmd gate) are killed by the new tests.
- **Residue (non-blocking).** One test retains a stale `_no_styles` name suffix though it
  now asserts a Link run — cosmetic only.
