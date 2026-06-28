---
type: Issue
title: "R4 — rich-text completeness: tables, strikethrough, underline, preformatted blocks"
description: Extend the richtext HTML→styled-line mapper to the remaining ActiveCollab allowed tags (table/tr/td/th, strike/del, u, pre), mirroring ActiveCollab's own toPlainText mapping with real terminal styling instead of stripping them.
status: closed
labels: [tui, render, richtext]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## R4 — rich-text completeness

Implements [ADR 0019](/adr/0019-richtext-full-activecollab-tag-coverage.md), observable
behavior pinned by [BDR 0013](/bdr/0013-richtext-full-tag-coverage.md). Traces to
research [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md),
which established ActiveCollab's allowed-tag whitelist as the target set.

### Problem

The R3 mapper (`src/richtext.rs`, ADR 0015) covers bold/italic/code, headings, lists,
blockquote, links — but strips tables, `strike`/`del`, `u`, and `pre`, which are all
inside ActiveCollab's `DEFAULT_ALLOWED_TAGS`. Real comment content using them collapses
("ainda tá zoada").

### Decision

Extend the mapper to the full whitelist, per ADR 0019: add `RichStyle::Strike` and
`RichStyle::Underline`; render `<pre>` as a verbatim whitespace-preserving code block;
render `<table>` as column-aligned text rows with bold `<th>`. Mirror ActiveCollab's
`toPlainText` structure with real styling. Mapper stays pure; never panics on malformed
input.

### Scope

Included: `src/richtext.rs` (new styles, `<pre>` handling, table builder + alignment),
the rendering layer that maps `RichStyle` → ratatui `Modifier` (strike → `CROSSED_OUT`,
underline → `UNDERLINED`) in `src/render.rs`, and `tests/unit/richtext.rs` fixtures.
Excluded: links (V5, issue 0020), selection (V6, issue 0021); CLI plain-text path
(unchanged); a real ratatui `Table` widget (rejected in ADR 0019).

### Acceptance

- `<del>`/`<strike>` → strikethrough span; `<u>` → underline span (BDR 0013 Sc. 1–2).
- `<pre>` preserves internal whitespace and newlines, code-styled, framed by blank
  lines; inline emphasis inside still applies (Sc. 3).
- `<table>` renders column-aligned rows (display-width padding, two-space gutter),
  `<th>` bold, framed by blank lines; ragged/degenerate tables never panic (Sc. 4–5).
- CLI `html_to_text` output unchanged (Sc. 6); styles survive wrapping (Sc. 7).
- Full suite green; clippy `-D warnings`, fmt, comment-policy clean; complexity within
  budget; tests assert observable styled segments / line structure (mutation-resistant).

### Plan

Single slice (R4). 1) Add `Strike`/`Underline` to `RichStyle` + map in render. 2) Add
`strike`/`del`/`u` to `process_tag_rich` emphasis handling. 3) Add `<pre>` verbatim
context. 4) Add table context (collect rows/cells) + a pure column-alignment helper
within the complexity budget. 5) Fixtures for each in `tests/unit/richtext.rs`.
