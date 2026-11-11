---
type: Issue
title: Show estimate and logged hours (this week and total) per task in ac mine
description: ac mine table, TUI cards, JSON and the MCP ac_read mine operation show each task's estimate, the user's hours logged this week and the task's total logged hours.
status: open
labels: [cli, tui, agent, slice]
blocked_by: [0073]
timestamp: 2026-11-11T21:40:56Z
---

## 0074. Show estimate and logged hours (this week and total) per task in ac mine

### Problem

`ac mine` lists open tasks with their due date only; the estimate and the hours already logged are visible only in a task's detail, so planning the day or the week means opening each task.

A read-only probe against collab.base.digital on 2026-11-11 showed that each task from `/api/v1/users/{id}/tasks` carries `estimate` and `start_on`, but no tracked time; the task's total logged hours come only from `/api/v1/projects/{pid}/tasks/{tid}` (`tracked_time`), and the user's records of a period come from `/api/v1/users/{id}/time-records/filtered-by-date`, each with `parent_type: "Task"` and `parent_id`.

### Scope

Included:

- `MineTask` and `MineTableRow` gain `estimate`, `logged_week` (the user's hours on the task this week, Monday to Sunday) and `logged_total` (all hours on the task).
- `logged_week` comes from one `filtered-by-date` call per instance, summed by `parent_id`; `logged_total` from one task call per listed task, run concurrently.
- The `ac mine` table and the TUI task cards show `est`, `week` and `total` hours per task.
- `ac mine --json` adds `due_on`, `estimate_hours`, `logged_week_hours` and `logged_total_hours` to each task; existing fields are unchanged.
- The MCP `ac_read` `mine` operation returns the new fields (through the shared JSON).
- The `active-collab` skill documents the new fields.

Excluded: totals per group ([issue 0075](/issues/0075-group-ac-mine-by-due-date-overdue-today-this-week-later-no-due-date.md)), other users' hours, editing from the list.

### Decision

- Estimate, week and total, as the owner chose (over estimate only or estimate plus total).
- The list keeps its stale-while-revalidate first paint: cached rows show at once, and the hour columns fill in when the revalidation finishes.
- A failed task call leaves that task's `logged_total` empty instead of failing the list.

### Acceptance

- AC1: `logged_week` sums only the user's records of the current Monday–Sunday whose parent is the task. (`verify_by: test`)
- AC2: the CLI table, the TUI cards and the JSON show the three values, with an empty value when unknown. (`verify_by: test`)
- AC3: the JSON keeps every existing field and adds only the four new ones. (`verify_by: test`)
- AC4: a failing task call does not fail the list. (`verify_by: test`)
- AC5: `ac mine` against collab.base.digital matches the estimates and hours the web UI shows for three tasks. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
