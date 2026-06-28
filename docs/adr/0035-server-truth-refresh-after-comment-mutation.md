---
type: ADR
title: After a comment mutation, re-derive the thread from the server (LoadDetail refresh) — no optimistic UI
description: On a successful create/edit/delete the app re-fetches the task+comments via the existing Cmd::LoadDetail { refresh: true } path instead of mutating the local comments vector optimistically. The server owns the comment id, created_on, resolved author, and HTML-normalized body — fabricating those locally risks divergence from what the server actually stored and rendered. The refresh path already exists, is single-flight, and is fast; one round-trip after submit is an acceptable cost for guaranteed consistency. Optimistic UI is deferred to a measured follow-up.
status: Accepted
supersedes:
superseded_by:
tags: [tui, comments, consistency, refresh, mutation]
timestamp: 2026-06-28T00:00:00Z
---

# 0035. Server-truth refresh after a comment mutation (no optimistic UI)

## Context

A comment mutation ([PRD 0002](/prd/0002-task-comment-authoring.md)) changes the
server-side thread. The detail view holds `comments: Vec<Value>` and renders it into
the scrollable content. After a create/edit/delete succeeds, the on-screen thread
must reflect the change. There are two ways to get there:

1. **Optimistic local mutation** — push/replace/remove a synthetic entry in the local
   `comments` vector immediately, before (or instead of) consulting the server.
2. **Server-truth refresh** — on the write's 2xx, re-fetch the task+comments and
   replace the thread with the server's version.

The data the UI renders per comment is **server-owned**: the real `id`, `created_on`
timestamp, the resolved author display name (from the user map), and the
**HTML-normalized `body`** (ActiveCollab wraps/sanitizes the submitted plain text into
its stored HTML, which the rich-text mapper then renders). The app cannot reproduce
that normalization faithfully, and for create it does not know the server id/timestamp
at all.

The app **already has** a fast, correct refetch path:
`Cmd::LoadDetail { instance, project_id, task_id, refresh: true }` →
`spawn_load_detail` → `controller::load_task_core(refresh = true)`, which is
single-flight and writes through the task cache. This is the same path the `r`
(refresh) key uses.

## Decision

On a successful comment mutation, emit **`Cmd::LoadDetail { refresh: true }`** for the
open task and do **not** optimistically edit the local `comments` vector.

Concretely: `spawn_submit_comment` (and the edit/delete spawns) send
`Msg::CommentMutationOk` when the client write returns 2xx; the pure `update()` arm
for `CommentMutationOk` clears the compose/confirm state and returns
`vec![Cmd::LoadDetail { instance, project_id, task_id, refresh: true }]`. The existing
`LoadedDetail` handler then replaces `comments` (and `assets`, `task`) with the fresh
server payload, and the next reflow re-renders the thread — now including the created
comment with its real id/author/timestamp, the edited body as the server stored it, or
the deleted comment gone.

A failure (`Msg::CommentMutationErr`) does **not** refresh; it sets the compose
`status = Error(reason)` and preserves the typed buffer (ADR 0034) so the user can
retry.

### Guard / fitness function

- **Refresh on success (unit, headless):** an `update()` test asserts
  `CommentMutationOk` returns exactly one `Cmd::LoadDetail { refresh: true }` for the
  open task and clears compose. This is the load-bearing consistency gate.
- **No refresh on failure:** an `update()` test asserts `CommentMutationErr` returns
  no `LoadDetail` Cmd, keeps the buffer, and sets `Error`.
- **Thread re-derived from server:** the existing `LoadedDetail` handler tests already
  prove the thread is rebuilt from the payload; no synthetic comment is constructed
  anywhere (a grep-level invariant the reviewer checks — the mutation path constructs
  no `Comment`/`Value` to insert locally).

## Alternatives considered

- **Optimistic insert/replace/remove.** Rejected for the first version: it must
  fabricate the server-owned id, timestamp, author, and HTML-normalized body, risking a
  visible flip when the real values arrive and a divergence class of bugs ("wrong
  author/time on my just-posted comment"). It also duplicates rendering assumptions the
  server owns. The win (instant feedback) is real but the cost is a correctness hazard;
  PRD 0002 defers it to a measured follow-up once base latency is known.
- **Targeted re-fetch of just the comments collection** (`GET comments/task/{id}`)
  instead of the whole task. Rejected: marginally less data but a *new* code path and
  cache story, where `LoadDetail refresh` already exists, is single-flight, and keeps
  task + comments + assets coherent in one shot.
- **No refresh, trust the write response body** (the create/edit endpoints return the
  Comment). Rejected: it would still need to merge that single object into the local
  vector and resolve its author against the user map — i.e. a partial optimistic path —
  and gives nothing for delete. The full refresh is simpler and uniformly correct
  across all three mutations.

## Consequences

**Positive:** the displayed thread is always exactly what the server stored — no
fabricated fields, no divergence, uniform handling of create/edit/delete. Reuses the
audited, single-flight refresh path; the mutation arms stay tiny (set state + emit one
existing Cmd). Failure handling is local and lossless (buffer preserved).

**Accepted trade-offs:** one extra round-trip after each mutation (submit → 2xx →
refetch → repaint) — a brief "Submitting…/loading" beat instead of instant insertion.
This is the consistency-for-latency trade PRD 0002 accepts for v1; the optimistic
follow-up can layer on top without changing the write seam or the compose mode.

## Related

- PRD: [/prd/0002-task-comment-authoring.md](/prd/0002-task-comment-authoring.md)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md) (the write whose 2xx triggers the refresh)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md) (emits CommentMutationOk/Err)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md)
- ADR: [/adr/0017-task-list-first-paint-cache-swr-entry.md](/adr/0017-task-list-first-paint-cache-swr-entry.md) (the single-flight refresh discipline this reuses)
