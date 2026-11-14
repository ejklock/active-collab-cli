---
type: Issue
title: "ac mcp: serve the CLI as MCP tools over stdio for parallel agent sessions"
description: Add ac mcp, an MCP stdio server whose read and write tools are adapters over the existing command cores, returning the --json contract; switch the store to WAL.
status: closed
labels: [cli, agent, mcp, slice]
timestamp: 2026-11-11T21:40:56Z
---

## 0073. ac mcp: serve the CLI as MCP tools over stdio for parallel agent sessions

### Problem

The owner runs several agent sessions in parallel (Claude Code, Codex, OpenCode) and wants each to call active-collab-cli as MCP tools rather than through shell commands. Implements [ADR 0070](/adr/0070-ac-mcp-serves-mcp-over-stdio-as-an-adapter-over-the-command-cores.md).

### Scope

Included:

- `ac mcp`: an MCP server over stdio built on `rmcp`, spawned by each harness session.
- Two tools, each selecting a command core by an `operation` field; every operation is an adapter over the existing core with `json = true`.
- `ac_read` (`readOnlyHint: true`): `get` (ref, `no_comments`, `refresh`, `download_attachments`, `attachments_dir`), `current` (optional `branch`, else the git branch of the server's working directory), `mine`.
- `ac_write` (`destructiveHint: true`), always exposed: `comment` (ref, body, `html`), `log_time` (ref, hours, date, summary, job type), `set_task` (ref, status, assignee, estimate), `time_report` (period or start/end, format).
- Every operation takes an optional `instance`.
- The store switches to `journal_mode=WAL`.
- `docs/architecture.md` gains the MCP adapter in its module and data-flow diagrams; the `active-collab` skill documents `ac mcp` and how to register it in each harness.

Excluded: an HTTP transport, MCP resources and prompts, the `browse` TUI, `setup` commands (token entry stays interactive), and the operations and parameters of the later issues, which ship in their own issues.

### Decision

- Two tools grouped by operation type, `ac_read` and `ac_write`, as the owner chose to save context tokens in every session; over one tool per verb (seven tools, growing with each issue) and over a single tool for everything, which would lose the harness's separate approval of reads and writes.
- Each tool's parameters are a union discriminated by `operation`, its schema from `rmcp`'s typed parameters. If Claude Code or Codex mishandles the union schema, the fallback is flat optional parameters plus `operation`, validated by the server.
- An unknown `operation` or a parameter missing for its operation is an `invalid_params` protocol error.
- A non-zero core exit maps to `isError: true` with the core's `err` text; a 401 keeps the CLI's re-auth message.
- The server's working directory resolves the `current` operation's branch, as the CLI does.
- Order: this issue first, as the owner chose; each later issue adds its parameters or operations to these two tools.
- Downloads move to a third tool, `ac_download_attachments` (not read-only, not destructive, idempotent), as the owner chose on 2026-11-11, because MCP annotations are per tool and `ac_read`'s `readOnlyHint` was false for `get` with `download_attachments`; over an operation in `ac_read` (same false hint) and an operation in `ac_write` (a destructive warning for a download). See [ADR 0072](/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md) and [issue 0080](/issues/0080-move-the-mcp-attachment-download-to-its-own-ac-download-attachments-tool.md).

- The live acceptance criterion verifies against the mock ActiveCollab of [issue 0084](/issues/0084-replay-the-live-acceptance-criteria-as-terminal-walkthroughs-against-a-mock-activecollab.md) (owner, 2026-11-12): the owner's `ac` is connected to the real instance, which no check may touch. Risk accepted: the mock returns the JSON shapes `tests/mcp_stdio.rs` mounts, so a real API that diverges from them still passes the walkthrough. Not taken: the mock plus live read-only reads.

### Acceptance

- AC1: `tools/list` returns `ac_read` and `ac_write` with their operation-discriminated input schemas and annotations. (`verify_by: test`)
- AC2: each operation's result is byte-identical to the matching `--json` command run against the same mocked API. (`verify_by: test`)
- AC3: a core failure (bad ref, 404, 401) returns `isError` with the CLI's error text, and the server keeps serving. (`verify_by: test`)
- AC4: across every operation, stdout carries only JSON-RPC frames. (`verify_by: test`)
- AC5: the store opens in WAL mode, and two `ac mcp` processes run tool calls at the same time without a lock error. (`verify_by: test`)
- AC6: the MCP Inspector CLI lists the tools of `ac mcp`, and `ac_read` `mine` and `get` answer against the mock ActiveCollab, replayed by a terminal walkthrough. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
