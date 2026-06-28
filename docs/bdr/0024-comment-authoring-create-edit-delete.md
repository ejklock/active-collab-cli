---
type: BDR
title: "Comment authoring: create a multi-line comment, edit and delete your own, with a server-truth refresh after each mutation"
description: From the detail view of an open task, pressing c opens a multi-line compose area (Enter inserts a newline, Ctrl+S submits, Esc cancels); submitting posts the comment and the thread reloads from the server to show it. Each of the user's own comments shows an [editar] and [excluir] affordance (absent on others' comments); [editar] opens the compose pre-filled and Ctrl+S saves; [excluir] asks to confirm, then removes it. After any successful create/edit/delete the thread is re-derived from a fresh server fetch. A failed write keeps the typed text, shows an inline error, and never crashes.
status: Accepted
superseded_by:
supersedes:
tags: [tui, comments, write, compose, edit, delete, mutation]
timestamp: 2026-06-28T00:00:00Z
---

# 0024. Comment authoring: create, edit, and delete with server-truth refresh

## Context

The app reads task comment threads but cannot write to them. [PRD 0002](/prd/0002-task-comment-authoring.md)
adds the first write capability — create/edit/delete comments on the open task —
delivered across three vertical slices ([issue 0032](/issues/0032-create-comment.md)
create, [issue 0033](/issues/0033-edit-comment.md) edit,
[issue 0034](/issues/0034-delete-comment.md) delete) over the write seam
([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)), the multi-line
compose mode ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md)), the
server-truth refresh ([ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md)),
and permission-aware targeting ([ADR 0036](/adr/0036-permission-aware-comment-targeting.md)).

## Textual Description

In the **detail view** of an open task:

- Pressing **`c`** opens a **compose area** for a new comment. Typing appends to a
  **multi-line** body; **Enter inserts a newline**; **Backspace** deletes the last
  character; **Ctrl+S submits**; **Esc cancels** (discarding the draft and closing the
  compose area without leaving the detail view).
- On **submit**, the body is posted to ActiveCollab as the authenticated user. While
  the write is in flight the compose area shows a **Submitting** state; the TUI keeps
  redrawing (the write is a background effect).
- On **success**, the comment thread **reloads from the server** and the new comment
  appears with its real author, timestamp, and server-stored body. The compose area
  closes.
- Each comment **authored by the current user** (`created_by_id == instance.user_id`)
  shows an **`[editar]`** and an **`[excluir]`** affordance in its header; comments by
  **other** users show neither.
- **`[editar]`** opens the compose area **pre-filled** with that comment's current
  body; **Ctrl+S** saves the change to the server; on success the thread reloads with
  the edited text.
- **`[excluir]`** opens an inline **confirm**; **confirming** deletes the comment on
  the server and the thread reloads without it; **cancelling** leaves it untouched.
- On **any write failure** (non-2xx, transport error, or permission denied), the typed
  text is **preserved**, an **inline localized error** is shown, and the app stays
  open. No draft is silently lost; no thread mutation happens on failure.
- The write request carries the instance token **only to the instance host**.

## Scenarios

**Scenario 1: open compose and type a multi-line body** — Given the detail view of an
open task, When the user presses `c`, types text, presses Enter, and types more, Then a
compose area is active with a two-line buffer (the Enter became a newline, not a
submit).

**Scenario 2: submit posts and the thread reloads from the server** — Given a non-empty
compose buffer, When the user presses Ctrl+S, Then the app emits a create-comment write
for the open task, sets a Submitting state, and on the write's 2xx re-fetches the
task+comments (`LoadDetail refresh`) so the new comment is shown from server data.

**Scenario 3: cancel discards the draft** — Given an active compose area with typed
text, When the user presses Esc, Then the compose area closes, the draft is discarded,
and the detail view is unchanged (no write, still on the same task).

**Scenario 4: edit/delete affordances appear only on own comments** — Given a thread
containing one comment authored by the current user and one by someone else, When the
detail renders, Then the user's own comment shows `[editar]`/`[excluir]` targets and the
other comment shows none.

