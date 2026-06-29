---
type: ADR
title: A non-interactive `comment` CLI command posts a comment as the logged-in user, reusing the authenticated client seam and extending the agent --json contract to a write
description: Comment authoring (PRD 0002) only exists inside the interactive TUI compose modal, so an LLM/agent/script cannot post a comment non-interactively. Add a one-shot `comment` subcommand (parallel to get/current/mine) that resolves a task from an explicit ref or the current git branch, takes the body from a -m/--message flag or stdin, and posts it via the existing host-gated client.create_comment seam (ADR 0033) — attributing the comment to the logged-in user (the instance token owner). A --json flag emits a curated minified write result, extending the agent read contract (ADR 0011) to a write.
status: Accepted
supersedes:
superseded_by:
tags: [cli, comments, write, agent, json, non-interactive, llm]
timestamp: 2026-06-29T00:00:00Z
---

# 0040. A non-interactive `comment` command for agent/LLM comment authoring

## Context

[PRD 0002](/prd/0002-task-comment-authoring.md) added comment authoring (create / edit
/ delete), but **only through the interactive TUI** — the compose modal
([ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md),
[ADR 0034](/adr/0034-comment-compose-mode-multiline.md)). There is **no way for an LLM,
agent, or script to post a comment non-interactively**. The user asked for exactly that:
*"incluir um método não-TTY para LLM conseguir comentar na task. Apenas para o usuário
logado."*

Two existing seams make this a thin adapter rather than a new capability:

- **The authenticated write seam** ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)):
  `client.create_comment(task_id, body) -> (status, Option<Comment>)`
  (`src/client.rs`) over `Http::authed_post`, which attaches the token **only** to the
  instance host (`host_gated_token_header`, `src/http.rs`). This is the same seam the TUI
  compose submit already uses (`spawn_comment_write`, `src/tui/mod.rs`).
- **The agent `--json` contract for reads** ([ADR 0011](/adr/0011-agent-json-output-contract.md)):
  `agent_json::task_object` emits one curated, minified, non-interactive JSON line for
  `get`/`current`/`mine`/`browse`. We extend this contract to a **write result**.

The existing CLI already has the supporting pieces: `parse_task_ref` (URL or
`PROJECT_ID/TASK_ID` → ids, `src/commands.rs`), the current-git-branch task resolver the
`current` command uses, `pick_instance` (selects/validates the configured instance,
`src/commands.rs`), and a non-TTY stdin-read pattern (`dispatch_setup_add` +
`stdin_is_tty`, `src/main.rs`).

## Decision

Add a **one-shot, non-interactive `comment` subcommand** — `Command::Comment` in
`src/cli.rs`, dispatched by `dispatch_comment` in `src/main.rs`, parallel to
`get`/`current`/`mine`. It is a synchronous CLI write (NOT the TEA event loop).

### 1. Invocation contract

```
ac comment [TASK_REF] [-m|--message <TEXT>] [--json] [--instance <NAME>]
```

- **Task** (`TASK_REF`, optional positional): a task URL or `PROJECT_ID/TASK_ID`, resolved
  by `parse_task_ref`. **If omitted**, resolve from the **current git branch** (the same
  resolver `current` uses). If neither yields a task → error, non-zero exit, **no write**.
- **Body**: `-m/--message <TEXT>` if provided; **otherwise read the full body from stdin**
  (piped). If both are absent/empty → error `no comment body`, **exit code 2**, no write.
  stdin preserves multi-line bodies verbatim.
- **`--json`**: emit a curated, minified, single-line write result (below); otherwise a
  human one-line confirmation.
- **`--instance <NAME>`**: select the instance via `pick_instance`; otherwise the single /
  default configured instance.

### 2. Posts as the logged-in user (structural, not a flag)

