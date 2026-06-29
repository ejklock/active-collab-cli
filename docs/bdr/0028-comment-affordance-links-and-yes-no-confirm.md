---
type: BDR
title: "Comment edit/delete affordances read as colored underlined links (own comments only); the delete-confirm modal presents Sim/Não"
description: On an own comment's card header, [editar] renders as a soft-cyan underlined link and [excluir] as a destructive-red underlined link; on others' comments neither appears. The styling is emitted by the layout over the affordance span (structural), so it stays correct regardless of the token text. Ctrl/Cmd+click on the styled token still opens edit / requests delete (unchanged). The delete-confirm modal shows [Sim] and [Não] buttons; clicking Sim confirms the delete, Não cancels; Enter confirms, Esc cancels. All button strings resolve through i18n.
status: Accepted
superseded_by:
supersedes:
tags: [tui, comments, affordance, link, style, modal, confirm, i18n]
timestamp: 2026-06-29T00:00:00Z
---

# 0028. Comment affordance links + Sim/Não confirm

## Context

[ADR 0041](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md) makes the
`[editar]`/`[excluir]` affordances read as colored links and relabels the delete-confirm
modal buttons to a yes/no choice. This BDR specifies the observable behavior. It amends the
confirm-button labels of [BDR 0026](/bdr/0026-comment-modal-overlay.md) and the affordance
appearance of [BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md), and builds on
the structural-styling behavior of [BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md).

## Textual Description

In the **detail view** of an open task with comments:

- On a comment **authored by the logged-in user**, the card header line ends with two
  affordance tokens: `[editar]` rendered in **soft cyan, underlined**, and `[excluir]` in
  **destructive red, underlined**. They read as links, visually distinct from the
  `autor · timestamp` text and from navigation links (muted green).
- On a comment **authored by someone else**, neither token appears (no style, no affordance).
- The colored styling is emitted by the layout over the token's column span; it does not
  depend on the token's text, so changing the label text would not drop the styling.
- **Ctrl/Cmd+click** on the `[editar]` token opens the edit compose; on `[excluir]` it
  requests delete (opening the confirm modal). This is unchanged — focus/keys never act.
- The **delete-confirm modal** shows two buttons, `[Sim]` and `[Não]`. Clicking `[Sim]`
  confirms the delete; `[Não]` cancels. `Enter` confirms, `Esc` cancels. The button labels
  resolve through `i18n::t()` (pt-BR `Sim`/`Não`).

## Scenarios

1. **Own comment — edit affordance is a cyan link.** Given a comment authored by the
   logged-in user, when the thread renders, then `[editar]` on the header carries soft-cyan
   foreground + the underline modifier.
2. **Own comment — delete affordance is a red link.** Given the same, then `[excluir]`
   carries destructive-red foreground + the underline modifier.
3. **Other's comment — no affordance.** Given a comment authored by another user, when the
   thread renders, then neither `[editar]` nor `[excluir]` appears and no affordance is
   registered for that card.
4. **Styling is structural, not text-derived.** Given an own comment, the style run covers
   exactly the affordance span recorded for the click hit-test (same coordinates), proving
   the layout emitted it (not a text-pattern match).
5. **Click still acts.** Given an own comment, when the user Ctrl/Cmd+clicks the `[editar]`
   span, then `ComposeOpen(Edit)` is emitted; clicking `[excluir]` emits the delete request.
6. **Confirm modal shows Sim/Não.** Given the delete-confirm modal open, when it renders,
   then the hint row shows `[Sim]` and `[Não]`.
7. **Sim confirms, Não cancels.** Given the confirm modal open, when the user clicks `[Sim]`
   then the delete is confirmed (`ConfirmDeleteComment`); when the user clicks `[Não]` then
   it is cancelled (`CancelDeleteComment`). Enter confirms, Esc cancels.
8. **i18n.** Given a pt-BR locale, the confirm buttons render `Sim`/`Não` (English source
   keys `Yes`/`No`, values present in `locales/pt_BR.json`).

## Test Design

| Scenario | Level | Technique | Instrument |
|---|---|---|---|
| 1, 2 | unit | buffer-derived render (TestBackend): assert the cell fg + UNDERLINED modifier over the `[editar]`/`[excluir]` columns | `tests/unit/tui_render.rs` |
| 3 | unit | render an other-authored comment; assert no affordance style + no registered affordance | `tests/unit/render.rs` / `tui_render.rs` |
| 4 | unit | assert the style run's `(start,len)` equals the affordance span used by the hit-test (single-source) | `tests/unit/render.rs` |
| 5 | unit | real-click-path: reflow the Detail model, read the affordance coord, drive `update(Msg::Click{..CONTROL})`, assert the dispatched Msg | `tests/unit/model.rs` |
| 6 | unit | render the confirm modal; assert the hint row contains `Sim` and `Não` | `tests/unit/tui_render.rs` |
| 7 | unit | resolve the button targets from the rendered modal Rect; assert click on the Sim/Não target dispatches Confirm/Cancel | `tests/unit/model.rs` / `tui_render.rs` |
| 8 | unit | assert `pt_BR.json` carries `Yes`→`Sim`, `No`→`Não`; the modal renders them under pt-BR | `tests/unit/tui_render.rs` |

Mutation guards: the render tests assert the **specific** fg color + modifier (a swapped
color or a dropped underline fails); Scenario 4 pins style-vs-hit-test agreement (re-deriving
the span in the test would not prove the layout emitted it, so the test reads the produced
style run). The confirm tests assert the is_confirm→Msg mapping so swapping the two buttons
fails.

## References

- [ADR 0041](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md). The decision.
- [BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md). Affordance appearance (amended).
- [BDR 0026](/bdr/0026-comment-modal-overlay.md). Confirm-button labels (amended).
