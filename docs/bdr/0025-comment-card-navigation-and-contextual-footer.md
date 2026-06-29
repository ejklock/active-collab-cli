---
type: BDR
title: "Comment-card keyboard navigation and a contextual two-region footer: j/k focuses a comment card (highlight + scroll-into-view), and the footer shows a mode-aware hint plus a thin transient status line"
description: In the detail view, j/k (and Up/Down) move a focus cursor across the comment cards — the focused card is highlighted and scrolled fully into view; PageUp/PageDown still scroll raw lines and leave focus unchanged; edit/delete stay on the existing Ctrl/Cmd+click affordances of the focused card. The footer becomes two stacked regions: a contextual instruction line that changes by Detail mode (browsing / composing / confirming-delete / own-comment-focused) and a thin status line below it that surfaces transient state (Enviando…, write error, Copiado ✓) and is blank when idle. The compose status moves out of the inline compose block into the footer status line.
status: Accepted
superseded_by:
supersedes:
tags: [tui, comments, navigation, focus, footer, status, keyboard]
timestamp: 2026-06-28T00:00:00Z
---

# 0025. Comment-card navigation and a contextual two-region footer

## Context

The detail thread renders comments as bordered cards in one globally-scrollable
`Vec<line>`, but the only keyboard control is raw line scroll and the only way to act on
a comment is a Ctrl/Cmd+click affordance ([BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md),
[ADR 0036](/adr/0036-permission-aware-comment-targeting.md)). Two usability gaps follow:
there is no keyboard way to move through the thread or a visible "which comment am I on",
and the footer shows one hardcoded hint while transient status (compose Submitting/error,
clipboard-copied) has no fixed home — the compose status renders inline and scrolls away.

This BDR specifies the observable behavior of two decisions that close those gaps: a
keyboard **focus cursor** over the comment cards ([ADR 0037](/adr/0037-comment-card-keyboard-focus.md))
and a **two-region contextual footer** ([ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md)),
delivered as two vertical slices ([issue 0035](/issues/0035-comment-card-keyboard-focus.md),
[issue 0036](/issues/0036-detail-contextual-footer-status-line.md)).

## Textual Description

In the **detail view** of an open task with at least one comment:

- Pressing **`j`** moves focus to the **next** comment card; **`k`** to the **previous**.
  `Up`/`Down` also move focus. The **focused card is visually highlighted** and, if it is
  not fully visible, the view **scrolls so the whole focused card is on screen**. Focus
  never wraps past the last or before the first comment.
- **`PageUp`/`PageDown`** (and the mouse wheel) still scroll the content by lines and
  **leave the focused comment unchanged** — focus is a cursor over the one global scroll,
  not a second scroll model.
- A thread with **no comments** has no focus cursor (`focused_comment` is `None`); `j`/`k`
  are no-ops there.
- **Edit/delete remain on the existing Ctrl/Cmd+click** `[editar]`/`[excluir]` affordances
  of the focused (own) card — no `e`/`x` key acts on the focused comment.
- The **footer is two stacked regions**:
  - an **instruction line** whose text **changes by Detail mode**:
    - *composing* → the compose controls (Ctrl+S enviar · Esc cancelar);
    - *confirming a delete* → confirm/cancel;
    - *an own comment is focused* → move · Ctrl+clique editar/excluir · novo;
    - *browsing* (default) → move · comentar · atualizar · voltar · sair;
  - a **thin status line** below it that shows a single transient string —
    **Enviando…** while a write is in flight, the **localized error** on a failed write,
    **Copiado ✓** after a clipboard copy — and is **blank when idle** (the row collapses,
    costing no content space at rest).
- The **compose status no longer renders inline** in the scrollable content; its only home
  is the footer status line.
- Other screens keep their existing single footer hint unchanged.

## Scenarios

**Scenario 1: j/k moves the focus cursor across cards** — Given the detail view of a task
with three comments and none focused, When the user presses `j`, Then the first comment
card becomes focused and highlighted; When the user presses `j` again, Then focus moves to
the second; When the user presses `k`, Then focus returns to the first.

**Scenario 2: focus clamps at the ends** — Given the last comment is focused, When the
user presses `j`, Then focus stays on the last comment (no wraparound); Given the first is
focused, When the user presses `k`, Then focus stays on the first.

