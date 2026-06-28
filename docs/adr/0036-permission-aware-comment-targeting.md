---
type: ADR
title: Edit/delete target a comment via permission-aware inline affordances rendered only on the user's own comments
description: The detail thread is a flat scrollable text blob, so edit/delete need a way to target one specific comment. Render an [editar] / [excluir] affordance in each comment's header line, as scroll-aware click targets, but only when created_by_id == instance.user_id (the user's own comment) — mirroring the existing Ctrl/Cmd+click asset model and the server's canEdit/canDelete authority. Clicking [editar] opens the compose mode pre-filled (ComposeKind::Edit{comment_id}); clicking [excluir] opens an inline confirm before the DELETE. The local own-comment check is an affordance filter, not a security boundary — the server enforces permission.
status: Accepted
supersedes:
superseded_by:
tags: [tui, comments, edit, delete, click, authorization, render]
timestamp: 2026-06-28T00:00:00Z
---

# 0036. Permission-aware edit/delete affordances target a comment

## Context

Editing or deleting a comment requires **targeting one specific comment** in the
thread. But the detail view renders comments as part of a **flat, globally-scrollable
`Vec<line>`** ([ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md)) —
there is no per-comment selection cursor or focus model (the sectioned-panel focus
model was reverted, [ADR 0010](/adr/0010-detail-sectioned-panels-focus-scroll.md)). So
the feature needs a targeting mechanism that fits a scrollable text view.

The app already has a precedent for "act on a specific element inside the scrollable
content": **assets** are click targets — a scroll-aware Ctrl/Cmd+click hit-test maps a
viewport row to an asset and emits `Cmd::OpenAsset`
([ADR 0025](/adr/0025-asset-activation-ctrl-cmd-click.md),
[ADR 0020](/adr/0020-body-links-inline-url-native-click.md)). The same machinery
(`click_targets`, the `offset + (row − text_top)` translation) can target a comment.

There is also an **authorization** dimension: a user may edit/delete only their own
comments (the server enforces `canEdit`/`canDelete`). The `Instance` carries
`user_id`; each comment JSON carries `created_by_id`. So "is this mine?" is a cheap,
local predicate — useful to decide *whether to show* the affordance, but **not** a
security control (the server remains the authority).

## Decision

Render, in each comment's **header line** (the existing author · date line), a small
**`[editar]` and `[excluir]` affordance pair** — but **only when
`comment.created_by_id == instance.user_id`**. Comments the user does not own render no
affordance.

- The affordances are **scroll-aware click targets**, registered the same way asset
  rows are: each maps (via the line→comment map) to a `comment_id`, and the click
  hit-test translates the viewport row through the current scroll `offset`.
- **Click `[editar]`** → `Msg::ComposeOpen(Edit { comment_id })`, which opens the
  compose mode ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md)) with `buffer`
  pre-filled from that comment's current plain-text body. Ctrl+S then issues
  `update_comment(comment_id, body)` (PUT) and refreshes
  ([ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md)).
- **Click `[excluir]`** → `Msg::DeleteCommentRequest { comment_id }`, which sets an
  inline **confirm** state on the Detail screen (a small `confirm_delete:
  Option<i64>`); a confirm key/click issues `delete_comment(comment_id)` (DELETE) and
  refreshes; cancel clears the confirm.

**The local `created_by_id == user_id` check is an affordance filter, not a security
boundary.** The server enforces `canEdit`/`canDelete`; if a write is rejected (403),
the standard `CommentMutationErr` path shows the inline error. This is recorded so the
local check is never mistaken for the authorization control (an LLM/agent must not
"optimize" the server round-trip away).

### Guard / fitness function

- **Own-only affordance (unit, pure):** a render/layout test asserts the
  `[editar]`/`[excluir]` targets are produced for a comment whose `created_by_id`
  equals the instance `user_id`, and **absent** for a comment owned by someone else.
- **Scroll-aware targeting (unit):** at scroll offset O, a click on the `[editar]`
  target row emits `ComposeOpen(Edit{comment_id})` for the correct comment; the
  `[excluir]` target emits `DeleteCommentRequest{comment_id}`. Reuses the asset
  click-map test pattern.
- **Edit pre-fill:** `ComposeOpen(Edit{id})` seeds `buffer` from that comment's body
  (asserted on `update()`).
- **Confirm gate:** `DeleteCommentRequest` sets `confirm_delete = Some(id)` and emits
  no write Cmd; only the confirm action emits `Cmd::DeleteComment`. A cancel clears it.

## Alternatives considered

- **A per-comment selection cursor** (arrow-key navigable, `e`/`x` act on the
  selected comment). Rejected for v1: it reintroduces a focus/selection model the
  detail view deliberately dropped (ADR 0010 revert) and is more machinery than the
  click-target approach the app already supports. A keyboard path can be layered later.
- **Always show edit/delete on every comment** and let the server 403 sort it out.
  Rejected: it invites a guaranteed-to-fail action and clutters foreign comments; the
  cheap local `user_id` predicate gives a clean, correct affordance without claiming to
  be the security boundary.
- **A modal list of "my comments" to pick from.** Rejected: a context switch away from
  the thread the user is already reading; inline affordances keep the action where the
  comment is.

## Consequences

**Positive:** edit/delete get a targeting mechanism that fits the flat scrollable view
and reuses the proven scroll-aware click-target machinery. Foreign comments stay
read-only and uncluttered. The own-comment predicate is cheap and local. The confirm
step makes delete safe.

**Accepted trade-offs:** targeting is mouse-driven for v1 (no keyboard path yet — a
follow-up). The thread render grows the per-comment header to include the affordance
tokens (own comments only), and the Detail screen gains a small `confirm_delete`
state. The local own-check can momentarily disagree with the server (e.g. admin rights,
or a comment whose ownership the user map hasn't resolved) — accepted because the
server is the authority and the error path is graceful.

## Related

- PRD: [/prd/0002-task-comment-authoring.md](/prd/0002-task-comment-authoring.md)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md) (edit reuses the compose mode)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md) (update_comment / delete_comment)
- ADR: [/adr/0035-server-truth-refresh-after-comment-mutation.md](/adr/0035-server-truth-refresh-after-comment-mutation.md)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md), [/adr/0025-asset-activation-ctrl-cmd-click.md](/adr/0025-asset-activation-ctrl-cmd-click.md) (the scroll-aware click-target machinery reused)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md)
