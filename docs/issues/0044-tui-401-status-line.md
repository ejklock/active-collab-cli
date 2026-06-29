---
type: Issue
title: "TUI surfaces HTTP 401 in the thin status line → guide the user to `ac setup add` (slice 3)"
description: On a detail load/refresh or a comment mutation that returns HTTP 401, the TUI sets an auth-error flag whose message renders in the existing thin status line (ADR 0038), pointing the user to re-authenticate with `ac setup add`, instead of silently showing an empty detail or the generic "failed to post comment" copy. No in-app re-auth modal. Reuses the HTTP_UNAUTHORIZED constant and the i18n message from issue 0042.
status: closed
labels: [auth, tui, error-handling, slice]
blocked_by: 0042, 0043
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## TUI 401 → status-line re-auth guidance

Implements [BDR 0029](/bdr/0029-token-invalidation-reauthentication.md) Scenarios 5, 6, 7
under [ADR 0042](/adr/0042-detect-401-and-guide-reauthentication.md). Reuses the
`HTTP_UNAUTHORIZED` constant and the i18n message from issue 0042 (no new locale key).

### Problem

The TUI detail load collapses non-200 to `(Value::Null, vec![])`
(`controller::load_task_data_from_path`) and `spawn_load_detail` sends
`Msg::LoadedDetail` anyway with `task = Null` — so a revoked token paints an empty
detail with no explanation. A 401 comment mutation surfaces only the generic
`CommentMutationErr` ("failed to post comment"). Neither tells the user the token is
invalid or how to recover.

### Decision (from ADR)

A 401 from the TUI load/refresh or a mutation sets an **auth-error** state whose message
renders in the existing **thin status line** ([ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md)):
the shared re-auth message pointing to `ac setup add`. The controller load path
propagates the 401 distinctly (instead of only the collapsed `Null`); the mutation path
detects `status == HTTP_UNAUTHORIZED`. No modal; re-auth happens outside the app.

### Scope

Included:

- `src/controller.rs` — the detail load path (`fetch_from_network` / `load_task_data_from_path`
  / `load_task_core`) propagates a distinct **unauthorized** signal on a 401 fetch
  (instead of collapsing it indistinguishably into the empty `Null` result).
- `src/tui/model.rs` — `DetailLoad` gains `unauthorized: bool`; `Screen::Detail` gains an
  `auth_error: bool` field. `update()` sets `auth_error` from `LoadedDetail.unauthorized`
  (cleared on a 200 load) and from a new `Msg::AuthExpired`.
- `src/tui/mod.rs` — `spawn_load_detail` fills `DetailLoad.unauthorized` from the fetch
  status; `spawn_comment_write` sends `Msg::AuthExpired` when a mutation returns
  `HTTP_UNAUTHORIZED` (distinct from the generic `CommentMutationErr`).
- `src/tui/view.rs` — `detail_status_line` returns the re-auth `t(...)` message when
  `auth_error` is set (priority over copied-feedback; shown for the auth case instead of
  the generic write error).
- Tests: `tests/unit/tui_render.rs` — buffer-derived: a 401 load result renders the
  re-auth message in the status line; a 401 mutation sets `auth_error` and renders it
  (and retains the compose buffer); a 200 load clears it.

Excluded: any in-app re-auth modal/password prompt; the CLI (issues 0042/0043). The
TUI mine-list load keeps `.unwrap_or_default()` (issue 0043) — this slice covers the
detail load/refresh and the mutation paths.

### Acceptance

- AC1 — TUI detail-load 401 (`verify_by: test`): driving the load result with a 401
  outcome sets `Screen::Detail.auth_error` and the rendered thin status line shows the
  re-auth message (asserted on the TestBackend buffer).
- AC2 — TUI mutation 401 (`verify_by: test`): a comment mutation returning 401 emits
  `Msg::AuthExpired`, sets `auth_error`, renders the re-auth message (distinct from the
  generic write-error copy), and **retains the compose buffer**.
- AC3 — cleared on success (`verify_by: test`): a subsequent 200 detail load clears
  `auth_error` (the status line no longer shows the re-auth message).
- AC4 — i18n (`verify_by: test`): the rendered message is the pt-BR translation under
  the pt-BR locale; the English source key is identity.
- CC — clean code (named `auth_error`/`AuthExpired`; no magic `401`; no banners/commented-out)
  (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior (rendered status-line cells; the set/clear of
  `auth_error`; buffer retention) and survive the mutation floor (dropping the auth
  branch, or rendering the generic copy, fails) (`verify_by: command`).

### Plan

1. `controller.rs`: propagate a distinct unauthorized signal from the detail fetch on a
   401 (carry the status out of `fetch_from_network` far enough for the load to mark it).
2. `model.rs`: add `DetailLoad.unauthorized` + `Screen::Detail.auth_error` + a
   `Msg::AuthExpired`; `update()` sets/clears `auth_error` accordingly (pure).
3. `mod.rs`: `spawn_load_detail` fills `unauthorized` from the fetch status;
   `spawn_comment_write` sends `Msg::AuthExpired` on `HTTP_UNAUTHORIZED`.
4. `view.rs`: `detail_status_line` renders the re-auth `t(...)` message when `auth_error`.
5. Tests: buffer-derived 401 load + 401 mutation + 200-clear + pt-BR mapping.

Observable end-to-end: open a task whose token was revoked → the status line reads
"Token inválido ou revogado — rode `ac setup add` para reautenticar." instead of an
empty detail.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
