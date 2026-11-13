---
type: Issue
title: Move a task between task lists from CLI, TUI and MCP
description: ac task set --list, a TUI picker on key l and the MCP ac_write set_task operation move a task to another task list of its project.
status: in-progress
labels: [cli, tui, agent, write, slice]
blocked_by: [0073]
timestamp: 2026-11-11T21:40:56Z
---

## 0077. Move a task between task lists from CLI, TUI and MCP

### Problem

The owner moves tasks between a project's task lists (the workflow columns) only in the web UI. Implements [ADR 0071](/adr/0071-task-list-joins-the-bounded-set-of-editable-task-fields.md).

### Scope

Included:

- A client read of `GET /api/v1/projects/{pid}/task-lists` (id, name, open only).
- `update_task` gains `task_list_id`.
- `ac task set <ref> --list <id|name>`, the name matched case-insensitively against the project's task lists; combinable with the existing fields.
- A TUI key, `l` (free today), in the task detail opens a searchable list picker like the assignee picker (issue 0070), marking the current list; confirming moves the task and refreshes from the server.
- The MCP `ac_write` `set_task` operation gains `list`.
- `--json` and the human output report the new list; the `active-collab` skill documents `--list`.
- The constitution already carries Amendment 2 for the widened field set; the change keeps it accurate.

Excluded: creating, renaming or reordering task lists; moving between projects; bulk moves.

### Decision

- "Status" in the owner's request means the task list, not open/completed, which already exists (`--status`, TUI `s`).
- An ambiguous or unknown name is a usage error that lists the candidates and sends no write.
- `--list` follows `--assignee`: an all-digits value is an id, any other value matches a list name exactly, case-insensitively. Unlike `--assignee`, the project's task lists are always read first, even for an id, so an id outside the task's project is refused before any write and the result can name the list.
- A failed task-lists read is a runtime failure (exit 1; 401 shows the re-authentication message), never "no list matches", and sends no write. With `--json` it prints the same `{"ok":false,...}` error object on stdout as a failed write, so a caller parses one failure shape per command.
- `update_task` gains an optional `task_list_id`; the PUT carries only the given fields, so a move sends `{"task_list_id":N}` alone. A completion change still goes first, then one PUT carries assignee, estimate and list together.
- The write result reports the move as `task_list_id` and `task_list` (the list name) in `--json` and as `list=<name>` in the human line.
- The MCP `set_task` operation takes `list` with the same meaning; its result is byte-identical to `ac task set --json`.
- Names compare with Unicode lowercase (`to_lowercase`), not full case folding or normalization: Portuguese list names fold correctly, and a name that fails to match (for example typed in another Unicode normal form) fails safe with exit 2 and the candidates, so the user can pass the id.
- The TUI picker reads the project's lists when `l` opens it (no read on every Detail load): it opens in a loading state, the core emits one `Cmd`, and the result returns as a `Msg` applied only to the same instance, project and task while the picker is still open. Lists keep server order (the board's column order), unlike the assignee picker's alphabetical order; the current list is marked and pre-selected.
- Confirming the current list closes the picker and sends no write. Confirming another list emits the shared `Cmd::SubmitTaskEdit` with only `task_list_id` set; success refreshes the Detail from the server as the other task edits do.
- The picker reuses the assignee picker's filtering and navigation instead of copying it; the assignee picker's behavior is unchanged. A failed lists read shows the failure in the picker. A 401 takes the existing re-authentication path and also ends the picker's loading state with the re-authentication text, so the picker never waits forever. A lists result reaching a picker that is no longer loading (already loaded, failed or reopened) is ignored. While the picker loads or after its read failed, its hint names only Esc.

- The live acceptance criterion verifies against the mock ActiveCollab of [issue 0084](/issues/0084-replay-the-live-acceptance-criteria-as-terminal-walkthroughs-against-a-mock-activecollab.md) (owner, 2026-11-12): the owner's `ac` is connected to the real instance, which no check may touch. Risk accepted: the mock returns the JSON shapes `tests/mcp_stdio.rs` mounts, so a real API that diverges from them still passes the walkthrough. Not taken: the mock plus live read-only reads.

### Acceptance

- AC1: `--list` resolves by id or unique name, and an unknown or ambiguous name exits 2 without a write. (`verify_by: test`)
- AC2: the PUT carries `task_list_id` to the instance host only. (`verify_by: test`)
- AC3: the TUI picker filters, confirms and refreshes; the transitions live in the pure update core. (`verify_by: test`)
- AC4: against the mock ActiveCollab, `ac task set --list` moves a task to another list: the write result names the new list and the mock records the PUT, replayed by a terminal walkthrough. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Plan

- S1: `ac task set --list` and the MCP `set_task` `list` move a task to another task list of its project | files: src/client.rs, src/commands/task.rs, src/commands/presenter.rs, src/agent_json.rs, src/cli.rs, src/main.rs, src/mcp/mod.rs, src/tui/mod.rs, locales/pt_BR.json, tests/unit/client.rs, tests/unit/commands.rs, tests/unit/agent_json.rs, tests/unit/cli.rs, tests/mcp_stdio.rs, .claude/skills/active-collab/SKILL.md | after: none
- S2: `l` in the TUI task detail opens a searchable task-list picker that marks the current list and moves the task on confirm | files: src/tui/model.rs, src/tui/events.rs, src/tui/mod.rs, src/tui/view.rs, src/tui/footer.rs, locales/pt_BR.json, tests/unit/model.rs, tests/unit/tui_render.rs, tests/unit/app.rs, docs/architecture.md, .claude/skills/active-collab/SKILL.md | after: S1 (reads the task lists and writes through S1's client calls)
