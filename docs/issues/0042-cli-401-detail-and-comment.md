---
type: Issue
title: "CLI detail + comment detect HTTP 401 → actionable re-auth message + non-zero exit (slice 1)"
description: In the CLI read/write paths that already expose the HTTP status — load_task (get/current) and comment_core (comment) — branch on status == 401 and print an actionable re-authentication message instructing the user to run `ac setup add`, exiting non-zero, instead of the current "task not found"/generic-HTTP output. Introduces the shared HTTP_UNAUTHORIZED constant and the single i18n re-auth message. Other non-200 responses keep their current behavior.
status: closed
labels: [auth, cli, error-handling, i18n, slice]
blocked_by:
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## CLI 401 → re-auth (detail + comment)

Implements [BDR 0029](/bdr/0029-token-invalidation-reauthentication.md) Scenarios 1, 2, 4, 7
under [ADR 0042](/adr/0042-detect-401-and-guide-reauthentication.md).

### Problem

When the stored token is revoked, `ac get`/`ac current` print the generic
`task not found (HTTP 401)` and `ac comment` prints a generic `HTTP 401 posting comment`.
Neither tells the user the token is invalid or how to recover. Both already have the
status in hand (`load_task` checks `status != 200` at `commands.rs:521`; `comment_core`
branches on status at `commands.rs:789`), so this slice needs no client-signature change.

### Decision (from ADR)

A 401 is a distinct condition. In the two CLI paths that already expose the status,
branch on `status == HTTP_UNAUTHORIZED` (a shared `401` constant) and emit the single
shared re-auth message via `i18n::t()`, exiting non-zero. Other non-200 is unchanged.

### Scope

Included:

- A shared constant `HTTP_UNAUTHORIZED: u16 = 401` (place next to the HTTP/transport
  code, e.g. `src/http.rs`, and reuse it — no magic `401` literals).
- `src/commands.rs` — `load_task`: on `status == HTTP_UNAUTHORIZED`, print the re-auth
  message (to the same writer the "not found" line uses) and ensure the caller path
  (`get_core`/`current_core`/`do_get_task`) yields a **non-zero** exit; do NOT also print
  the "task not found" line for a 401. `comment_core`: on `status == HTTP_UNAUTHORIZED`,
  print the re-auth message (and the `--json` failure shape, no `"ok":true`) and exit
  non-zero, distinct from the generic HTTP-error branch.
- `locales/pt_BR.json` — add the single key
  `"Token invalid or revoked — run \`ac setup add\` to re-authenticate."` →
  `"Token inválido ou revogado — rode \`ac setup add\` para reautenticar."`.
- Tests: `tests/unit/commands.rs` (mocked-server 401 for get/current and comment).

Excluded: `mine` (issue 0043, collapses in `fetch_open_tasks`); the TUI (issue 0044);
any in-app re-auth modal.

### Acceptance

- AC1 — `get`/`current` 401 (`verify_by: test`): with a MockServer returning 401 for the
  task fetch, `load_task`/`get_core` prints the re-auth message (asserted on the writer),
  does NOT print "task not found", and the command yields a non-zero exit.
- AC2 — `comment` 401 (`verify_by: test`): with a MockServer returning 401 for the
  create-comment POST, `comment_core` prints the re-auth message and exits non-zero; with
  `--json`, the output is the failure shape (no `"ok": true`).
- AC3 — other non-200 unchanged (`verify_by: test`): a 404/500 keeps the existing
  output (e.g. "task not found (HTTP 404)") with no re-auth message (regression).
- AC4 — i18n (`verify_by: test`): the message resolves through `i18n::t()`; pt-BR maps
  the key to the translated string; the English source key is identity.
- CC — clean code (no superfluous comments / banners / commented-out code; no magic
  `401` literal — use the named constant) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable output (printed message, exit code, JSON shape) and
  survive the mutation floor (changing the status check or dropping the message fails)
  (`verify_by: command`).

### Plan

1. Add `pub const HTTP_UNAUTHORIZED: u16 = 401;` next to the HTTP transport code.
2. `commands.rs::load_task`: special-case `HTTP_UNAUTHORIZED` before the generic
   non-200 branch → print `t("Token invalid or revoked — run \`ac setup add\` to re-authenticate.")`;
   propagate a non-zero exit through `get_core`/`current_core`/`do_get_task`.
3. `commands.rs::comment_core`: special-case `HTTP_UNAUTHORIZED` in the failure branch →
   re-auth message + non-zero exit + `--json` failure shape.
4. `locales/pt_BR.json`: add the key → pt-BR value.
5. Tests: mocked-server 401 for get/current and comment; 404 regression; pt-BR mapping.

Observable end-to-end: `ac get <task>` (or `ac comment ...`) with a revoked token prints
"Token inválido ou revogado — rode `ac setup add` para reautenticar." and exits non-zero.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
