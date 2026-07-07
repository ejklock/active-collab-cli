---
type: ADR
title: Squared legend panels — replace rounded box-drawing corners crate-wide
description: Change the six box-drawing corner glyphs in text_measure.rs from rounded (╭ ╮ ╰ ╯) to squared (┌ ┐ └ ┘). Every panel, card and comment box already embeds its label in the top rule and stacks directly on the surface with one blank pad row; squaring the corners is the last step that makes them read as the crisp "legend" panels the design system specifies. One constant change flows to every screen because the glyphs are single-homed (ADR 0049/0051).
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, design-system, panels, box-drawing]
timestamp: 2026-07-06T00:00:00Z
---

# 0061. Squared legend panels

## Context

ADR 0060 recolored the TUI to the design-system tokens but explicitly left panel
*geometry* alone. The design system asks panels to read as **legend panels**:
squared corners, the label seated in the top rule, one cell of interior padding,
one blank pad row, stacked directly on the surface with no outer wrapper.

Reading the code, almost all of that is **already true**:

- `panel_box` / `panel_box_rich` (`src/render/detail_render.rs`) embed
  ` {label} ` in the top border and pad with `PANEL_HPAD = 1` / `PANEL_VPAD = 1`.
- `build_detail_content` stacks the Details panel, Description panel and Comments
  panel directly with blank separators — there is **no** outer wrapper frame.

The single remaining gap is the corner glyph: the boxes use **rounded** corners
(`╭ ╮ ╰ ╯`), which read softer and less structured than the squared legend look.

The box-drawing glyphs are single-homed in `src/render/text_measure.rs`
(ADR 0049 §box-chars, ADR 0051) and consumed by both the detail panels
(`detail_render.rs`) and the task cards (`screens/tasks.rs`). So one change flows
everywhere, keeping the whole app consistent.

## Decision

Change four constants in `src/render/text_measure.rs`:

| Const   | Was (rounded)   | Now (squared)   |
|---------|-----------------|-----------------|
| `BOX_TL`| `\u{256D}` `╭`  | `\u{250C}` `┌`  |
| `BOX_TR`| `\u{256E}` `╮`  | `\u{2510}` `┐`  |
| `BOX_BL`| `\u{2570}` `╰`  | `\u{2514}` `└`  |
| `BOX_BR`| `\u{256F}` `╯`  | `\u{2518}` `┘`  |

`BOX_H` (`─`) and `BOX_V` (`│`) are unchanged. No call site changes: every
consumer references the constants, never the literal glyph.

`box_inner_content` keeps working unchanged — it matches on the `│` vertical
border (`\u{2502}`), not the corners.

For visual consistency, the modal overlay (`src/tui/widgets/modal.rs`) should use
a squared ratatui border (`BorderType::Plain`, the default) rather than
`BorderType::Rounded` if it currently sets the latter.

## Alternatives considered

- **A dedicated `panel_style`/legend refactor.** Rejected as overkill: the legend
  structure (label-in-rule, padding, no wrapper) already exists; only the glyph
  differs. A one-constant change is the whole of it.
- **Accent + bold panel titles in the same PR.** Deferred. Styling the label
  inside the top border means routing a `StyleRun` (a new `RichStyle::PanelTitle`)
  through both `panel_box_rich` *and* the plain-string `panel_box` used by the
  Details panel, touching the strict `lines == line_styles` alignment invariants.
  Worth its own small ADR; kept out of this one so the diff stays trivially safe.

## Consequences

**Positive:** panels/cards read as crisp legend blocks matching the design system;
one-line change; whole-app consistency for free.

**Test impact:** `tests/unit/render.rs` asserts the rounded glyphs in several
places (e.g. `panel_box_top_border_starts_with_tl_and_ends_with_tr`,
`panel_box_bottom_border_has_rounded_corners`, and the outer-panel corner check
around line 1631). Update those expected glyphs from `╭ ╮ ╰ ╯` to `┌ ┐ └ ┘` and
rename `..._rounded_corners` to `..._squared_corners`. These are the only tests
that pin the corner glyph.

## Related

- ADR: [/adr/0060-design-system-semantic-tokens.md](/adr/0060-design-system-semantic-tokens.md) (the color layer this completes)
- ADR: [/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md) (single home for box glyphs)
- ADR: [/adr/0051-extract-task-layout-module.md](/adr/0051-extract-task-layout-module.md) (box glyphs shared with task cards)
- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) (panel_box vocabulary)
