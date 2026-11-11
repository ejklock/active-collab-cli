---
type: Issue
title: "Group ac mine by due date: overdue, today, this week, later, no due date"
description: ac mine groups open tasks into due-date buckets with per-bucket counts and estimate and hour sums, in the table, the TUI, the JSON and MCP.
status: open
labels: [cli, tui, agent, slice]
blocked_by: [0074]
timestamp: 2026-11-11T21:40:56Z
---

## 0075. Group ac mine by due date: overdue, today, this week, later, no due date

### Problem

`ac mine` is one flat list; the owner cannot see at a glance what is late, what is due today and what is due this week, nor how many estimated hours each of those holds.

### Scope

Included:

- `ac mine` (table and TUI) groups tasks into buckets by `due_on`: Overdue, Today, This week (to Sunday), Later, No due date, in that order; tasks inside a bucket sort by due date, then project, then task number.
- Each bucket header shows its task count and the sums of estimate, week and total hours.
- `ac mine --json` adds `due_bucket` (`overdue|today|this_week|later|none`) to each task and a `buckets` array with each bucket's key, count and sums; existing fields are unchanged.
- The `list_my_tasks` MCP tool returns the same.
- Bucket labels go through the existing i18n (English source, pt_BR translation).

Excluded: grouping by start date or by an interval, a separate `ac week` command, filters.

### Decision

- Grouping by `due_on`, as the owner chose (over the start-to-due interval, a new command, or a filter only).
- The week runs Monday to Sunday, like `--period week` of the hours report.
- `due_on` is a UTC-midnight timestamp and is read in UTC (the hours report's rule); "today" is the local date.
- Empty buckets are hidden in the table and the TUI and kept with zero counts in the JSON.

### Acceptance

- AC1: the bucket of a task is right for due dates yesterday, today, Sunday of this week, next Monday and none, from an injected today, including a Sunday and a year boundary. (`verify_by: test`)
- AC2: bucket sums equal the sums of their tasks' estimate, week and total hours. (`verify_by: test`)
- AC3: the TUI task list renders bucket headers and keeps selection and scrolling on tasks only; the transitions live in the pure update core. (`verify_by: test`)
- AC4: the JSON adds only `due_bucket` and `buckets`. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
