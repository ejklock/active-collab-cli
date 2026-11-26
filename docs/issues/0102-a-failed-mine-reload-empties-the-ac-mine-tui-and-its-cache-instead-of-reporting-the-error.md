---
type: Issue
title: A failed mine reload empties the ac mine TUI and its cache instead of reporting the error
description: The mine loader turns a 401 or network failure into no tasks and still sends LoadedMineTasks, so a full failure blanks the screen, saves the empty list to the mine cache and never shows the re-authentication message.
status: open
timestamp: 2026-11-26T00:26:58Z
---

## 0102. A failed mine reload empties the ac mine TUI and its cache instead of reporting the error

### Problem

`collect_mine_task_lists` (`src/commands/mine.rs`) calls `fetch_open_tasks(...).await.ok()` per instance, so a 401 (`Err(Unauthorized)` from `src/client.rs`) or a network error drops that instance silently; `load_mine_tasks` (`src/tui/mod.rs`) then always sends `Msg::LoadedMineTasks`. When every instance fails, `handle_loaded_mine_tasks` replaces the shown rows with an empty list, the mine snapshot cache is overwritten with it, and no `AuthExpired` is sent. Every launch can hit it; `r` on the mine screen (issue 0101) adds another trigger. Found on 2026-11-25 in the review of issue 0101, traced in the code, not reproduced.

### Scope

Included: the mine TUI loader on a partial or full failure (launch and `r`).

Excluded: the CLI `ac mine`, which already reports 401.

### Acceptance

- AC1: when every instance fails, the shown rows and the mine cache stay as they were, and the TUI shows the error, or the re-authentication message on a 401. (`verify_by: test`)
