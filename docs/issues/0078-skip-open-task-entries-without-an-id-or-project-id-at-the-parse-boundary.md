---
type: Issue
title: Skip open-task entries without an id or project id at the parse boundary
description: ac mine and ac browse skip an API task entry without an integer id or project_id instead of showing it under project or task 0.
status: open
labels: [cli, tui, bug]
timestamp: 2026-11-12T00:21:45Z
---

## 0078. Skip open-task entries without an id or project id at the parse boundary

### Problem

`MineTask::from_api` (src/models.rs:103) reads `id` with `unwrap_or(0)` and keeps `project_id` as `Option<i64>`. Downstream, `mine_task_to_row` (src/commands/mine.rs:170) and `build_groups` (src/controller.rs:175) turn a missing `project_id` into `0`, so a task entry without one prints a `ref` such as `0/75159` that `ac get` cannot fetch, and `ac browse` groups it under a project `0`. Found in the review of commit 2db5bca; it predates that change.

### Scope

Included:

- The API task entry is parsed once at the boundary: an entry without an integer `id` or `project_id` is not a task and is skipped there, so `MineTask` carries both as plain integers.
- `ac mine` (table, `--json`, TUI, MCP `ac_read` `mine`) and `ac browse` stop showing such entries.

Excluded: the task detail parse (`Task`), other `unwrap_or(0)` uses outside the open-task list.

### Decision

- Skip the entry rather than carry an `Option` into every row: a task without a project has no fetchable `ref`, and ActiveCollab always returns both fields for a real task (inference from the API shape; no counter-example observed).

### Acceptance

- AC1: an open-task entry without `id`, or without `project_id`, or with a non-integer one, is absent from `ac mine` and `ac browse`; the other entries of the same response are listed. (`verify_by: test`)
- AC2: no row or group carries project `0` or task `0` from a missing field. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
