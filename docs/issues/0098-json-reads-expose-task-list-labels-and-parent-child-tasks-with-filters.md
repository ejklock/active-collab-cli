---
type: Issue
title: JSON reads expose task list, labels and parent/child tasks, with filters
description: ac get and ac mine --json gain task_list, labels and task dependencies, ac mine filters by label and task list, and ac_read returns the same, so an agent can list the tasks a deploy ships.
status: open
tracker: https://github.com/ejklock/active-collab-cli/issues/58
timestamp: 2026-11-25T19:00:00Z
---

## 0098. JSON reads expose task list, labels and parent/child tasks, with filters

Tracks GitHub issue [#58](https://github.com/ejklock/active-collab-cli/issues/58).

### Problem

An agent preparing a release cannot answer "which tasks are approved for this deploy?" from `ac --json` alone. The team encodes that answer in ActiveCollab data the CLI does not return:

- A deploy task (for example "Deploy 24.11") lists the tasks it ships under **Parent Tasks**, the ActiveCollab task dependencies.
- Each task carries a **label** such as `STATUS:GO LIVE`, `STATUS:DESENVOLVIMENTO` or `ROTINA MENSAL`.
- The board column is the **task list** (for example "Go live").

`ac get --json` and `ac mine --json` return no task list, no labels and no dependencies. `task_list_id`/`task_list` appear only in the task-edit result (`src/agent_json.rs`).

### Scope

Included:

- `task_list_id`, `task_list` and `labels` on every task of `ac get --json` and `ac mine --json`.
- `parents` and `children` on `ac get --json`, each with `ref`, `name`, `status`, `task_list` and `labels`, from the ActiveCollab task dependencies; a flag to skip the extra request.
- `ac mine --label <name>` and `ac mine --task-list <name|id>`.
- The same fields and filters through the MCP `ac_read` tool (`get`, `mine`), documented in the skill's JSON schemas.

Excluded: subtasks (open question in #58), a new project-level task listing command, TUI changes.

### Acceptance

- AC1: `ac get 817/81802 --json` lists the parent tasks of the deploy task, each with its labels and task list. (`verify_by: live`)
- AC2: `ac get <ref> --json` and `ac mine --json` include `task_list` and `labels` for every task. (`verify_by: test`)
- AC3: `ac mine --label "STATUS:GO LIVE" --json` returns only tasks carrying that label. (`verify_by: test`)
- AC4: filtering by task list works by name and by id. (`verify_by: test`)
- AC5: the MCP `ac_read` tool returns the same fields, and the skill documents them. (`verify_by: test`)
- AC6: existing JSON consumers keep working: the new fields are additive. (`verify_by: test`)

### Decision

- `labels` is an array of `{id, name}` objects on every task, from the payload's label objects. Not taken: names only (turning them into objects later breaks consumers), `{id, name, color}` (color is noise for agents).
- `ac get --json` fetches `parents` and `children` by default from `GET /api/v1/dependencies/tasks/{id}`; `--no-dependencies` (and `no_dependencies` on `ac_read get`) skips the request. Not taken: opt-in `--dependencies`.
- The task list names of parents and children come from `GET /projects/{id}/task-lists`, one request per distinct project.
- `parse_task_payload` keeps the top-level `task_list` object; the task cache format version goes from 1 to 2 so older entries without it are refetched.
- `ac mine` reads `labels` and `task_list_id` from each task and the list name from the `related.TaskList` side-load, an object keyed by id; no extra request.
- `--label` matches a label name exactly, ignoring case, is repeatable, and keeps tasks that carry every given label. `--task-list` takes an id when all digits, else a name matched like `ac task set --list`. Both filter the table and the JSON; the TUI is out of scope.
- `--task-list` compares names without trimming, unlike `ac task set --list`, which trims; noted to the owner, kept as built.
- A filter flag in a TTY prints the filtered table instead of launching the TUI; a filter that matches nothing prints `count` 0 and exits 0, with no "unknown list" error, because list names repeat across projects.
- `parents` and `children` are `[]` when the task has none and `null` when they were skipped (`--no-dependencies`) or their request failed, so a skipped lookup never reads as "no dependencies". Dependencies are not cached and appear only in `--json`; trashed dependencies are left out.
- Subtasks are out of scope.

### Plan

- S1: `ac get --json` and `ac mine --json` carry `task_list_id`, `task_list` and `labels` (AC2, AC6) | files: `src/controller.rs`, `src/store/cache.rs`, `src/agent_json.rs`, `src/models.rs`, `src/client.rs`, `src/render/mod.rs`, `src/commands/mine.rs`, skill and `docs/adr/0011-agent-json-output-contract.md` | after: none
- S2: `ac mine --label` and `--task-list` filter the table and the JSON, and `ac_read mine` takes the same filters (AC3, AC4, AC5 for mine) | files: `src/cli.rs`, `src/main.rs`, `src/commands/mine.rs`, `src/mcp/mod.rs`, skill | after: S1
- S3: `ac get --json` carries `parents` and `children`; `--no-dependencies` and `ac_read get` `no_dependencies` skip them (AC1, AC5 for get) | files: `src/client.rs`, `src/commands/task.rs`, `src/agent_json.rs`, `src/cli.rs`, `src/main.rs`, `src/mcp/mod.rs`, skill | after: S2
- The slices run one after another in the main checkout: they share `src/agent_json.rs`, `src/cli.rs` and `src/mcp/mod.rs`.

### Outcome

Implemented on branch `feat/ek-0098-json-task-list-labels-dependencies` (uncommitted as of 2026-11-25) in three reviewed slices:

- S1: `task_list_id`, `task_list` and `labels` on `ac get --json` and `ac mine --json`; task cache format 2. Approved in round 1.
- S2: `ac mine --label`/`--task-list` and `ac_read mine` `labels`/`task_list`. Approved after a repair round that added the end-to-end wiring test in `tests/mcp_stdio.rs`.
- S3: `parents`/`children` on `ac get`/`current --json`, `--no-dependencies` and `ac_read get` `no_dependencies`. Approved, with a fix round that tested the `current` wiring.

Proof: AC1 live on 2026-11-25 with the built binary: `ac get 817/81802 --json` printed 7 parents with their lists and labels, `--no-dependencies` printed null, `ac mine --label "STATUS:GO LIVE" --json` and `ac mine --task-list "Go live" --json` printed 6 tasks each. AC2–AC6 by tests: `cargo test -- --test-threads=1` green (1940 unit tests and every integration suite), clippy `--all-targets -D warnings`, `fmt --check`, `comment_policy` and `se-gates check` green.

Left open: ADR 0011 and BDR 0010 do not list the new fields (owner to decide); `--task-list` does not trim names unlike `ac task set --list` (owner to decide); the `current` dependencies test in `tests/mcp_stdio.rs` carries no `#[cfg(unix)]` (CI runs tests on Linux only); the stale cache after `ac task set` is issue 0099.
