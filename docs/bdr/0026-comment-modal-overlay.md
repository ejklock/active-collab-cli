---
type: BDR
title: "Comment compose and delete-confirm open as a centered modal overlay over a dimmed thread, sharing one reusable modal primitive"
description: Opening a comment compose (new or edit) or a delete-confirm renders a centered, bordered modal box over the Detail thread; the thread behind it is dimmed (still visible, not hidden) and the modal captures input. The compose modal shows the multi-line buffer with an in-box hint (Ctrl+S enviar · Esc cancelar) and the transient status (Enviando… / error) inside the box; Ctrl+S submits, Esc cancels, Enter inserts a newline. The confirm modal shows [confirmar]/[cancelar] buttons; clicking a button, or pressing Enter (confirm) / Esc (cancel), acts. Neither the compose field nor the confirm prompt appears in the scrollable content anymore. Both modals are drawn by the same reusable primitive.
status: Accepted
superseded_by:
supersedes:
tags: [tui, modal, overlay, comments, compose, confirm, render]
timestamp: 2026-06-28T00:00:00Z
---

# 0026. Comment compose and delete-confirm as a centered modal overlay

## Context

The comment compose field and the delete-confirm prompt render inline in the Detail
screen's scrollable content, so they scroll with the thread and have no focus framing
([BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md),
[BDR 0025](/bdr/0025-comment-card-navigation-and-contextual-footer.md)). This BDR
specifies the observable behavior of moving both into a **reusable centered modal overlay
with a dimmed backdrop** ([ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md)),
delivered as two vertical slices ([issue 0037](/issues/0037-modal-primitive-and-compose.md)
the modal primitive + compose, [issue 0038](/issues/0038-confirm-delete-modal.md) the
confirm).

## Textual Description

In the **detail view** of an open task:

- Opening a **new comment** (`c`) or **editing** an own comment (`[editar]`) renders a
  **centered, bordered modal box** over the thread. The thread behind the box is
  **dimmed** — still visible, not painted over. The modal **captures input** (the compose
  key map owns every key while it is open).
- The compose modal shows its **title** (`Novo comentário` / `Editar comentário`), the
  **multi-line buffer** being typed, and a **bottom line inside the box** with the
  controls hint (`Ctrl+S enviar · Esc cancelar`) and the **transient status**
  (`Enviando…` while in flight, the localized error on failure).
- **Ctrl+S submits**, **Esc cancels** (discards the draft, closes the modal), **Enter
  inserts a newline**, **Backspace** deletes the last character — same compose semantics
  as before; only the rendering changed.
- The compose field **no longer appears in the scrollable content** — it is only the modal.
- Activating **`[excluir]`** on an own comment opens a **confirm modal**: title
  `Excluir comentário?`, a short body, and **`[confirmar]`/`[cancelar]` buttons**.
- **Clicking `[confirmar]`** deletes the comment (then the thread refreshes); **clicking
  `[cancelar]`**, or pressing **Esc**, closes the modal with no change; pressing **Enter**
  confirms. The confirm prompt **no longer appears as tokens inside the comment card**.
- Only **one modal** is open at a time (compose or confirm), derived from the Detail state
  (`compose` / `confirm_delete`).
- Both modals are drawn by the **same reusable primitive** (a centered `Rect` + a dimmed
  backdrop + a `Clear`ed bordered box).

## Scenarios

**Scenario 1: opening compose shows a large centered modal over a strongly dimmed thread** —
Given the detail view of a task, When the user presses `c`, Then a centered bordered modal
titled `Novo comentário` is rendered over the thread occupying ≈ 70 % of the frame, the
thread cells behind it are **strongly dimmed** (carry `Modifier::DIM` **and** a dark
backdrop background — still faintly visible, not transparent), and the modal shows the
(empty) buffer and the in-box hint.

**Scenario 2: typing a multi-line body in the modal** — Given an open compose modal, When
the user types text, presses Enter, and types more, Then the modal body shows a two-line
buffer (Enter inserted a newline, did not submit).

**Scenario 3: compose is not in the scrollable content** — Given an open compose modal,
When the Detail content (the scrollable `lines`) is inspected, Then it does **not** contain
the compose label or buffer — the compose lives only in the overlay.

**Scenario 4: submit shows status in the modal, then refreshes** — Given a non-empty
compose modal, When the user presses Ctrl+S, Then the modal's status line shows `Enviando…`
while the write is in flight, and on success the modal closes and the thread reloads from
the server.

