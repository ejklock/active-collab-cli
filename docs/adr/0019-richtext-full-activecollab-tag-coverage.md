---
type: ADR
title: Extend the rich-text mapper to the full ActiveCollab allowed-tag set
description: Grow the richtext HTML→styled-line mapper (ADR 0015) to cover the remaining tags ActiveCollab permits — tables, strike/del, underline, and pre/code-block — mirroring ActiveCollab's own toPlainText mapping with real terminal styling instead of stripping them.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, ratatui, html, richtext]
timestamp: 2026-06-26T00:00:00Z
---

# 0019. Rich-text: cover the full ActiveCollab allowed-tag set

## Context

[ADR 0015](/adr/0015-richtext-html-subset-styled-segments.md) (slice R3) introduced
a styled `richtext` mapper over a *fixed subset* of HTML: bold, italic, code,
headings, lists, blockquote, links. It deliberately left "exotic markup" to degrade
to stripped text.

The operator reports the detail body "ainda tá zoada" — still wrong. Research
([/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md))
established the authoritative cause: ActiveCollab's canonical allowed-tag whitelist
(`HtmlCleanerInterface::DEFAULT_ALLOWED_TAGS`) permits tags the current mapper does
**not** handle, so real comment content using them collapses:

- **tables** (`table`/`thead`/`tbody`/`tfoot`/`tr`/`td`/`th`) — today only `<tr>`
  becomes a newline; cells and structure are lost.
- **strikethrough** (`strike`, `del`) — stripped, so struck text reads as normal.
- **underline** (`u`) — stripped.
- **preformatted** (`pre`) — stripped, so code blocks lose their monospace/verbatim
  intent and internal whitespace.

The "exotic markup degrades" assumption from ADR 0015 was too broad: these tags are
**inside** the whitelist, i.e. authors really do produce them. The gap is exactly the
delta between ADR 0015's subset and ActiveCollab's whitelist.

Force: **fidelity of a read view** (same force as ADR 0015). The pure TEA core is
untouched; this is the rendering layer.

## Decision

Extend the `richtext` mapper (`src/richtext.rs`) to the **full ActiveCollab allowed
tag set**, mirroring ActiveCollab's own `Angie\HTML::toPlainText()` mapping but with
real terminal styling. Delivered as slice **R4**.

### 1. New emphasis styles

`RichStyle` gains `Strike` and `Underline` variants (joining `Plain`, `Bold`,
`Italic`, `Code`). The rendering layer maps them to ratatui modifiers
(`CROSSED_OUT`, `UNDERLINED`). Wrapping stays style-aware (carry the style across a
wrap), as already required by ADR 0015.

| HTML | Rendered as |
|---|---|
| `<strike>`, `<del>` | strikethrough span |
| `<u>` | underlined span |

### 2. Preformatted blocks

`<pre>` opens a verbatim block: inner text is emitted line-for-line with internal
whitespace preserved (no entity-driven collapsing of runs), each line styled `Code`.
Nested inline tags inside `<pre>` still apply. A `<pre>` block is framed by a blank
line above and below, like headings/blockquotes.

### 3. Tables

A `<table>` renders as aligned text rows, mirroring `toPlainText` but readable:

- Each `<tr>` is one logical line; `<td>`/`<th>` are the cells.
- Cells are **column-aligned** by padding each column to the widest cell in that
  column (computed in display-width, the metric ratatui uses), separated by a
  two-space gutter.
- `<th>` cells render **bold**.
- The table is framed by a blank line above and below.
- Degenerate tables (a single cell, ragged rows, missing `tbody`) never panic — a
  missing cell is treated as empty; rows define the column count by their own cells.

The table builder is a small pure helper (within the complexity budget), not an
extension of any existing god function, consistent with
[ADR 0016](/adr/0016-refactor-render-decompose-relocate.md).

### 4. Robustness unchanged

Malformed/partial HTML still never panics; genuinely unknown tags (outside the
whitelist) still degrade to stripped text. The mapper stays pure and unit-tested on
representative fixtures (table, struck text, underline, `<pre>` with whitespace,
mixed), per ADR 0015's testing discipline.

## Alternatives considered

- **Adopt an HTML→text/markdown crate** (`tui-markdown`, `ansi-to-tui`, `html2text`).
  Rejected — research 0001 found none target ratatui `Style` spans from HTML; each
  needs a lossy pre-pass and post-processing. The in-house mapper already exists and
  gives direct styling control.
- **Render tables as a real ratatui `Table` widget.** Rejected: the body is a single
  flowing `Paragraph` of styled lines with style-aware wrapping; embedding a widget
  mid-paragraph breaks the unified scroll/wrap/selection model. Text-aligned rows
  keep one rendering path.
- **Keep degrading these tags (status quo).** Rejected — it is the reported defect;
  the tags are inside ActiveCollab's whitelist, so they occur in real content.

## Consequences

**Positive:** the detail body renders the full range of formatting ActiveCollab can
store — struck/underlined text, code blocks with preserved whitespace, and readable
column-aligned tables — closing the "ainda tá zoada" gap. Mapper stays pure and
headless-testable.

**Accepted trade-offs:** two new `RichStyle` variants and a table builder; the
rendering layer grows two modifier mappings. Table alignment is best-effort text
layout, not a pixel-perfect grid (acceptable for a read view). One mapper, one
render path preserved.

## Related

- ADR: [/adr/0015-richtext-html-subset-styled-segments.md](/adr/0015-richtext-html-subset-styled-segments.md) (extended here)
- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md)
- BDR: [/bdr/0013-richtext-full-tag-coverage.md](/bdr/0013-richtext-full-tag-coverage.md)
- Research: [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md)
- Issue: [/issues/0019-r4-richtext-full-tag-coverage.md](/issues/0019-r4-richtext-full-tag-coverage.md)
- Architecture: [/architecture.md](/architecture.md)
