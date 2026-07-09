---
type: ADR
title: Styled panel titles — accent + bold labels in the top rule
description: Give the Details / Description / Comments panel titles the design-system treatment (accent + bold) by carrying a new RichStyle::PanelTitle run over the label span of each panel's top-border row. This reuses the exact mechanism that already styles the [edit]/[excluir] affordance tokens on a comment card's border row (ADR 0041), so it threads through the existing StyleRun pipeline with no new rendering path and no change to the strict lines == line_styles alignment. Comment-card headers are intentionally left as-is.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, design-system, panels, richtext]
timestamp: 2026-07-06T00:00:00Z
---

# 0063. Styled panel titles

## Context

ADR 0060 (tokens) and ADR 0061 (squared corners) leave one design-system detail
unfinished: the panel **titles** — `Details`, `Description`, `Comments (n)` —
render in the default body color, seated in the top rule but visually flat. The
design system asks them to be **accent + bold**, the same treatment as the modal
title, so a screen's structure reads at a glance.

The label already lives in the top-border row, built by `panel_box`
(`src/render/detail_render.rs`). The obstacle noted in ADR 0061 was that styling
it means routing a style run over a *border* row while preserving the strict
`lines == line_styles` alignment and the run pipeline.

Reading the code, that obstacle is already solved for a sibling case: a comment
card's `[editar]`/`[excluir]` affordance tokens are styled by pushing
`RichStyle::EditAffordance` / `DeleteAffordance` runs onto the **border row's**
`line_styles[0]` (`push_affordance_style_runs`, ADR 0041). Styling a panel title
is the same move with a different span and style.

## Decision

### 1. New emphasis variant — `RichStyle::PanelTitle` (`src/richtext.rs`)

Add `PanelTitle` to the `RichStyle` enum, alongside `Link` / `EditAffordance` /
`DeleteAffordance`. It is layout-emitted (never produced by the HTML parser), so
no parser arm changes.

### 2. Map it to `theme::panel_title_style()` (`src/tui/screens/detail.rs`)

Add one arm to `emphasis_style`: `RichStyle::PanelTitle => theme::panel_title_style()`
(accent + bold; already added to `theme.rs` in the ADR 0060 handoff). No change to
`emphasis_at_col` or `styled_line_with_runs` — a run over the border row is
rendered exactly like the affordance runs already are.

### 3. Emit the run over the label span (`src/render/detail_render.rs`)

Extract the header-fitting arithmetic already inside `panel_box` into
`fit_panel_header(label, width) -> (String, usize)` (fitted string + its display
width), and reuse it both in `panel_box` and in a new
`panel_title_run(label, width) -> Option<StyleRun>` that returns a `PanelTitle`
run at `start = 2` (past `BOX_TL` + `BOX_H`) with `len =` the fitted label width.

Apply it explicitly at the **three section panels only**, pushing onto
`line_styles[0]`:

- **Description** — in `build_body_lines`, after `unzip_boxed`.
- **Comments** (outer panel) — in `build_comment_lines`, after
  `merge_nested_styles_into_outer`.
- **Details** — in `build_detail_content`, on the header block's first row
  (currently seeded with `repeat_n(vec![], header_count)`).

Applying it at the `build_*` sites — not inside `panel_box_rich` — is deliberate:
`panel_box_rich` is also what renders each **comment card**, and those headers
already carry affordance runs. Keeping the title run out of the shared primitive
leaves comment-card headers exactly as they are today.

## Alternatives considered

- **Bake the run into `panel_box_rich`.** Rejected: it would also restyle every
  comment-card header (accent+bold author·date), overlapping the existing
  affordance runs on the same row. Out of scope; would need its own decision.
- **A separate render pass that recolors border rows by regex/label match.**
  Rejected — fragile text matching for something the layout already knows
  structurally.
- **Duplicate the header-fit arithmetic in `panel_title_run`.** Rejected in favor
  of extracting `fit_panel_header` so the run width can never drift from what
  `panel_box` actually draws.

## Consequences

**Positive:** panel titles match the design system; reuses the proven affordance
run pipeline; no new render path; alignment invariant untouched; comment cards
unaffected.

**Test impact:** add a case asserting `line_styles[0]` of each section panel
carries a `PanelTitle` run at the fitted-label span; `fit_panel_header`'s
extraction is covered by the existing `panel_box` width tests. If any test asserts
the Description/Details border row has *empty* styles, update it.

## Related

- ADR: [/adr/0060-design-system-semantic-tokens.md](/adr/0060-design-system-semantic-tokens.md) (defines `panel_title_style`)
- ADR: [/adr/0061-squared-legend-panels.md](/adr/0061-squared-legend-panels.md) (this is 0061's deferred follow-up)
- ADR: [/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md) (the border-row run mechanism reused here)
- ADR: [/adr/0032-asset-row-link-style-structural.md](/adr/0032-asset-row-link-style-structural.md) (structural style runs from the layout)
