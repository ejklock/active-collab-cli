---
type: Issue
title: r on a browse task list in the TUI stays on Loading forever
description: On a browse task list r sets loading and emits LoadTasksByProject, whose reply is applied only when the top screen is Projects, so the task list never leaves Loading and later r presses are dropped.
status: open
timestamp: 2026-11-26T00:26:58Z
---

## 0103. r on a browse task list in the TUI stays on Loading forever

### Problem

`handle_refresh` (`src/tui/model.rs`) on a browse `Screen::Tasks` (`due_today: None`) sets `loading = true` and emits `Cmd::LoadTasksByProject`; `handle_loaded_tasks` applies the reply only when the top screen is `Projects`, so the task list stays on "Loading…" and the single-flight guard drops every later `r`. Same defect as issue 0101, on the browse screen. Found on 2026-11-25 in the review of issue 0101, traced in the code, not run.

### Scope

Included: `r` on a browse task list.

Excluded: the mine screen (issue 0101).

### Acceptance

- AC1: `r` on a browse task list reloads its project's tasks and leaves the loading state when they arrive. (`verify_by: test`)
