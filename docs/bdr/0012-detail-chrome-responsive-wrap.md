---
type: BDR
title: "Detail chrome responsiveness: header, task title, footer, and artifacts wrap on narrow widths"
description: On a narrow terminal the Detail screen's chrome — the user header bar, the task-name header, the status/hint footer, and the Anexos/Artefatos block — wraps text to the next line instead of truncating with an ellipsis. Region heights grow to fit the wrapped lines. Includes the Test Design matrix.
status: Accepted
supersedes:
superseded_by:
tags: [tui, responsiveness, detail, wrap, view, behavior]
timestamp: 2026-06-26T00:00:00Z
---

# 0012. Detail chrome responsiveness — wrap instead of truncate

Realizes [ADR 0018](/adr/0018-detail-chrome-dynamic-height-wrap.md). Extends the
responsiveness line ([S7], U9 responsive name, D1 border-by-width) to the four
Detail "chrome" regions the body already handles (the Description/comment body
wraps via `Paragraph::wrap`, R3/U10). The data and colors are unchanged — only the
layout reflows.

## Context

On a narrow width the Detail chrome truncates (right-clips or ellipsis):

- the **user header bar** (`{name} <{email}> · {instance}`) — single-line
  `Paragraph` (view.rs), silently right-clipped;
- the **task-name header** — promoted to the ratatui Block **title** (D2), which is
  single-line by design and clipped with an ellipsis;
- the **status/hint footer** (`↑/↓ scroll  r refresh … · Updated at …`) — two
  single-line paragraphs;
- the **Anexos/Artefatos** rows (`[N] ↗ <label>`) — single-line, truncated.

## Definitions

- **Wrap** — greedy word-wrap to the available display-columns (reusing
  `render::wrap_text`/`display_width`), breaking to the next line; never clip with
  an ellipsis unless a single unbreakable token exceeds the width.
- **Dynamic region height** — the region's Layout `Constraint::Length` is computed
  from the wrapped line count for the current width, not a fixed `1`.

## Scenarios

### S1 — User header bar wraps and grows
**Given** a width too narrow for `{name} <{email}> · {instance}` on one line,
**When** the Detail (or any) screen renders,
**Then** the header text wraps to as many lines as needed (the header region height
grows to match), and no part of the identity is clipped.

### S2 — Task-name header wraps
**Given** a task name wider than the inner content width,
**When** the Detail screen renders,
**Then** the full task name is shown wrapped across lines (no ellipsis); a
single-line name still renders on one line.

### S3 — Footer wraps and grows
**Given** the hint text plus the "Updated at" timestamp do not fit on one line,
**When** the footer renders,
**Then** the footer wraps to multiple lines (the footer region height grows), and
neither the hints nor the timestamp is clipped.

### S4 — Anexos/Artefatos rows wrap
**Given** an artifact label wider than the panel inner width,
**When** the assets panel renders,
**Then** the `[N] ↗ <label>` row wraps to the next line (continuation lines aligned
under the label), and the panel height accounts for the wrapped rows (within its
existing max-height cap).

### S5 — Wide terminal is unchanged
**Given** a width that fits each element on one line,
**When** rendering,
**Then** every chrome region renders exactly as today (single line, height 1 for
header/footer), with no extra blank lines.

### S6 — A single unbreakable token still fits gracefully
**Given** a token with no break opportunity longer than the width (e.g. a long URL
slug label),
**When** rendering,
**Then** it is hard-broken at the width boundary (display-column safe) rather than
overflowing the region.

## Test Design

| Scenario | Level | Technique | Instrument / assertion |
|---|---|---|---|
| S1 | unit (render) | example | header wrap helper yields N lines for a narrow width; header region height == N; TestBackend buffer shows the full identity across rows |
| S2 | unit (render) | example | the task-name header builder returns wrapped lines (no ELLIPSIS) when name width > inner width; one line when it fits |
| S3 | unit (render) | example | footer builder returns >1 line when hint+timestamp exceed width; footer height grows; both substrings present in the buffer |
| S4 | unit (render) | example | an asset row wider than inner width wraps to 2 lines; `asset_panel_height` counts wrapped rows; continuation indent asserted |
| S5 | unit (render) | example | at a wide width each region height is 1 (header/footer) and no element wraps (golden single-line buffer) |
| S6 | unit (render) | boundary | an unbreakable token longer than width hard-breaks at the display-column boundary, no overflow panic |

All scenarios are rendered against a ratatui TestBackend buffer (the TUI core stays
pure; this is presentation logic). No browser/web involved — Detail is a terminal
view, asserted via buffer, not a browser gate.

## References

- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md)
- BDR: [/bdr/0009-richtext-formatting-detail-view.md](/bdr/0009-richtext-formatting-detail-view.md)
- Issue: [/issues/0017-detail-chrome-responsive-wrap.md](/issues/0017-detail-chrome-responsive-wrap.md)