**Scenario 5: cancel discards the draft** — Given an open compose modal with typed text,
When the user presses Esc, Then the modal closes, the draft is discarded, and the detail
view is unchanged.

**Scenario 6: edit opens the modal pre-filled** — Given the user's own comment, When the
user activates `[editar]`, Then a compose modal titled `Editar comentário` opens pre-filled
with that comment's body.

**Scenario 7: delete opens a confirm modal (not inline tokens)** — Given the user's own
comment, When the user activates `[excluir]`, Then a confirm modal titled
`Excluir comentário?` opens with `[confirmar]`/`[cancelar]` buttons, and the comment card
shows **no** inline confirm tokens.

**Scenario 8: confirm acts by click or key** — Given an open confirm modal, When the user
clicks `[confirmar]` (or presses Enter), Then the app emits the delete and the thread
refreshes; When the user clicks `[cancelar]` (or presses Esc) instead, Then the modal
closes and the comment remains.

**Scenario 9: the modal is large, centered, and clamps to the frame** — Given a large
frame, When the modal renders, Then it is centered and occupies ≈ 70 % of the frame width
and height (a dominant panel, not a thin content-hugging strip); Given a frame
narrower/shorter than the target modal size, Then the modal clamps to fit within the frame
with a margin (never overflows).

**Scenario 10: the modal owns its hint/status; the footer does not duplicate them** —
Given an open compose modal showing `Enviando…` in the box, When the footer renders, Then
the footer does not also show the compose hint/status for the modal (one home — the modal).

## Test Design

`modal_area` centering/clamping and the pure `update()` transitions are asserted
headlessly; the modal rendering, the dimmed backdrop, the in-box hint/status, and the
button hit-tests are asserted from the real `TestBackend` buffer; geometry is derived from
the rendered buffer, never assumed.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Centered + clamp | unit (pure) | 9 | `modal_area` returns a centered Rect within the frame for large frames; clamps (no overflow) for small frames | layout primitive correct |
| Size ≈ 70 % | render (`TestBackend`) | 9 | for a large frame the rendered modal box spans ≈ 70 % of width and height (within tolerance), never full-frame nor a thin strip | dominant centered panel |
| Compose modal render | render (`TestBackend`) | 1 | a centered titled box over the content; backdrop cells carry `Modifier::DIM` **and** the dark backdrop background; buffer + in-box hint shown | modal over strongly-dimmed thread |
| Multi-line buffer | unit (`update`) | 2 | `ComposeInput`/`ComposeNewline` build a buffer with `\n`; Enter did not submit | compose semantics intact |
| Compose absent from scroll | unit + render | 3 | after `reflow_detail` with `compose.is_some()`, scrollable `lines` lack the compose label/buffer; the buffer shows only in the overlay | rendering moved out of scroll |
| Submit status in modal | unit + render | 4 | `ComposeSubmit` sets `Submitting`; the modal status line renders `Enviando…`; `CommentMutationOk` closes the modal + refreshes | status lives in the modal |
| Cancel discards | unit (`update`) | 5 | `ComposeCancel` clears compose, emits no Cmd | no accidental write |
| Edit pre-fill | unit + render | 6 | `ComposeOpen(Edit{id})` seeds the buffer; the modal title is `Editar comentário` | edit reuses the modal |
| Confirm modal (no inline) | render (`TestBackend`) | 7 | `confirm_delete=Some(id)` renders the confirm modal with buttons; the comment card has no `[confirmar]`/`[cancelar]` tokens | confirm moved to modal |
| Confirm by click/key | unit + render | 8 | clicking the `[confirmar]` button span emits Confirm; `[cancelar]` emits Cancel; Enter→Confirm, Esc→Cancel | dual click/key path |
| One home for hint/status | render (`TestBackend`) | 10 | with the compose modal open, the footer does not render the compose hint/status | no duplicated chrome |

## Related

- ADR: [/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md), [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md), [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md) (rendering amended by ADR 0039)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md), [/bdr/0025-comment-card-navigation-and-contextual-footer.md](/bdr/0025-comment-card-navigation-and-contextual-footer.md) (the inline behavior this re-homes)
- Issues: [/issues/0037-modal-primitive-and-compose.md](/issues/0037-modal-primitive-and-compose.md), [/issues/0038-confirm-delete-modal.md](/issues/0038-confirm-delete-modal.md)
