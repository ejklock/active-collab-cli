---
type: ADR
title: ac mcp serves MCP over stdio as an adapter over the command cores
description: An ac mcp subcommand serves MCP over stdio with rmcp; each tool calls an existing command core with json output, so the --json contract is the only agent contract; read and write tools always exposed with MCP annotations; the store moves to WAL.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-11T21:40:48Z
---

# 0070. ac mcp serves MCP over stdio as an adapter over the command cores

## Context

Agents reach active-collab-cli today through the shell: an agent harness runs `ac … --json` and reads the curated minified contract of [ADR 0011](/adr/0011-agent-json-output-contract.md), taught by the `active-collab` skill served by `ac skill` ([ADR 0057](/adr/0057-agent-skill-served-by-ac-skill-command.md)). The owner runs several agent sessions in parallel (Claude Code, Codex, OpenCode and others), on different tasks or on the same task from different fronts, and wants every one of them to call the CLI's capabilities as Model Context Protocol (MCP) tools instead of composing shell commands.

The MCP specification fixes the protocol (JSON-RPC 2.0, `tools/list`, `tools/call`) and offers two transports: stdio, where the client spawns the server as a child process, and Streamable HTTP, where the server listens on a port. The constitution fixes single-binary distribution, token host isolation and no telemetry. The owner ranked the driving characteristics as token security first, a single agent contract (MCP output equals the `--json` output) second, and distribution (one binary, no daemon) third.

Every command already has a testable core that writes to injected `out`/`err` writers and returns an exit code (`get_core`, `current_core`, `mine_core`, `comment_core`, `time_log_core`, `task_set_core`, `time_report_core`). The local SQLite store runs with `journal_mode=DELETE` and `busy_timeout=5000` (`src/store/mod.rs`).

## Decision

We will add an `ac mcp` subcommand that serves MCP over stdio with the official Rust SDK `rmcp` (3.9.0 on crates.io, probed 2026-11-11). Each MCP tool is a driving adapter over an existing command core: it calls the core with `json = true` and in-memory writers, returns the core's `out` buffer as the tool result on exit 0, and returns the `err` buffer as an `isError` result otherwise. No tool builds its own JSON; the `--json` contract of `src/agent_json.rs` is the only agent contract.

Read and write tools are always exposed, with no opt-in flag, as the owner chose. Each tool carries the MCP annotations: `readOnlyHint: true` on reads, `destructiveHint` on writes, so harnesses can require approval for writes only. Write tools go through the same host-gated write seam as the CLI ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)). The store moves to `journal_mode=WAL` so parallel `ac mcp` processes read and write the cache without waiting on each other.

Rejected alternatives:

- **Keep the skill and shell only** (the existing pattern): no new contract, but every agent composes shell commands and parses exit codes, and harness permission rules cannot tell a read from a write inside one `Bash` call.
- **Streamable HTTP on a local port** (`ac mcp --port N`): one shared process for every session, but any local process, or a web page through DNS rebinding, can call the write tools with the owner's token unless the server adds its own authentication and `Origin` checks, and it needs a daemon lifecycle. Stdio gives each parallel session its own process with the same result.
- **Writes behind `--allow-writes`**: the owner declined it; the per-tool permission rules of each harness are the control.
- **A hand-written JSON-RPC loop**: the official SDK covers the handshake, schemas and transport; owning a protocol implementation is not worth it.
- **Tools that build their own JSON**: two contracts that drift; rejected for the single-contract characteristic.

## Consequences

**Easier / gained:**
- Every agent session gets typed tools with schemas and per-tool permissions, in parallel, with no network listener.
- One contract: a change to `--json` is a change to MCP, and the reverse.

**Harder / accepted trade-offs:**
- A new dependency tree (`rmcp` and its transitive crates) in the single binary, and a larger binary.
- Stdout belongs to JSON-RPC while `ac mcp` runs: any `print!` reached from a core path corrupts the channel. Cores must write only to their injected writers.
- Write tools are always on: an agent fed untrusted task or comment text can post, log time or move a task when its harness auto-approves tool calls. The owner accepted this; the annotations and harness permissions are the mitigation.
- Each session spawns its own process; the SQLite cache is the shared state.

**Follow-ups:**
- Each later capability (estimate and hours in `mine`, day/week grouping, comment attachments, moving a task between task lists) ships its MCP tool in the same issue.

## Verification

**Implementation impact:** `Cargo.toml`, `src/cli.rs`, `src/main.rs`, a new `src/mcp/` module, `src/store/mod.rs` (WAL), `docs/architecture.md`, the `active-collab` skill.

**Verification criteria:**
- An integration test drives `ac mcp` over stdio: `initialize`, `tools/list` returns every tool with its annotations, and a `tools/call` result is byte-identical to the matching `--json` command output against the same mocked API.
- Fitness function: a test runs each tool against a mocked API and asserts the process stdout holds only JSON-RPC frames.
- Fitness function: a test asserts the store opens in `WAL` mode.
- A request through any tool to a non-instance host carries no token (the existing token-host-isolation tests cover the shared seam).

# References

[1] [Model Context Protocol specification](https://modelcontextprotocol.io/specification)
[2] [rmcp, the official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