**Scenario 3: moving focus scrolls the card into view** — Given a focused card that lies
below the current viewport, When focus moves to it, Then the scroll `offset` changes so the
card's last line is visible; Given a focused card above the viewport, Then `offset` lands
on the card's first line; Given a focused card already fully visible, Then `offset` is
unchanged.

**Scenario 4: page-scroll does not move focus** — Given a focused comment, When the user
presses `PageDown`, Then the content scrolls by lines and `focused_comment` is unchanged.

**Scenario 5: empty thread has no focus** — Given a task with no comments, When the user
presses `j` or `k`, Then nothing changes (`focused_comment` stays `None`, no scroll).

**Scenario 6: actions stay on click** — Given an own comment is focused, When the user
Ctrl/Cmd+clicks its `[editar]`, Then the compose opens pre-filled (the ADR 0036 path);
no key press on the focused comment triggers edit or delete.

**Scenario 7: the footer instruction line is contextual** — Given the detail view, When
the user is browsing, Then the footer instruction line shows the browse hint; When compose
is open, Then it shows the compose hint; When a delete confirm is pending, Then it shows
the confirm hint; When an own comment is focused, Then it shows the focused-own-comment
hint.

**Scenario 8: the status line surfaces transient state** — Given a comment submit in
flight, When the detail renders, Then the footer status line shows **Enviando…**; When the
write fails, Then it shows the localized error; When the user copies a selection, Then it
shows **Copiado ✓**; When nothing is happening, Then the status line is blank.

**Scenario 9: compose status has one home** — Given a submit in flight, When the detail
content renders, Then the inline compose block does **not** contain the status text — the
status appears only on the footer status line.

## Test Design

The pure `update()` focus transitions, the scroll-into-view math, the mode→hint mapping,
and the status derivation are deterministic and asserted headlessly; the highlight, the
contextual hint, and the status line are asserted from the real `TestBackend` buffer;
scroll geometry is derived from a `comment_spans` fixture and the rendered buffer, never
assumed (mirrors the [ADR 0031](/adr/0031-tasks-card-layout-cache.md) prefix-sum test
pattern).

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Focus next/prev | unit (`update`) | 1 | `FocusNextComment`/`FocusPrevComment` move `focused_comment` by one and emit no Cmd | keyboard moves the cursor |
| Focus clamps | unit (`update`) | 2 | at the last/first, a further move is a no-op (no wraparound) | bounded focus |
| Scroll into view | unit (`update`) | 3 | with a `comment_spans` fixture, a focus move sets `offset` so the focused card is fully visible (below→end visible, above→start, visible→unchanged) | focus reveals the card |
| Page-scroll keeps focus | unit (`update`) | 4 | `PageDown`/`PageUp`/wheel change `offset`, leave `focused_comment` unchanged | two move models don't collide |
| Empty thread | unit (`update`) | 5 | with zero comments, `FocusNext`/`Prev` keep `focused_comment = None`, emit no Cmd | safe no-op |
| Highlight render | render (`TestBackend`) | 1,3 | the focused card carries the focus style and others do not; moving focus moves the highlight | visible cursor |
| Actions still click | unit (`update`) | 6 | Ctrl/Cmd+click `[editar]`/`[excluir]` on the focused card emits `ComposeOpen(Edit)` / `DeleteCommentRequest`; no key acts | one action path |
| Contextual hint | render (`TestBackend`) | 7 | the footer instruction line shows the mode's hint for browse / compose / confirm / own-focused; switching modes switches the text | mode-aware footer |
| Status line | render (`TestBackend`) | 8 | `Submitting`→`Enviando…`, `Error`→localized error, `copied_feedback`→`Copiado ✓`, idle→blank | transient status surfaced |
| One home for status | render (`TestBackend`) | 9 | the inline compose block no longer renders the status text | no duplicated/lost status |

## Related

- ADR: [/adr/0037-comment-card-keyboard-focus.md](/adr/0037-comment-card-keyboard-focus.md)
- ADR: [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (the click affordances kept; this layers keyboard nav)
- ADR: [/adr/0031-tasks-card-layout-cache.md](/adr/0031-tasks-card-layout-cache.md) (the card selection + prefix-sum + first-visible pattern mirrored)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md) (the compose/affordance behavior this builds on)
- Issues: [/issues/0035-comment-card-keyboard-focus.md](/issues/0035-comment-card-keyboard-focus.md), [/issues/0036-detail-contextual-footer-status-line.md](/issues/0036-detail-contextual-footer-status-line.md)