The comment is created via `client.create_comment` using the **picked instance's
host-gated token**, so it is attributed to the **token owner = the logged-in user**. The
command **requires a configured instance** (`pick_instance` fails otherwise → "not logged
in"). There is **no author/impersonation option** — *"apenas para o usuário logado"* is
enforced structurally by the token, not by a parameter.

### 3. The `--json` write-result contract (extends ADR 0011 to a write)

On success, one minified line:

```json
{"ok":true,"comment_id":123,"task_id":75346,"project_id":524}
```

On failure with `--json`, a minified error object and a non-zero exit:

```json
{"ok":false,"error":"<reason>"}
```

`agent_json::comment_result` owns this shape (sibling of `task_object`).

### 4. Exit codes (mirror get/current)

- `0` — comment posted.
- `2` — usage/input error: no comment body, or no resolvable task.
- non-zero (runtime) — no configured instance / auth, or an HTTP `4xx`/`5xx` (no false
  success; the failure is reported on stderr or as the `--json` error object).

### 5. Decomposition (testable seam)

A pure-ish `comment_core(task_ref, body_source, instance, client, flags, out, err) ->
exit_code` mirrors `get_core`: it resolves the task, reads the body (flag or stdin),
calls `client.create_comment`, and writes either the human line or the `--json` result to
the injected writers. Unit-tested against a mocked client; `dispatch_comment` is the thin
I/O wiring (open store, load instances, pick instance, build client, read stdin).

## Guard / fitness function

- **One write path (deletion test):** the command calls the **same**
  `client.create_comment` the TUI uses — deleting the command leaves the TUI write intact;
  it is a non-interactive adapter, not a second write implementation.
- **Token host-isolation unchanged:** the post goes through `authed_post` /
  `host_gated_token_header` — the existing negative test (no token off-host) still covers
  it.
- **Non-interactive:** the command never prompts; body comes from `-m` or stdin; `--json`
  is minified and single-line (ADR 0011 parity), asserted by a unit test.
- **`comment_core` unit tests** (mocked client): `-m` body; stdin body (incl. multi-line);
  missing body → exit 2 + no `create_comment` call; `--json` vs human output; explicit ref
  vs current-branch resolution; HTTP failure → non-zero + no false success.

## Alternatives considered

- **Feed piped input into the TUI.** Rejected: the TUI is interactive (raw mode, event
  loop); an agent needs a one-shot command with a deterministic exit code and `--json`.
- **A generic `api`/`write` passthrough command.** Rejected: unscoped and unsafe; a
  focused `comment` command is the minimal surface for the asked-for capability.
- **An `--author`/`--as-user` flag.** Rejected: the instance token defines the identity;
  posting as another user is impersonation, out of scope, and a security risk. "Only the
  logged-in user" is the requirement.
- **Body only via `-m` (no stdin).** Rejected: multi-line bodies an LLM generates become
  shell-escaping pain; stdin is the natural non-TTY channel. Both are supported.

## Consequences

**Positive:** the app gains its **first non-interactive write** and extends the agent
surface from reads (ADR 0011) to a write, reusing the authenticated seam (ADR 0033) with
no new network boundary. An LLM can post a comment with `printf '…' | ac comment <ref>`
and parse `--json` for the new `comment_id`.

**Accepted trade-offs:** the `--json` write-result `{ok,comment_id,task_id,project_id}` is
a **new contract agents depend on** (specified in [BDR 0027](/bdr/0027-non-interactive-comment-creation.md)). A
stdin-read path is added to the CLI (the pattern already exists in `dispatch_setup_add`).
The command reports the created comment from the `create_comment` 2xx response; unlike the
TUI it does **not** re-render the thread (no server-truth refresh — ADR 0035 is a TUI
concern; the CLI is one-shot).

## Related

- PRD: [/prd/0002-task-comment-authoring.md](/prd/0002-task-comment-authoring.md) (the comment-authoring capability this extends to non-interactive use)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md) (the `create_comment` / host-gated `authed_post` seam reused)
- ADR: [/adr/0011-agent-json-output-contract.md](/adr/0011-agent-json-output-contract.md) (the curated `--json` contract extended to a write result)
- ADR: [/adr/0013-tty-gated-default-subcommand.md](/adr/0013-tty-gated-default-subcommand.md) (interactive vs non-interactive mode selection)
- BDR: [/bdr/0027-non-interactive-comment-creation.md](/bdr/0027-non-interactive-comment-creation.md)
- Issue: [/issues/0039-non-tty-comment-command.md](/issues/0039-non-tty-comment-command.md)
