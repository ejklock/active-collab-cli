---
type: Issue
title: "ac mcp: serve the CLI as MCP tools over stdio for parallel agent sessions"
description: Add ac mcp, an MCP stdio server whose read and write tools are adapters over the existing command cores, returning the --json contract; switch the store to WAL.
status: open
labels: [cli, agent, mcp, slice]
timestamp: 2026-11-11T21:40:56Z
---

## 0073. ac mcp: serve the CLI as MCP tools over stdio for parallel agent sessions

### Problem

The owner runs several agent sessions in parallel (Claude Code, Codex, OpenCode) and wants each to call active-collab-cli as MCP tools rather than through shell commands. Implements [ADR 0070](/adr/0070-ac-mcp-serves-mcp-over-stdio-as-an-adapter-over-the-command-cores.md).

### Scope

Included:

- `ac mcp`: an MCP server over stdio built on `rmcp`, spawned by each harness session.
- Read tools (`readOnlyHint: true`), each an adapter over the existing core with `json = true`: `get_task` (ref, `no_comments`, `refresh`, `download_attachments`, `attachments_dir`), `current_task` (optional `branch`, else the git branch of the server's working directory), `list_my_tasks` (optional instance).
- Write tools (`destructiveHint`), always exposed: `add_comment` (ref, body, `html`), `log_time` (ref, hours, date, summary, job type), `set_task` (ref, status, assignee, estimate), `time_report` (period or start/end, format).
- Every tool takes an optional `instance`.
- The store switches to `journal_mode=WAL`.
- `docs/architecture.md` gains the MCP adapter in its module and data-flow diagrams; the `active-collab` skill documents `ac mcp` and how to register it in each harness.

Excluded: an HTTP transport, MCP resources and prompts, the `browse` TUI, `setup` commands (token entry stays interactive), and the tools of the later issues, which ship in their own issues.

### Decision

- Tool names are snake_case verbs; schemas come from `rmcp`'s typed parameters.
- A non-zero core exit maps to `isError: true` with the core's `err` text; a 401 keeps the CLI's re-auth message.
- The server's working directory resolves `current_task`'s branch, as the CLI does.
- Order: this issue first, as the owner chose; each later issue adds its own tool.

### Acceptance

- AC1: `tools/list` returns the seven tools with their input schemas and annotations. (`verify_by: test`)
- AC2: each tool's result is byte-identical to the matching `--json` command run against the same mocked API. (`verify_by: test`)
- AC3: a core failure (bad ref, 404, 401) returns `isError` with the CLI's error text, and the server keeps serving. (`verify_by: test`)
- AC4: across every tool call, stdout carries only JSON-RPC frames. (`verify_by: test`)
- AC5: the store opens in WAL mode, and two `ac mcp` processes run tool calls at the same time without a lock error. (`verify_by: test`)
- AC6: registered in Claude Code and Codex, `list_my_tasks` and `get_task` answer against collab.base.digital. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
