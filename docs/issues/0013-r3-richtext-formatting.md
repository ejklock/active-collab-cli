---
type: Issue
title: "R3 — preserve comment/description rich-text via an HTML-subset styled mapper"
description: Map a fixed HTML subset (bold, italic, code, headings, lists, blockquote, links) to ratatui styled segments in the detail view; CLI plain-text path unchanged.
status: open
labels: [tui, render, html, richtext]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## R3 — rich-text formatting in the detail view

Stop flattening comments/descriptions to one paragraph. Implements
[ADR 0015](/adr/0015-richtext-html-subset-styled-segments.md); pins
[BDR 0009](/bdr/0009-richtext-formatting-detail-view.md).

### Scope

Included: a focused `richtext` mapper (HTML subset → styled lines) feeding the
existing `styled_line`/`LinkSegment` render; style-aware wrapping; reuse of the V4
`↗ Link N` label for `<a>`. Excluded: a full HTML/CSS renderer; any change to the
CLI/non-TTY `html_to_text` plain path (kept for BDR 0003 parity).

### Acceptance

- Inline `<strong>/<b>`, `<em>/<i>`, `<code>` carry bold/italic/dim styles
  (BDR 0009 S1).
- `<ul>/<li>` → `• ` lines; `<ol>/<li>` → `N. ` lines (S2–S3).
- `<h1>`–`<h6>` → bold lines; `<blockquote>` → `> ` prefix (S4–S5).
- `<a href>` → `↗ Link N` label resolving to the URL (S6).
- Malformed HTML degrades to stripped text, no panic (S7).
- CLI path output is byte-for-byte unchanged (S8).
- A bold span keeps its style across a wrap (S9).
- Pure unit tests on representative HTML fixtures for every row of the matrix.

### Plan

Per ADR 0015: add the `richtext` module (kept small per ADR 0016); extend the
line builders to carry styles; make `wrap_text` style-aware. Coordinate with R3's
sibling slice ARCH so the new module lands in its final shape.
