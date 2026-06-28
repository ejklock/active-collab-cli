---
type: Issue
title: "Create a comment on the open task — multi-line compose, authenticated POST, server-truth refresh"
description: Slice 1 of comment authoring. Add the authenticated write seam (Http::authed_post + ActiveCollabClient::create_comment), the multi-line compose mode on Screen::Detail (c opens; Enter=newline; Ctrl+S submits; Esc cancels), the Cmd::SubmitComment effect + spawn, and the on-success LoadDetail refresh. Render the compose area + footer hint. The first WRITE operation in the app.
status: closed
labels: [tui, comments, write, compose, slice]
blocked_by:
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Create a comment — compose + POST + refresh

Slice 1 of [PRD 0002](/prd/0002-task-comment-authoring.md). Implements
[BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md) Scenarios 1–3, 7, 8 for
the **create** path, under [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md),
[ADR 0034](/adr/0034-comment-compose-mode-multiline.md), and
[ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md).

### Problem

The app can read a task's comment thread but cannot add to it; a user must leave the
terminal for the web UI to respond on a task.

### Decision (from ADRs)

- **Write seam (ADR 0033):** `Http::authed_post(url, base, token, body)` reusing
  `host_gated_token_header`; `ActiveCollabClient::create_comment(task_id, body)` →
  `POST /api/v1/comments/task/{task_id}` with `{ "body": body }`, returning
  `(status, Option<Value>)`.
- **Compose mode (ADR 0034):** `compose: Option<Compose>` on `Screen::Detail`;
  `'c'` → `Msg::ComposeOpen(New)`; the shell picks `map_compose_key_event` while
  composing (printable → `ComposeInput`, Enter → `ComposeNewline`, Backspace →
  `ComposeBackspace`, Ctrl+S → `ComposeSubmit`, Esc → `ComposeCancel`); `update()`
  stays pure.
- **Submit + refresh (ADR 0035):** `ComposeSubmit` emits `Cmd::SubmitComment` and sets
  `Submitting`; `spawn_submit_comment` calls `create_comment` and sends
  `Msg::CommentMutationOk` (2xx) or `Msg::CommentMutationErr(reason)`;
  `CommentMutationOk` clears compose and emits `Cmd::LoadDetail { refresh: true }`;
  `CommentMutationErr` keeps the buffer and sets `Error`.
- **Render:** a compose area (the multi-line buffer) and a footer hint
  (`Ctrl+S enviar · Esc cancelar`), localized (en + pt-BR).

### Scope

Included:

- `src/http.rs` — `authed_post` (+ token-gate test surface).
- `src/client.rs` — `create_comment`.
- `src/tui/model.rs` — `Compose`/`ComposeKind`/`ComposeStatus`, `Screen::Detail.compose`,
  `Cmd::SubmitComment`, the compose Msgs + `CommentMutationOk`/`CommentMutationErr`,
  the pure `update()` arms.
- `src/tui/events.rs` — `map_compose_key_event`; the `'c'` open arm.
- `src/tui/mod.rs` — mode-aware `handle_input_event`; `spawn_submit_comment` + dispatch.
- `src/render.rs` (and/or `src/tui/screens/detail.rs`) — compose-area + hint rendering.
- `src/i18n/*` — compose hint + error strings (en + pt-BR).
- Tests: `tests/unit/http.rs`, `tests/unit/client.rs`, `tests/unit/model.rs`,
  `tests/unit/tui_render.rs` (or `render.rs`).

Excluded: edit (issue 0033) and delete (issue 0034); the `[editar]`/`[excluir]`
affordances and `ComposeKind::Edit` wiring (defined in ADR 0034/0036, implemented in
their slices); optimistic UI (deferred, ADR 0035); a CLI comment command.

### Acceptance

- AC1 — `create_comment` POSTs `comments/task/{id}` with `{"body":…}` and returns
  `(200, Some(comment))`; a non-2xx returns the status with no parse (wiremock).
- AC2 — `authed_post` attaches `X-Angie-AuthApiToken` when the request host equals the
  instance host and **omits** it off-host (negative test; token isolation).
- AC3 — `update()`: `ComposeOpen(New)` opens compose; `ComposeInput`/`ComposeNewline`/
  `ComposeBackspace` mutate a multi-line buffer (Enter yields `\n`, not a submit);
  `ComposeCancel` clears compose and emits no Cmd.
- AC4 — `update()`: `ComposeSubmit` on a non-empty buffer emits `Cmd::SubmitComment{
  task_id, body }` and sets `Submitting`; an empty buffer does not submit.
- AC5 — `update()`: `CommentMutationOk` emits exactly one `Cmd::LoadDetail{refresh:true}`
  and clears compose; `CommentMutationErr(msg)` keeps the buffer, sets `Error(msg)`, and
  emits no `LoadDetail`.
- AC6 — render (TestBackend, buffer-derived): with compose active, the typed multi-line
  body and the localized footer hint are visible; with compose inactive, neither is.
- CC — clean code: no superfluous comments; extract well-named functions over
  explanatory comments; no banners or commented-out code; only non-obvious why-comments
  (`verify_by: inspection`).
- CX — every new/changed function stays within the complexity budget (cyclomatic ≤ 10,
  ≤ 8 for new; cognitive ≤ gate threshold) (`verify_by: command`).
- TE — tests assert observable behavior (Cmds, buffer, status, rendered cells), not
  implementation, and survive the mutation floor on changed lines (`verify_by: command`).

### Plan

1. `Http::authed_post` + token-gate test (mirror `authed_get`).
2. `create_comment` + wiremock contract test.
3. Compose state + Msgs + pure `update()` arms + tests.
4. `map_compose_key_event` + the `'c'` open arm + mode-aware `handle_input_event`.
5. `Cmd::SubmitComment` + `spawn_submit_comment` + `CommentMutationOk`/`Err` wiring.
6. Compose-area + hint rendering + i18n strings + buffer-derived render test.

Observable end-to-end: open a task, press `c`, type a multi-line comment, Ctrl+S, and
the new comment appears in the thread (loaded from the server).

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
