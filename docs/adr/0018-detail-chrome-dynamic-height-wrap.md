---
type: ADR
title: Detail chrome wraps via dynamic region heights, and the task name moves off the un-wrappable frame title
description: Make the Detail user-header, footer, and assets rows word-wrap with Layout heights computed from the wrapped line count, and relocate the task-name header out of the ratatui Block title (single-line, un-wrappable) into a wrapped header line so a long name reflows instead of truncating.
status: Accepted
supersedes:
superseded_by:
tags: [tui, responsiveness, detail, wrap, layout, view]
timestamp: 2026-06-26T00:00:00Z
---

# 0018. Detail chrome — dynamic-height word-wrap; task name off the frame title

## Context

The Detail body (Description/comments) already wraps via `Paragraph::wrap` (R3/U10),
but the four chrome regions do not: the user header bar and footer are single-line
`Paragraph`s at a fixed `Constraint::Length(1)`, the Anexos/Artefatos rows are
single-line strings, and the task name was promoted to the ratatui **Block title**
(D2). On a narrow terminal all four truncate (right-clip or ellipsis) — reported
from a real session at ~40 columns.

Force: **responsiveness at narrow widths** (a [PRD 0001](/prd/0001-rust-tui-cli-parity.md)
NFR; same family as S7/U9/D1). The constraint that shapes the decision: a ratatui
**Block title is single-line and cannot wrap** — so the task-name header cannot both
stay in the frame title *and* reflow.

## Decision

Wrap each chrome region and let its Layout height grow to fit; relocate the
task-name header off the frame title.

### 1. Dynamic region heights (reuse the existing wrap helper)

Each chrome region computes its height from the wrapped line count at the current
width, using the existing `render::wrap_text` + `display_width` (no new wrap engine):

- **User header bar** (`src/tui/view.rs`): wrap `header.header_line()` to the area
  width; the top-level `Layout::vertical` first constraint becomes
  `Constraint::Length(header_height)` instead of `Length(1)`; render the wrapped
  lines as a multi-line `Paragraph`.
- **Footer** (`render_footer`): wrap the hint text; when the hint + "Updated at"
  timestamp do not co-fit, stack them; the footer constraint becomes
  `Length(footer_height)`.
- **Anexos/Artefatos** (`render_assets_panel` + `asset_panel_height`): wrap each
  `[N] ↗ <label>` row to the panel inner width with a hanging indent under the
  label; `asset_panel_height` counts wrapped rows (still capped at the existing
  max-height; overflow scrolls as today).

### 2. Task name: frame title → wrapped header line

The task name moves from the Block **title** into a **wrapped, bold header line(s)**
rendered at the top of the Detail content region (inside the border), above the
body. A single-line name occupies one line (visually close to today); a long name
wraps across lines instead of being ellipsised. The Block keeps its border; its
title becomes a short static label (or empty) that may still ellipsise harmlessly.

This **refines D2** (issue 0031, "promote task name to frame header"): the intent of
D2 — the name is the prominent Detail header — is preserved; only its *placement*
moves from the un-wrappable title to a wrappable line, because that is the only way
to honor the wrap requirement within ratatui.

### 3. Fitness function

Rendered against a ratatui `TestBackend` buffer (the TUI core stays pure — this is
presentation logic):

- At a **narrow** width each chrome element produces **multiple** lines and the full
  text is present in the buffer (no `ELLIPSIS`, no clip) — header, task name,
  footer, asset rows (BDR 0012 S1–S4, S6).
- At a **wide** width every region is single-line and header/footer height is `1`
  (no regression, no stray blank line) — BDR 0012 S5.
- A single unbreakable token hard-breaks at the display-column boundary without
  overflow (S6).

## Alternatives considered

- **Keep the task name in the Block title; just ellipsise more cleverly.** Rejected:
  ratatui Block titles are single-line; no amount of cleverness wraps them — it
  cannot satisfy the explicit "wrap to the next line" requirement.
- **Truncate the header/footer but wrap only the task name.** Rejected: the user
  asked for all four chrome elements to reflow; partial wrapping leaves the same
  clipping the report is about.
- **Horizontal scroll the chrome instead of wrapping.** Rejected: there is no
  horizontal-scroll affordance in this TUI and it hides information by default;
  wrapping shows everything within the existing vertical-scroll model.
- **A new generic wrap engine for chrome.** Rejected: `wrap_text`/`display_width`
  already do unicode-width-correct greedy wrap (used by the body) — reuse them.

## Consequences

**Positive:** at narrow widths the full identity, task name, hints, timestamp, and
artifact labels are all visible (wrapped) instead of clipped; heights reflow
automatically; reuses the proven body-wrap helpers; wide terminals are unchanged.

**Accepted trade-offs:** the header/footer regions consume more vertical space when
wrapped (less body area on very narrow/short terminals — bounded by the assets
max-height cap and the existing too-small guard from S7); the task name no longer
sits in the frame border line (a deliberate D2 placement refinement, recorded here).

## Related

- BDR: [/bdr/0012-detail-chrome-responsive-wrap.md](/bdr/0012-detail-chrome-responsive-wrap.md)
- ADR: [/adr/0014-browse-list-project-name-cache-swr.md](/adr/0014-browse-list-project-name-cache-swr.md)
- PRD: [/prd/0001-rust-tui-cli-parity.md](/prd/0001-rust-tui-cli-parity.md)
- Issue: [/issues/0017-detail-chrome-responsive-wrap.md](/issues/0017-detail-chrome-responsive-wrap.md)
- Refines: D2 (issue [/issues/0014-arch-refactor-render-decompose-relocate.md] family — task-name-as-frame-header)
