---
type: Issue
title: "Delete your own comment — permission-aware [excluir] affordance, inline confirm, authenticated DELETE"
description: Slice 3 of comment authoring. Render an [excluir] click target on each comment authored by the current user; activating it opens an inline confirm on the Detail screen (no write yet); confirming issues Http::authed_delete + ActiveCollabClient::delete_comment (DELETE /api/v1/comments/{comment_id}); on 2xx the thread refreshes without the comment; cancelling leaves it. Closes the comment-authoring feature.
status: closed
labels: [tui, comments, delete, write, slice]
blocked_by: [0032]
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Delete your own comment — target, confirm, DELETE, refresh

Slice 3 of [PRD 0002](/prd/0002-task-comment-authoring.md). Implements
[BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md) Scenario 6 (and the
isolation/failure invariants for the delete path), under
[ADR 0036](/adr/0036-permission-aware-comment-targeting.md) (targeting + confirm),
[ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md) (DELETE), and
[ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md) (refresh).

### Problem

There is no in-app way to remove a comment the user posted by mistake, and a delete is
destructive — it needs a targeting mechanism (own comments only) and a confirmation so a
stray click cannot drop a comment.

### Decision (from ADRs)

- **Targeting + confirm (ADR 0036):** render an `[excluir]` click target on each comment
  where `created_by_id == instance.user_id`; activating it →
  `Msg::DeleteCommentRequest{comment_id}`, which sets `confirm_delete: Option<i64>` on
  `Screen::Detail` and emits **no** write. A confirm action emits
  `Cmd::DeleteComment{comment_id}`; cancel clears `confirm_delete`.
- **Write + refresh (ADR 0033/0035):** `Http::authed_delete`;
  `ActiveCollabClient::delete_comment(comment_id)` →
  `DELETE /api/v1/comments/{comment_id}`, returning the status; `spawn_delete_comment`
  sends `CommentMutationOk` on 2xx (→ `LoadDetail refresh`) or
  `CommentMutationErr(reason)`.

### Scope

Included:

- `src/http.rs` — `authed_delete` (+ token-gate test).
- `src/client.rs` — `delete_comment`.
- `src/tui/model.rs` — `confirm_delete` state on `Screen::Detail`;
  `Msg::DeleteCommentRequest` / confirm / cancel; `Cmd::DeleteComment`; pure `update()`
  arms.
- `src/render.rs` (and/or `src/tui/screens/detail.rs`) — `[excluir]` affordance on own
  comments + the inline confirm prompt rendering.
- `src/tui/model.rs` click hit-test — `[excluir]` target click → `DeleteCommentRequest`.
- `src/tui/mod.rs` — `spawn_delete_comment` + dispatch.
- `src/i18n/*` — `excluir` label + confirm prompt (en + pt-BR).
- Tests: `tests/unit/http.rs`, `tests/unit/client.rs`, `tests/unit/model.rs`,
  `tests/unit/tui_render.rs`.

Excluded: create (issue 0032) and edit (issue 0033); keyboard targeting (deferred).

### Acceptance

- AC1 — `delete_comment` DELETEs `comments/{id}` and returns the status (2xx on success;
  the status on failure) (wiremock).
- AC2 — `authed_delete` attaches the token on-host and omits it off-host (token isolation).
- AC3 — render + hit-test: an `[excluir]` click target is present for a comment whose
  `created_by_id == user_id` and absent for a comment owned by another user.
- AC4 — `update()`: `DeleteCommentRequest{id}` sets `confirm_delete = Some(id)` and emits
  no write Cmd; the confirm action emits exactly one `Cmd::DeleteComment{id}`; cancel
  clears `confirm_delete` and emits nothing.
- AC5 — `update()`: `CommentMutationOk` (after delete) emits `Cmd::LoadDetail{refresh:true}`;
  `CommentMutationErr(msg)` surfaces the error and emits no refresh (per ADR 0035).
- AC6 — the inline confirm prompt renders (buffer-derived) when `confirm_delete` is set
  and is absent otherwise.
- CC — clean code (no superfluous comments / banners / commented-out code) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior and survive the mutation floor on changed lines
  (`verify_by: command`).

### Plan

1. `Http::authed_delete` + token-gate test.
2. `delete_comment` + wiremock contract test.
3. `confirm_delete` state + `DeleteCommentRequest`/confirm/cancel `update()` arms + tests.
4. Render `[excluir]` target on own comments + inline confirm prompt (buffer-derived test).
5. Hit-test arm: `[excluir]` click → `DeleteCommentRequest`.
6. `Cmd::DeleteComment` + `spawn_delete_comment` + dispatch; i18n strings.

Observable end-to-end: open a task with your own comment, click `[excluir]`, confirm, and
the thread reloads without it.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