**Scenario 5: edit opens pre-filled and saves** — Given the user's own comment, When the
user activates `[editar]`, Then the compose area opens pre-filled with that comment's
body and carries its `comment_id`; When the user changes the text and presses Ctrl+S,
Then the app emits an edit (PUT) for that `comment_id` and refreshes on success.

**Scenario 6: delete asks to confirm, then removes** — Given the user's own comment,
When the user activates `[excluir]`, Then an inline confirm appears and **no** write is
emitted yet; When the user confirms, Then the app emits a delete (DELETE) for that
`comment_id` and refreshes the thread without it; When the user cancels instead, Then no
write is emitted and the comment remains.

**Scenario 7: a failed write keeps the draft and shows an error** — Given a submit (or
edit) whose write returns non-2xx or errors, When the failure lands, Then the compose
buffer is preserved, an inline localized error is shown, no `LoadDetail` refresh is
emitted, and the app stays open.

**Scenario 8: token isolation on writes** — Given a comment write whose URL host is not
the instance host, When the request is built, Then it carries no `X-Angie-AuthApiToken`
header (the host gate); a write to the instance host does carry it.

## Test Design

The pure `update()` state machine, the key mapping, the own-comment affordance
predicate, and the click hit-test are deterministic and asserted headlessly; the
compose/affordance rendering is asserted from the real `TestBackend` buffer; the client
write contract + token gate are asserted with a mocked server (wiremock). Geometry is
derived from the real rendered buffer, never assumed.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Compose multi-line buffer | unit (`update`) | 1 | `ComposeOpen(New)` then `ComposeInput`/`ComposeNewline` yield a buffer containing `\n`; Enter did not submit | Enter = newline, not submit |
| Submit emits write + Submitting | unit (`update`) | 2 | `ComposeSubmit` on a non-empty buffer emits `Cmd::SubmitComment{task_id, body}` and sets `Submitting` | submit drives the write |
| Success refreshes from server | unit (`update`) | 2 | `CommentMutationOk` emits exactly one `Cmd::LoadDetail{refresh:true}` and clears compose | server-truth refresh |
| Cancel discards draft | unit (`update`) | 3 | `ComposeCancel` clears compose, emits no Cmd, stack unchanged | no accidental write/leave |
| Own-only affordances | unit + render | 4 | `[editar]`/`[excluir]` click targets exist for `created_by_id == user_id`, absent otherwise; buffer shows the tokens only on own comments | permission-aware affordance |
| Edit pre-fills + PUT | unit (`update`) | 5 | `ComposeOpen(Edit{id})` seeds buffer from the comment body; `ComposeSubmit` emits the edit write carrying `id` | edit reuses compose, targets id |
| Delete confirm gate | unit (`update`) | 6 | `DeleteCommentRequest{id}` sets `confirm_delete=Some(id)` and emits no write; confirm emits `Cmd::DeleteComment{id}`; cancel clears it | no unconfirmed delete |
| Failure keeps draft | unit (`update`) | 7 | `CommentMutationErr(msg)` keeps the buffer, sets `Error(msg)`, emits no `LoadDetail` | lossless, no crash |
| Client write contract | unit (wiremock) | 2,5,6 | `create_comment` POSTs `comments/task/{id}` with `{"body":…}`→`(200,Some)`; `update_comment` PUTs `comments/{id}`; `delete_comment` DELETEs→status | endpoint/verb/body correct |
| Token gate on writes | unit | 8 | `authed_post`/`authed_put`/`authed_delete` attach the token on-host, omit it off-host | no cross-host token leak |

## Related

- PRD: [/prd/0002-task-comment-authoring.md](/prd/0002-task-comment-authoring.md)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md)
- ADR: [/adr/0035-server-truth-refresh-after-comment-mutation.md](/adr/0035-server-truth-refresh-after-comment-mutation.md)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md)
- Issues: [/issues/0032-create-comment.md](/issues/0032-create-comment.md), [/issues/0033-edit-comment.md](/issues/0033-edit-comment.md), [/issues/0034-delete-comment.md](/issues/0034-delete-comment.md)
