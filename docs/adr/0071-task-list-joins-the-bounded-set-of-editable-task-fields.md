---
type: ADR
title: Task list joins the bounded set of editable task fields
description: Moving a task between its project's task lists (task_list_id) joins status, assignee and estimate as an editable task field, through the existing update_task write seam.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-11T21:40:49Z
---

# 0071. Task list joins the bounded set of editable task fields

## Context

[ADR 0069](/adr/0069-active-collab-cli-becomes-a-limited-write-client.md) and constitution Amendment 1 made active-collab-cli a limited write client with a bounded set of editable task fields: status, assignee and time estimate. "Status" there means open or completed. ActiveCollab also places every task in one of its project's task lists, the columns a team uses as workflow stages ("In progress", "Review", …). The owner wants to move a task between those columns from the CLI, the TUI and MCP; Amendment 1 says any field outside the bounded set awaits its own recorded decision.

A read-only probe against collab.base.digital on 2026-11-11 showed that each task from `/api/v1/users/{id}/tasks` and `/api/v1/projects/{pid}/tasks/{tid}` carries `task_list_id`, and that `GET /api/v1/projects/{pid}/task-lists` returns the project's lists with `id` and `name`.

## Decision

We will add the task list (`task_list_id`) to the bounded set of editable task fields. A move is one `PUT /api/v1/projects/{pid}/tasks/{tid}` carrying `task_list_id`, through the existing `update_task` write seam, with the typed outcome and server-truth refresh of ADR 0069. The target list is named by id or by a name matched against the project's task lists, like `--assignee`.

Rejected alternatives:

- **Leave columns out of scope**: the owner asked for it, and the read side already shows the data.
- **A general field editor** (any task field): that is the bulk/full-parity scope Amendment 1 keeps out.

## Consequences

**Easier / gained:**
- Moving a task through the workflow without the web UI, from the CLI, the TUI and MCP.

**Harder / accepted trade-offs:**
- One more write path to keep host-gated and typed.
- The `task_list_id` PUT payload is inferred from the API's task shape; the first implementation proves it with a live move by the owner.

**Follow-ups:**
- Constitution Amendment 2 records the widened field set.

## Verification

**Implementation impact:** `src/client.rs` (`update_task`, a task-lists read), `src/commands/task.rs`, `src/cli.rs`, `src/tui/`, `src/mcp/`.

**Verification criteria:**
- A test asserts the PUT body for a move carries only `task_list_id` and goes to the instance host with the token.
- A test asserts an unknown or ambiguous list name is a usage error and sends no write.
