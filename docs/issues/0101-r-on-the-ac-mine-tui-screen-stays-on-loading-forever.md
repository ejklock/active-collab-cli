---
type: Issue
title: r on the ac mine TUI screen stays on Loading forever
description: handle_refresh on a mine Tasks screen emits LoadTasksByProject, whose reply only a Projects screen applies, so the mine list (and now the kanban) stays on Loading and a second r is dropped.
status: open
timestamp: 2026-11-25T23:29:17Z
---

## 0101. r on the ac mine TUI screen stays on Loading forever

### Problem

Pressing `r` on the `ac mine` TUI screen leaves "Loading…" on screen for good. `handle_refresh` (`src/tui/model.rs`) sets `loading = true` on any Tasks screen and emits `Cmd::LoadTasksByProject`; its reply, `Msg::LoadedTasksByProject`, is applied by `handle_loaded_tasks` only to a Projects screen, so the mine screen never leaves the loading state, and the single-flight guard drops a second `r`. Found on 2026-11-25 in the review of issue 0100 (kanban), reproduced there with a test on a temporary copy; the same code is on `main`. The kanban footer and README advertise `r refresh`, so the kanban inherits it.

### Scope

Included: `r` on the mine screen (list and kanban) reloads the mine tasks.

Excluded: refresh on the browse screens.

### Acceptance

- AC1: on the mine screen, `r` emits the mine reload, the rows it returns replace the shown ones, and the screen leaves the loading state; the same in the kanban view. (`verify_by: test`)
