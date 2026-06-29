---
type: Issue
title: "Non-TTY `comment` command — post a comment to a task as the logged-in user (-m or stdin, --json result)"
description: Add a one-shot, non-interactive `comment` subcommand so an LLM/agent/script can post a comment to a task. Resolves the task from an explicit ref (URL or PROJECT_ID/TASK_ID) or the current git branch; takes the body from -m/--message or stdin; posts via the existing host-gated client.create_comment seam as the logged-in user (token owner); prints a human confirmation or, with --json, a minified {"ok":true,"comment_id":N,"task_id":N,"project_id":N}. Empty body / no task / no instance / HTTP error all exit non-zero with no false success.
status: open
labels: [cli, comments, write, agent, json, non-interactive, slice]
blocked_by:
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## Non-TTY comment command

Implements [BDR 0027](/bdr/0027-non-interactive-comment-creation.md) under
[ADR 0040](/adr/0040-non-interactive-comment-write-command.md), reusing the authenticated
write seam ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)) and extending
the agent `--json` contract ([ADR 0011](/adr/0011-agent-json-output-contract.md)) to a
write result.

### Problem

Comment authoring is TUI-only (compose modal). An LLM/agent/script has no non-interactive
way to post a comment to a task.

### Decision (from ADR)

A one-shot `comment` subcommand (NOT the TEA loop), parallel to `get`/`current`/`mine`:
`ac comment [TASK_REF] [-m|--message <TEXT>] [--json] [--instance <NAME>]`. Task from the
explicit ref or the current git branch; body from `-m` or stdin; posted via
`client.create_comment` with the instance's host-gated token (as the logged-in user);
human or `--json` `{"ok":true,"comment_id":N,"task_id":N,"project_id":N}` output; exit `0`
on success, `2` on usage error (no body / no task), non-zero on runtime failure.

### Scope

Included:

- `src/cli.rs` — add `Command::Comment(CommentArgs)`; `CommentArgs { task_ref: Option<String>,
  message: Option<String>, json: bool, instance: Option<String> }` (mirror `GetArgs`).
- `src/main.rs` — `dispatch_comment`: open store, load instances, `pick_instance`, build
  client, resolve the body (flag or stdin via the `stdin_is_tty`/read pattern), call
  `comment_core`; wire it into `dispatch`.
- `src/commands.rs` — `comment_core(task_ref, body, instance, client, json, out, err) ->
  exit_code`: resolve the task (`parse_task_ref` or the current-branch resolver `current`
  uses), call `client.create_comment`, write the human line or delegate to the `--json`
  shaper; map failures to exit codes.
- `src/agent_json.rs` — `comment_result(comment_id, task_id, project_id) -> String` (and an
  error shape) for the minified `--json` line (sibling of `task_object`).
- Tests: `tests/unit/commands.rs` (or the existing command tests module) against a mocked
  client; `tests/unit/agent_json.rs` for the result shape.

Excluded: editing/deleting comments via CLI (this issue is create-only); any change to the
TUI write path or the `create_comment` client seam (reused unchanged); posting as another
user (out of scope by ADR 0040).

### Acceptance

- AC1 — `comment_core` with `-m "text"` on an explicit `PROJECT_ID/TASK_ID` (or URL) calls
  `create_comment(task_id, "text")` once and writes a confirmation; exit 0. (`verify_by: test`)
- AC2 — with no `-m`, the body is read from stdin (multi-line preserved) and passed verbatim
  to `create_comment`; exit 0. (`verify_by: test`)
- AC3 — with `--json` on success, stdout is exactly one minified line
  `{"ok":true,"comment_id":N,"task_id":N,"project_id":N}` and nothing else; exit 0. (`verify_by: test`)
- AC4 — empty/absent body (no `-m`, empty stdin) exits `2` with `no comment body` and makes
  **no** `create_comment` call. (`verify_by: test`)
- AC5 — with no `TASK_REF`, the task is resolved from the current git branch (the resolver
  `current` uses); a non-resolvable task exits non-zero with no write. (`verify_by: test`)
- AC6 — no configured instance exits non-zero ("not logged in") with no write; the post is
  attributed to the logged-in user (instance token owner) and the token is sent only to the
  instance host. (`verify_by: test`)
- AC7 — an HTTP `4xx`/`5xx` exits non-zero and reports the failure (human stderr or `--json`
  `{"ok":false,"error":...}`); it never prints a success line. (`verify_by: test`)
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  helpers `comment_core`, `comment_result`) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ 12) (`verify_by: command`).
- TE — tests assert observable behavior (the mocked `create_comment` call + the written
  output + exit codes) and survive the mutation floor on changed lines (`verify_by: command`).

### Plan

1. `cli.rs`: add `Command::Comment(CommentArgs)` + the `CommentArgs` struct (positional
   optional task ref, `-m/--message`, `--json`, `--instance`), mirroring `GetArgs`.
2. `agent_json.rs`: add `comment_result` (success) and an error shaper for the minified line.
3. `commands.rs`: add `comment_core` — task resolution (explicit ref / current branch),
   body required-check, `create_comment` call, output (human / `--json`), exit-code mapping.
4. `main.rs`: add `dispatch_comment` (store + instances + `pick_instance` + client + stdin
   body read) and wire it into `dispatch`.
5. Tests: `comment_core` against a mocked client for AC1–AC7; `comment_result` shape test.

Observable end-to-end: `printf 'Deploy em homolog.' | ac comment 524/75346 --json` prints
`{"ok":true,"comment_id":<id>,"task_id":75346,"project_id":524}` and the comment appears on
the task, authored by the logged-in user.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
