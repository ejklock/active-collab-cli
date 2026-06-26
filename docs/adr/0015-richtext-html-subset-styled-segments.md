---
type: ADR
title: Render comment/description HTML as styled segments over a known tag subset
description: Replace the flatten-everything html_to_text path in the TUI detail render with a small HTML→styled-segment mapper that preserves a fixed subset of formatting (bold, italic, code, headings, lists, blockquote, links) as ratatui styles; CLI plain-text output keeps html_to_text for parity.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, ratatui, html, richtext]
timestamp: 2026-06-26T00:00:00Z
---

# 0015. Rich-text: map an HTML tag subset to styled segments

## Context

Comments and task descriptions arrive as **HTML** from ActiveCollab. The TUI
detail render flattens them through `html_to_text` (`src/render.rs`): block tags
(`br`, `p`, `div`, `li`, `tr`, `h1`–`h6`) become newlines, **every other tag is
stripped**, entities are decoded. So `<strong>`, `<em>`, `<code>`, `<a>`,
`<blockquote>`, and list bullets all collapse into one undifferentiated
paragraph.

The user observed that "algumas ficam certas mas outras nao" — comments that are
plain paragraphs look fine; comments authored with lists, emphasis, headings, or
nested markup render as a flat blob, losing the structure that made them readable.

Force: **fidelity of a read view.** The detail screen is where the operator reads
a task; structure (bullets, emphasis, headings) is meaning, not decoration. This
is a presentation concern — the pure TEA core is untouched; the change is in the
rendering layer that builds the detail lines.

## Decision

Introduce a **styled rich-text mapper** over a fixed HTML subset, delivered as
slice **R3**, feeding the existing styled-line render path (`styled_line` /
`LinkSegment` from V4).

### 1. Tag → style mapping (the subset)

| HTML | Rendered as |
|---|---|
| `<strong>`, `<b>` | bold span |
| `<em>`, `<i>` | italic span |
| `<code>` | dim / distinct style span |
| `<h1>`–`<h6>` | bold line |
| `<ul>`/`<ol>` + `<li>` | one line per item, `• ` prefix (ordered: `N. `) |
| `<blockquote>` | `> ` line prefix |
| `<a href>` | the existing `↗ Link N` label ([V4](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) link handling) |
| `<p>`, `<div>`, `<br>` | line breaks (current behavior) |
| any other tag | stripped (current behavior) |

The mapper emits a `Vec` of styled **lines**, each a sequence of
`{ text, style }` segments, so wrapping (`wrap_text`) and the link-label collector
continue to operate on text while styles ride along. Output flows into the
existing `styled_line` renderer; no new widget.

### 2. Where it lives

A focused **`richtext` module/function** — not an extension of `html_to_text`.
`html_to_text` stays the plain-text flattener for the **CLI/non-TTY** path
(`render_task_to_str`, `get`/`current` output), preserving
[BDR 0003](/bdr/0003-cli-command-output-parity.md) parity. Keeping the new mapper
separate and small also respects [ADR 0016](/adr/0016-refactor-render-decompose-relocate.md)
(no new god function in `render.rs`).

### 3. Robustness

Malformed/partial HTML must never panic: unknown or unbalanced tags degrade to
stripped text (the current safe behavior). The mapper is pure and unit-tested on
representative fixtures (list, nested emphasis, heading, blockquote, link, mixed).

## Alternatives considered

- **Full HTML/CSS rendering.** Rejected: there is no terminal CSS; the cost is
  enormous for a read view that needs a handful of inline styles.
- **HTML → Markdown round-trip, then render Markdown.** Rejected: adds a lossy
  conversion and a second grammar; we would still need a Markdown→styled-segment
  step. Mapping the HTML subset directly is fewer moving parts.
- **Adopt a crate (e.g. `html2text`).** Considered. Rejected for now: it targets
  plain-text reflow, not ratatui `Style` segments, so we would post-process its
  output anyway; a tiny in-house subset mapper gives direct control over terminal
  styling and avoids a heavy dependency. Revisit if the subset grows.
- **Keep flattening (status quo).** Rejected — it is the defect the user reported.

## Consequences

**Positive:** comments and descriptions keep their structure — bullets, emphasis,
headings, quotes — making the detail view readable as authored; links continue to
use the V4 label; the CLI path is unchanged (parity preserved). The mapper is pure
and headless-testable.

**Accepted trade-offs:** a fixed subset means exotic markup still degrades to
stripped text (acceptable — the subset covers what comment authors actually use).
The segment model adds a style dimension to the line builders; wrapping must be
style-aware (carry the style across a wrap). One new small module.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md)
- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md)
- BDR: [/bdr/0009-richtext-formatting-detail-view.md](/bdr/0009-richtext-formatting-detail-view.md)
- Issue: [/issues/0013-r3-richtext-formatting.md](/issues/0013-r3-richtext-formatting.md)
- Architecture: [/architecture.md](/architecture.md)
