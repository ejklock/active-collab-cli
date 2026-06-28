---
type: PRD
title: Task comment authoring — create, edit, and delete comments from the TUI
description: The app gains its first WRITE capability. From the detail view of an open task the user can compose and post a new comment (multi-line), edit one of their own comments, and delete one of their own comments. Every mutation goes to ActiveCollab over an authenticated write, and the on-screen thread is re-derived from the server after each mutation so what the user sees always matches the server. Read/browse stays unchanged.
status: Accepted
superseded_by:
tags: [tui, comments, write, mutation, authoring]
timestamp: 2026-06-28T00:00:00Z
---

# 0002. Task comment authoring — create, edit, and delete comments

<!-- Status lives in frontmatter. This PRD specifies the capability; the HOW is
     ADR 0033-0036, the observable behavior is BDR 0024. -->

## Problem / Motivation

The Rust app reads tasks and their comment threads but cannot write anything back
([PRD 0001](/prd/0001-rust-tui-cli-parity.md) scoped the rewrite to *read/browse
only* — an explicit non-goal). In practice the most common action a user wants
*after* reading a task is to respond on it: add a comment, fix a typo in a comment
they just posted, or remove one. Today they must leave the tool and open the
ActiveCollab web UI to do any of that, which breaks the "stay in the terminal"
value of the app.

This PRD **lifts the PRD 0001 "no writing" non-goal for comments specifically** (it
does not open the door to editing tasks, time, or other entities — those remain out
of scope). It introduces the app's first authenticated **write** path, deliberately
scoped to the one mutation surface with the highest daily value and the smallest
blast radius: comments on the currently-open task.

## Goals

- From the detail view, **create** a new comment on the open task, with a
  **multi-line** body (real comments wrap and use line breaks).
- **Edit** one of the user's *own* comments in place.
- **Delete** one of the user's *own* comments, with a confirmation step.
- After any successful mutation, the on-screen comment thread **matches the server**
  (correct author, timestamp, id, and server-normalized body) without a manual
  refresh.
- A mutation never leaks the instance token to a foreign host, and never blocks the
  TUI event loop (the write runs as a background effect, like every fetch).

## Non-goals

- Editing or deleting **other people's** comments (the affordance is shown only on
  the user's own comments; the server is the final authority via `canEdit`/`canDelete`).
- Creating/editing/deleting any entity **other than a comment** (tasks, subtasks,
  time records, attachments) — still out of scope.
- Attachments on a comment, @mentions, reactions, or rich-text *composition* (the
  user types plain text; existing rich-text **rendering** of the posted result is
  unchanged).
- Optimistic UI that shows the comment before the server confirms — see
  [ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md) for why the
  first version refreshes from the server instead.
- A CLI (non-TTY) comment command — this capability is TUI-only for now.

## Requirements

1. In the detail view, a key opens a **compose** area to write a new comment on the
   open task; **Enter inserts a newline**, **Ctrl+S submits**, **Esc cancels**.
2. Submitting posts the body to ActiveCollab as the authenticated user; on success
   the thread reloads and shows the new comment.
3. Each of the user's **own** comments in the thread shows an **edit** and a
   **delete** affordance; other comments show neither.
4. Activating **edit** opens the compose area pre-filled with that comment's current
   body; Ctrl+S saves the change to the server; the thread reloads.
5. Activating **delete** asks for confirmation; confirming removes the comment on the
   server; the thread reloads without it.
6. Any write failure (non-2xx, transport error, or lack of permission) leaves the
   user's typed text intact and surfaces an inline, localized error — it never
   crashes the TUI and never silently drops the body.
7. The token-host-isolation guarantee ([PRD 0001](/prd/0001-rust-tui-cli-parity.md)
   req. 7) holds for writes: a write request carries the instance token only to the
   instance host.
8. All user-facing strings (compose hint, confirm prompt, error messages) are
   localized (en + pt-BR).

## Quality requirements (NFRs)

