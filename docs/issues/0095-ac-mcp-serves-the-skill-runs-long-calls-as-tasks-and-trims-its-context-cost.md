---
type: Issue
title: ac mcp serves the skill, runs long calls as tasks and trims its context cost
description: The ac MCP server serves the active-collab skill over the MCP skills extension, runs the attachment download and the hours export as tasks when the client declares the tasks extension, and spends fewer tokens on its tool list.
status: open
timestamp: 2026-11-25T12:00:00Z
---

## 0095. ac mcp serves the skill, runs long calls as tasks and trims its context cost

### Problem

`ac mcp` exposes three tools (`ac_read`, `ac_write`, `ac_download_attachments`) with about 1.2k tokens of schemas and no server instructions. An agent connected only to the server never sees the `active-collab` skill that `ac skill active-collab` prints. The attachment download and the hours export block the call for as long as they run, with no progress and no cancellation.

The owner asked (2026-11-25) that every MCP server of the team serve its skills over MCP and run long calls as tasks, following `qa-walkthrough-cli` (its ADR 0063 and `crates/cli/src/mcp/skills.rs`).

### Scope

Included: `src/mcp/` and `tests/mcp_stdio.rs`; the skills extension (SEP-2640) and the tasks extension (`io.modelcontextprotocol/tasks`, SEP-2663) through `rmcp` 3.9.0; tool and parameter descriptions.

Excluded: the CLI commands and their output; the `ac_read` operations' behavior; the pointer copies of the skill.

### Decision

- The skill is the same embedded text `ac skill active-collab` prints (`src/commands/skill.rs` `SKILLS`), served as the resource `skill://active-collab/SKILL.md` and as the one entry of `skills/list` and `skills/get`; the server instructions name that resource.
- A call becomes a task only when the client declares the tasks extension and the call is `ac_download_attachments` or `ac_write` with `operation` `time_report`; every other call stays synchronous.
- Shorter descriptions keep every fact a caller needs to choose a tool, an operation and its fields; a test bounds the `tools/list` size.

### Plan

- S1: tool and parameter descriptions trimmed; a test bounds the `tools/list` response size | files: src/mcp/mod.rs, tests/mcp_stdio.rs | after: none
- S2: the skill served over the skills extension and named in the server instructions | files: src/mcp/mod.rs, src/mcp/skills.rs, tests/mcp_stdio.rs, docs/adr/0076-ac-mcp-serves-its-skill-over-the-skills-extension-and-runs-long-calls-as-tasks.md, docs/adr/index.md, docs/architecture.md | after: S1
- S3: the download and the hours export run as tasks with progress and cancellation when the client declares the tasks extension | files: src/mcp/mod.rs, tests/mcp_stdio.rs, docs/adr/0076-…, .claude/skills/active-collab/SKILL.md | after: S2

### Acceptance

- AC1: `tools/list` is smaller than before and a test fails when it grows past the new bound; the existing description tests still pass. (`verify_by: test`)
- AC2: `initialize` declares resources and the skills extension, carries instructions naming `skill://active-collab/SKILL.md`; `resources/list`, `resources/read`, `skills/list` and `skills/get` return the embedded skill; an unknown URI is an `invalid_params` error. (`verify_by: test`)
- AC3: with the tasks extension declared, `ac_download_attachments` and `time_report` return a task that reaches `completed` through `tasks/get` with the same result as the synchronous call, and `tasks/cancel` ends it as cancelled; without the extension, both return the result in the call; every other call stays synchronous. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)
