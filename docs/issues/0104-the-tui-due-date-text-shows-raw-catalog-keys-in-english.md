---
type: Issue
title: The TUI due-date text shows raw catalog keys in English
description: The relative due-date text uses snake_case catalog keys (due_overdue, due_days, due_none, due_in, due_today, due_tomorrow, due_day) as msgids; only pt_BR maps them, so in English the cards read 'due_overdue 247 due_days'.
status: open
timestamp: 2026-11-26T01:11:34Z
---

## 0104. The TUI due-date text shows raw catalog keys in English

### Problem

`relative_due` in `src/tui/model.rs` builds the due-date text of every task card from `t("due_overdue")`, `t("due_days")`, `t("due_day")`, `t("due_in")`, `t("due_none")`, `t("due_today")` and `t("due_tomorrow")`. `locales/pt_BR.json` maps these keys; English has no catalog and `t()` returns the msgid, so in English the list and the kanban cards read `due_overdue 247 due_days`, `due_in 16 due_days`, `due_none`. Seen by the owner on 2026-11-25 in the vertical kanban of issue 0100; the same code is on `main`, so the due-date list has it too.

### Scope

Included: the relative due-date text on TUI task cards in English and pt_BR.

Excluded: other catalog keys.

### Acceptance

- AC1: in English a card reads plain English (for example "overdue 247 days", "due in 16 days", "no due date", "due today", "due tomorrow"), and in pt_BR the current Portuguese text is unchanged. (`verify_by: test`)
