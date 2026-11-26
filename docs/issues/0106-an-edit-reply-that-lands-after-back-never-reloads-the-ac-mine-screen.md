---
type: Issue
title: An edit reply that lands after Back never reloads the ac mine screen
description: If the user cancels a submitting picker and presses Back before TaskEditOk arrives, no stale mark is set because the detail is no longer on top, so the moved card stays in its old column until r.
status: open
timestamp: 2026-11-26T04:40:58Z
---

## 0106. An edit reply that lands after Back never reloads the ac mine screen

### Problem

The mine TUI reloads on Back only when `TaskEditOk` arrives while the task detail is on top of the mine screen (`mine_screen_below_detail` in `src/tui/model.rs`). The task-list and assignee pickers can be closed while their edit is still submitting (`TaskListPickerCancel` / `AssigneePickerCancel` call `close_detail_overlay` without a status check), so Esc, Esc pops the detail before the reply; the reply then sets nothing and the edited card stays in its old column until `r`. Found on 2026-11-26 in the review of issue 0100 slice S5 with a probe test, not seen by a user; the condition existed before that slice.

### Scope

Included: an edit reply that arrives after the detail was left.

Excluded: the in-flight reload case fixed in issue 0100.

### Acceptance

- AC1: Select, submit an edit, Back before the reply, then `TaskEditOk` → the mine screen reloads (`revalidating` and `Cmd::LoadMineTasks`). (`verify_by: test`)
