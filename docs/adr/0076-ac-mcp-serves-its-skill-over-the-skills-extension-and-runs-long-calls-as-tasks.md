---
type: ADR
title: ac mcp serves its skill over the skills extension and runs long calls as tasks
description: The ac MCP server serves the embedded active-collab skill as a resource and over the MCP skills extension, names it in its instructions, and runs the attachment download and the hours export as tasks when the client declares the tasks extension.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-25T12:00:00Z
---

# 0076. ac mcp serves its skill over the skills extension and runs long calls as tasks

## Context

`ac mcp` exposes three tools and no server instructions. An agent connected only to the server never sees the `active-collab` skill that `ac skill active-collab` prints. The team's other MCP servers (`qa-walkthrough-cli`, its ADR 0063) serve their skills over the MCP skills extension (SEP-2640) and run long calls as tasks (SEP-2663); the owner asked that every server do the same.

## Decision

- The skill is the same embedded text `ac skill active-collab` prints (`src/commands/skill.rs` `SKILLS`), served as the resource `skill://active-collab/SKILL.md` and as the one entry of `skills/list` and `skills/get`; the server instructions name that resource. A hand-written `ServerHandler` replaces the one `#[tool_router(server_handler)]` generated, so it can serve resources and skills; the three tools delegate to the same tool router and behave as before.
- Only that exact URI is served, from the embedded text; any other `resources/read` or `skills/get` URI is `invalid_params`, and no request argument reaches the filesystem. A `skills/get` without a text `uri` is `invalid_params`, not method-not-found.
- The catalog reads the skill's `name` and `description` from the two single-line frontmatter fields of the embedded file with a minimal line parse; no YAML dependency is added for a block the project owns and tests.
- A call becomes a task only when the client declares the tasks extension and the call is `ac_download_attachments` or `ac_write` with `operation` `time_report`; every other call stays synchronous.
- A task is created through rmcp's `TaskManager`, with no lifetime (rmcp's default drops a running task after five minutes), and the server answers `tasks/get` and `tasks/cancel` from it; an unknown task id is a protocol error. A request's progress token receives a notification when the work starts, one every 5 s while it runs (rising, always below the total of 100) and a final one at the total; the same goes to the token of a task.
- The command cores cannot be interrupted mid-run. On `tasks/cancel` the task ends `cancelled` at once; the core finishes on its blocking thread, its result is discarded and the files it already wrote stay. Each long tool's description says so in one short sentence, which raised the `tools/list` size bound by about 140 bytes.

## Alternatives

- Add `yaml-rust2` to parse the frontmatter, as `qa-walkthrough-cli` does: handles any YAML, but adds a dependency for two fixed plain fields.
- Embed a second copy of the skill for the server: simpler wiring, but two texts to keep equal.
- Leave the skill to the plugin stubs: an agent on the bare server still never sees it.

## Consequences

- The skill text reaches an agent that has only the MCP server, at the cost of a few hundred bytes of instructions in `initialize`.
- `AcServer` is no longer a unit struct: it holds the tool router, the skill catalog and the `TaskManager`.
- Fitness function: `tests/mcp_stdio.rs` asserts the initialize capabilities and instructions, the resource and skill listings and reads against the embedded `SKILL.md`, `invalid_params` for every other URI, and the task tests (long calls declared as tasks complete with the synchronous result, calls without the extension stay synchronous, cancel and unknown task ids, progress rising below the total).
