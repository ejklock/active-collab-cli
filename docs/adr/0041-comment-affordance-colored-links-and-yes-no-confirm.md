---
type: ADR
title: Comment edit/delete affordances render as structurally-emitted colored links (edit cyan, delete destructive red); the delete-confirm modal presents Sim/Não buttons
description: The [editar]/[excluir] tokens appended to an own comment's card header are currently plain text — they carry a click affordance but no visual signal that they are actionable, so they read as part of the timestamp line. Make them read as links — underlined, colored — with the style emitted STRUCTURALLY from the layout that places the tokens (the build_comment_card span), not re-detected from the rendered text (ADR 0032 discipline). Edit is non-destructive → SOFT_CYAN; delete is destructive → DUE_RED. Separately, relabel the delete-confirm modal's [confirmar]/[cancelar] buttons to a plain yes/no choice ([Sim]/[Não] in pt-BR, English keys Yes/No), a clearer and more universal confirm affordance.
status: Accepted
supersedes:
superseded_by:
tags: [tui, comments, affordance, link, style, theme, modal, confirm, i18n]
timestamp: 2026-06-29T00:00:00Z
---

# 0041. Comment affordances as colored links + Sim/Não confirm

## Context

[ADR 0036](/adr/0036-permission-aware-comment-targeting.md) appends `[editar]` and
`[excluir]` tokens to an own comment's card header line and records their display-column
spans as click affordances (`AffordanceKind::Edit/Delete`). But the tokens are rendered as
**plain header text** — `build_comment_card` (`src/render.rs`) places them in the label and
emits **no style run** over their spans. Visually they blend into the `autor · timestamp`
line, so a user cannot tell they are clickable.

[ADR 0032](/adr/0032-asset-row-link-style-structural.md) already established the discipline
for this class of element: a link/affordance style is a **structural** fact owned by the
layout, and must be **emitted by the layout that knows the fact**, never re-inferred from a
text pattern in the rendered string (a regression class — when the visible text changes, a
pattern matcher silently stops styling).

Separately, the delete-confirm modal ([ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md))
labels its two buttons `[confirmar]`/`[cancelar]`. The user asked for a plainer confirm:
*"os modais para confirmar pode ser apenas opção sim/não"*. A yes/no pair is more universal
and shorter than a confirmar/cancelar verb pair.

The user also asked that edit/delete *"deve ter destaque em forma de link mas podem ser de
outra cor não sei"* — link-styled, color deferred to us.

## Decision

### 1. Edit/delete affordances render as colored, underlined links

`build_comment_card` (`src/render.rs`), which already computes `edit_span`/`delete_span`
on the card header line, also **emits a `StyleRun` over each span** with a new theme style:

- `[editar]` → `theme::edit_affordance_style()` = **SOFT_CYAN + UNDERLINED** (the app accent;
  non-destructive).
- `[excluir]` → `theme::delete_affordance_style()` = **DUE_RED + UNDERLINED** (the palette's
  existing destructive red; signals a destructive action).

Both reuse existing palette constants (`SOFT_CYAN` Rgb(102,204,204), `DUE_RED` Rgb(220,80,80))
— no new colors. The UNDERLINED modifier gives the shared "this is a link" reading that the
body/asset links already have (`theme::link_style`), while the distinct foreground colors
separate the two actions from each other and from navigation links (muted green).

**Structural emission (ADR 0032):** the style runs are pushed onto the card header's
`line_styles[0]` at the **same `edit_span`/`delete_span` coordinates** the click affordance
uses, inside `build_comment_card`, so the styling and the hit-test are single-sourced and
travel through the same indent/chrome offsetting. The style is NOT applied by scanning the
rendered line for `[editar]`/`[excluir]` text.

**Style carrier — two new `RichStyle` variants.** A `StyleRun` carries a
`richtext::RichStyle`, not a raw `ratatui::Style`; the render path maps each variant to a
concrete style in `emphasis_style` (`src/tui/screens/detail.rs`), exactly as
`RichStyle::Link` → `theme::link_style()`. So the affordance styles are carried as two new
variants — `RichStyle::EditAffordance` and `RichStyle::DeleteAffordance`
(`src/richtext.rs`) — mapped in `emphasis_style` to `theme::edit_affordance_style()` /
`theme::delete_affordance_style()`. This keeps the affordance a first-class structural style
threaded through the existing run pipeline, rather than retyping `StyleRun.style` or applying
a `ratatui::Style` out of band.

This amends ADR 0036 (which deferred affordance styling) and applies the ADR 0032 pattern.

### 2. The delete-confirm modal presents Sim/Não

`render_confirm_modal` (`src/tui/view.rs`) labels its two buttons via `t("Yes")` / `t("No")`
(English source keys; pt-BR values `Sim` / `Não` in `locales/pt_BR.json`), rendered as
`[Sim]`/`[Não]` for the pt-BR user. The button click-target geometry is already derived from
the label display widths (`register_confirm_button_targets`), so it reflows automatically;
the `is_confirm` mapping (first button confirms, second cancels) and the Enter/Esc keys are
unchanged. This amends the button labels of ADR 0039 / BDR 0026.

## Consequences

- The actionable affordances are now discoverable: edit reads as a cyan link, delete as a
  red link, both underlined.
- Styling stays correct if the token text ever changes (it is emitted from the span, not the
  text) — the ADR 0032 regression class does not apply.
- The confirm modal reads as a plain yes/no question; geometry and click/keys are unchanged.
- The affordances remain **own-comment-only** (ADR 0036, `is_own` gate) and **navigation is
  still click-only** (ADR 0037) — this ADR changes only the *visual* of existing tokens and
  the *labels* of the confirm buttons, no behavior of who-can-act or how-to-act.

## Alternatives considered

- **Same color as body/asset links (muted green) for both.** Rejected: it would not separate
  the destructive delete from the non-destructive edit, and would read as a navigation link.
- **Reverse-video / background highlight instead of underline+color.** Rejected: heavier than
  a link, and the focused-card highlight already owns a background band (`focused_comment_style`).
- **A new ADR/decision per change.** Folded into one ADR: both are small comment-thread
  affordance-clarity refinements requested together.
- **Text-pattern styling (scan the line for the tokens).** Rejected by ADR 0032 — a silent
  regression when the label text changes.

## References

- [ADR 0032](/adr/0032-asset-row-link-style-structural.md). Structural link styling.
- [ADR 0036](/adr/0036-permission-aware-comment-targeting.md). Permission-aware affordances (amended).
- [ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md). Modal overlay (button labels amended).
- [BDR 0028](/bdr/0028-comment-affordance-links-and-yes-no-confirm.md). Observable behavior.
