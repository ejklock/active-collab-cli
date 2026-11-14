---
type: ADR
title: ac mcp exposes downloads as a third tool with honest annotations
description: "ac mcp keeps stdio, rmcp and core adapters, and exposes three tools: ac_read (read-only), ac_write (destructive) and ac_download_attachments (not read-only, not destructive, idempotent), because MCP annotations are per tool."
owner: Evaldo Klock
status: Accepted
supersedes: 0070
timestamp: 2026-11-12T00:33:51Z
---

# 0072. ac mcp exposes downloads as a third tool with honest annotations

## Context

[ADR 0070](/adr/0070-ac-mcp-serves-mcp-over-stdio-as-an-adapter-over-the-command-cores.md) chose `ac mcp` over stdio with two tools grouped by operation type: `ac_read` (`readOnlyHint: true`) and `ac_write` (`destructiveHint: true`). Its context still holds: parallel agent sessions, token security first, a single agent contract second, distribution third; every command has a core that writes to injected writers.

Delivered in commit 514c5b5, `ac_read`'s `get` also takes `download_attachments` and `attachments_dir`, so a call to a tool announced as read-only writes files to a caller-chosen directory. MCP tool annotations (`readOnlyHint`, `destructiveHint`, `idempotentHint`) belong to a tool, never to one of its operations, so no operation inside `ac_read` can carry an honest hint for a download. Issue 0076 widens the download to comment attachments.

## Decision

We will keep `ac mcp` over stdio with `rmcp`, each operation a driving adapter over an existing command core that calls it with `json = true` and in-memory writers, returns its `out` buffer on exit 0 and its `err` buffer as an `isError` result otherwise; the `--json` contract of `src/agent_json.rs` stays the only agent contract.

The server exposes three tools, as the owner chose on 2026-11-11:

- `ac_read` (`get`, `current`, `mine`), `readOnlyHint: true`, which no longer downloads.
- `ac_write` (`comment`, `log_time`, `set_task`, `time_report`), `destructiveHint: true`.
- `ac_download_attachments`, `readOnlyHint: false`, `destructiveHint: false`, `idempotentHint: true`: downloads a task's attachments (by `ref`, optional `attachments_dir`, `instance`) and returns the `get --download-attachments --json` output.

All three are always exposed. Writes go through the host-gated write seam of [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md); the store runs in `journal_mode=WAL`.

Rejected alternatives:

- **Keep the download in `ac_read`** (the existing pattern): no new tool, but a harness that trusts `readOnlyHint` writes files without asking.
- **A `download_attachments` operation in `ac_read`**: same false hint, since hints are per tool.
- **A `download_attachments` operation in `ac_write`**: honest about the write, but `destructiveHint` makes a harness warn as for posting a comment.
- The alternatives ADR 0070 rejected (skill and shell only, Streamable HTTP, one tool per verb, a single tool, `--allow-writes`, a hand-written JSON-RPC loop, tools building their own JSON) stay rejected for its reasons.

## Consequences

**Easier / gained:**
- Every annotation tells the truth, so per-tool approval in each harness matches what the call does.
- Comment attachments (issue 0076) land in the download tool with no new hint question.

**Harder / accepted trade-offs:**
- A third tool definition in every session's context, against the owner's earlier goal of fewest tokens.
- `ac_read` `get` drops `download_attachments` and `attachments_dir`; nothing outside this repository called them (never pushed as of 2026-11-11).
- The trade-offs of ADR 0070 hold: the `rmcp` dependency tree, stdout reserved for JSON-RPC, writes always on, one process per session sharing the SQLite cache.

## Verification

**Implementation impact:** `src/mcp/mod.rs`, `tests/mcp_stdio.rs`, the `active-collab` skill, `README.md`, `docs/architecture.md`.

**Verification criteria:**
- `tools/list` returns exactly `ac_read`, `ac_write` and `ac_download_attachments` with the hints above.
- `ac_download_attachments` returns bytes identical to `ac get <ref> --download-attachments --attachments-dir <dir> --json` against the same mocked API.
- `ac_read` `get` with `download_attachments` is an `invalid_params` error.

# References

[1] [Model Context Protocol specification, tool annotations](https://modelcontextprotocol.io/specification)