| Quality attribute | Scenario (source · stimulus · artifact · environment · response · measure) | Verified by |
|---|---|---|
| Correctness (server truth) | A user · posts/edits/deletes a comment · the on-screen thread · normal use · re-renders from a fresh server fetch, not a local guess · 0 divergences between shown thread and server state | After-mutation `Cmd::LoadDetail { refresh: true }`; unit test on `update()` asserting the refresh Cmd is emitted on success (BDR 0024) — `verify_by: command` |
| Security (token isolation) | The client · issues a write (POST/PUT/DELETE) to a non-instance host · from the TUI · the request carries no instance token · 0 tokens leaked cross-host | The write methods reuse `host_gated_token_header`; negative test asserts no token off-host (ADR 0033) — `verify_by: command` |
| Robustness (input safety) | A user · submits and the write fails · in the compose area · the typed body is preserved and an inline error shows · 0 lost drafts, 0 crashes | Unit test on `update()` for the failure Msg keeping the buffer + setting an error (BDR 0024) — `verify_by: command` |
| Responsiveness (non-blocking) | A user · submits a comment · the TUI · under normal use · the event loop keeps redrawing while the write runs in the background · 0 frozen frames | The write is a `Cmd` spawned on the mpsc/`tokio::select!` path, mirroring `LoadDetail` (ADR 0034) — `verify_by: command` |
| Authorization (own-only) | A user · views a thread with their own and others' comments · the detail view · normal use · edit/delete affordances appear only on their own comments · 0 affordances on foreign comments | Pure render/hit-test test keyed on `created_by_id == instance.user_id` (ADR 0036, BDR 0024) — `verify_by: command` |

## Acceptance criteria

- A user composes a multi-line comment, presses Ctrl+S, and sees it appear in the
  thread (loaded from the server).
- A user edits their own comment and sees the updated text; the edit affordance is
  absent on comments they do not own.
- A user deletes their own comment after a confirm, and the thread re-renders without
  it.
- A failed write keeps the typed text and shows an inline error; the app stays open.
- No write request carries the instance token to a non-instance host (asserted by a
  test).

## Success metrics

- A user can post a comment without leaving the terminal (was: switch to the web UI).
- Zero reported "my comment vanished / showed the wrong author or time" issues after
  a mutation (the server-truth refresh closes this class).
- Zero token-leak regressions for the new write path (same bar as the read path).

## Behavior (BDRs)

- [BDR 0024 — Comment authoring: create, edit, and delete with server-truth refresh](/bdr/0024-comment-authoring-create-edit-delete.md)

## Open questions

- Optimistic insert (show the comment instantly, reconcile on server reply) — deferred
  to a follow-up once the refresh-based version ships and its latency is measured.
- A non-TTY CLI comment command (`ac comment <ref> <body>`) — out of scope here;
  candidate follow-up PRD.
- Multi-line editing niceties (cursor movement within the body, soft-wrap caret) —
  the first version is append/backspace + newline; richer caret editing is a
  follow-up if needed.

## Decision log

- [ADR 0033 — Authenticated write seam + comment mutation client methods](/adr/0033-authenticated-write-seam-comment-client.md)
- [ADR 0034 — Multi-line comment compose as a mode on the Detail screen](/adr/0034-comment-compose-mode-multiline.md)
- [ADR 0035 — Server-truth refresh after a comment mutation (no optimistic UI)](/adr/0035-server-truth-refresh-after-comment-mutation.md)
- [ADR 0036 — Permission-aware edit/delete affordances target a comment](/adr/0036-permission-aware-comment-targeting.md)

## Related

- Constitution: [/constitution.md](/constitution.md)
- PRD: [/prd/0001-rust-tui-cli-parity.md](/prd/0001-rust-tui-cli-parity.md) (this PRD lifts its read-only non-goal for comments)
- Issues: [/issues/0032-create-comment.md](/issues/0032-create-comment.md), [/issues/0033-edit-comment.md](/issues/0033-edit-comment.md), [/issues/0034-delete-comment.md](/issues/0034-delete-comment.md)
