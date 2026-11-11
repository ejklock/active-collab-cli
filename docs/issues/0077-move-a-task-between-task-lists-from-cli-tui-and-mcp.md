---
type: Issue
title: Move a task between task lists from CLI, TUI and MCP
description: ac task set --list, a TUI picker on key l and the MCP ac_write set_task operation move a task to another task list of its project.
status: open
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

### Acceptance

- AC1: `--list` resolves by id or unique name, and an unknown or ambiguous name exits 2 without a write. (`verify_by: test`)
- AC2: the PUT carries `task_list_id` to the instance host only. (`verify_by: test`)
- AC3: the TUI picker filters, confirms and refreshes; the transitions live in the pure update core. (`verify_by: test`)
- AC4: the owner moves a real task between two lists on collab.base.digital and the web UI shows the new column. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
