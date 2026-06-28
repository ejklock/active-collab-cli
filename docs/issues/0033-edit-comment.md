---
type: Issue
title: "Edit your own comment — permission-aware [editar] affordance, pre-filled compose, authenticated PUT"
description: Slice 2 of comment authoring. Render an [editar] click target on each comment authored by the current user (created_by_id == instance.user_id); activating it opens the compose mode pre-filled with the comment body and carrying its comment_id; Ctrl+S issues Http::authed_put + ActiveCollabClient::update_comment (PUT /api/v1/comments/{comment_id}); on 2xx the thread refreshes. Reuses the slice-1 compose machine.
status: closed
labels: [tui, comments, edit, write, slice]
blocked_by: [0032]
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Edit your own comment — target, pre-fill, PUT, refresh

Slice 2 of [PRD 0002](/prd/0002-task-comment-authoring.md). Implements
[BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md) Scenarios 4 and 5 (and
the failure/isolation invariants for the edit path), under
[ADR 0036](/adr/0036-permission-aware-comment-targeting.md) (targeting),
[ADR 0034](/adr/0034-comment-compose-mode-multiline.md) (`ComposeKind::Edit`), and
[ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md) /
[ADR 0035](/adr/0035-server-truth-refresh-after-comment-mutation.md).

### Problem

A user who posts a comment with a typo (or wants to amend it) must leave the terminal;
there is no in-app edit. Editing also needs a way to **target one specific comment** in
the flat scrollable thread, and must not offer to edit comments the user does not own.

### Decision (from ADRs)

- **Targeting (ADR 0036):** render an `[editar]` click target in the header line of each
  comment where `created_by_id == instance.user_id`; none on others'. The target maps
  (via the line→comment map) to a `comment_id`, scroll-aware like the asset click.
- **Pre-filled compose (ADR 0034):** activating `[editar]` →
  `Msg::ComposeOpen(Edit{comment_id})`, which opens compose with `buffer` seeded from
  the comment's plain-text body. Ctrl+S → `ComposeSubmit` (now branch on
  `ComposeKind::Edit`).
- **Write + refresh (ADR 0033/0035):** `Http::authed_put`;
  `ActiveCollabClient::update_comment(comment_id, body)` →
  `PUT /api/v1/comments/{comment_id}` with `{ "body": body }`; `spawn_submit_comment`
  branches to `update_comment` for `Edit`; `CommentMutationOk` refreshes via
  `LoadDetail`.

### Scope

Included:

- `src/http.rs` — `authed_put` (+ token-gate test).
- `src/client.rs` — `update_comment`.
- `src/tui/model.rs` — `ComposeKind::Edit{comment_id}` handling in `ComposeOpen` and
  `ComposeSubmit`; the line→comment map / own-comment predicate used for targeting.
- `src/render.rs` (and/or `src/tui/screens/detail.rs`) — `[editar]` affordance in the
  comment header for own comments; its click target registration.
- `src/tui/model.rs` click hit-test — map an `[editar]` target click to
  `Msg::ComposeOpen(Edit{comment_id})`.
- `src/i18n/*` — `editar` label (en + pt-BR).
- Tests: `tests/unit/http.rs`, `tests/unit/client.rs`, `tests/unit/model.rs`,
  `tests/unit/tui_render.rs`.

Excluded: delete (issue 0034); the create path (issue 0032, reused); keyboard targeting
(deferred, ADR 0036).

### Acceptance

- AC1 — `update_comment` PUTs `comments/{id}` with `{"body":…}` and returns
  `(200, Some(comment))`; non-2xx returns the status with no parse (wiremock).
- AC2 — `authed_put` attaches the token on-host and omits it off-host (token isolation).
- AC3 — render + hit-test: an `[editar]` click target is present for a comment whose
  `created_by_id == user_id` and **absent** for a comment owned by another user.
- AC4 — `update()`: a click on an `[editar]` target emits
  `Msg::ComposeOpen(Edit{comment_id})`; `ComposeOpen(Edit{id})` seeds `buffer` from that
  comment's body and records `kind = Edit{id}`.
- AC5 — `update()`: `ComposeSubmit` with `kind = Edit{id}` emits the edit write carrying
  `id` (not a create); `CommentMutationOk` refreshes; `CommentMutationErr` keeps the
  buffer (lossless), per ADR 0035.
- AC6 — targeting is scroll-aware: at scroll offset O, the `[editar]` target row maps to
  the correct `comment_id` (reuses the asset click-map translation).
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  functions over explanatory comments) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior and survive the mutation floor on changed lines
  (`verify_by: command`).

### Plan

1. `Http::authed_put` + token-gate test.
2. `update_comment` + wiremock contract test.
3. Own-comment predicate + line→comment map; render `[editar]` target on own comments
   (buffer-derived render + absence test).
4. Hit-test arm: `[editar]` click → `ComposeOpen(Edit{id})`; pre-fill in `update()`.
5. `ComposeSubmit` Edit branch + `spawn_submit_comment` `update_comment` path.
6. i18n `editar` string.

Observable end-to-end: open a task with your own comment, click `[editar]`, change the
text, Ctrl+S, and the thread reloads with the edited comment.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
