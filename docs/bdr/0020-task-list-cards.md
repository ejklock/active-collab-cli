---
type: BDR
title: "The task list shows a bordered card per task with a relative, colored due date"
description: In the Minhas Tarefas / browse task list, each task renders as a bordered card showing #task_number + name on the first line and a relative color-coded due date, the project name, and the status on the second line. The NOME column header is removed. Overdue dates render red, near-due yellow. A click anywhere on the card drills into the task.
status: Accepted
superseded_by:
supersedes:
tags: [tui, ux, task-list, cards]
timestamp: 2026-06-27T00:00:00Z
---

# 0020. Task list as per-task cards

## Context

The task list was a single `NOME` column of wrapped names. The operator asked for a card per
task with the due date. Delivered by slices **D2a** (card shell, header removed) and **D2b**
(project + relative colored due) under [ADR 0026](/adr/0026-task-list-as-cards.md).

## Textual Description

In the **task list** (Minhas Tarefas and browse-into-project):

- The `NOME` column header is **removed**.
- Each task renders as a **bordered card** (the box vocabulary of the detail/Anexos panels).
- **Line 1:** `#<task_number>  <name>`, the name wrapping within the card width.
- **Line 2:** the **relative due date**, the **project name**, and the **status**, separated
  by `·`. The due date is **relative to today** — "hoje", "amanhã", "vence em N dias", or
  "atrasada N dias" — and **color-coded**: overdue → red, due within the near window →
  yellow, otherwise the default style. A task with no due date shows "sem data" (no color).
- The **selected card** is highlighted across its whole height.
- A **click anywhere on a card** selects/drills into that task (the click target spans the
  whole card, not just one line).
- The due date and project name come from data **already fetched/cached** — no extra request
  per task.

## Scenarios

**Scenario 1: cards replace the column** — Given the task list with tasks, When it renders,
Then there is no `NOME` header and each task is a bordered card with `#number name` on line 1.

**Scenario 2: relative colored due** — Given a task due in the future within the near window,
When its card renders, Then line 2 shows "vence em N dias" in the near-due (yellow) style;
given a task past due, Then it shows "atrasada N dias" in the overdue (red) style; given a
task due today, Then "hoje".

**Scenario 3: missing due date degrades** — Given a task with no `due_on`, When its card
renders, Then line 2 shows "sem data" with no due-date color (project/status still shown).

**Scenario 4: project name shown** — Given a task in project P, When its card renders, Then
line 2 includes P's name resolved from the project-name cache (no per-task fetch).

**Scenario 5: click drills in anywhere on the card** — Given a multi-line card, When the
operator clicks any row of it, Then that task is selected/opened (the card's whole
y-range is one click target).

**Scenario 6: selection highlights the whole card** — Given a selected task, When the list
renders, Then the entire card (all its rows) carries the selection style.

## Test Design

Rendering and click mapping are deterministic and asserted via the TestBackend buffer and
`ClickTarget` resolution; the relative/colored due is a pure function of `(due_on, today)`.
Each row names what it proves.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Cards, no NOME header | render (TestBackend) | 1 | no "NOME"; card border + `#n name` present | column→card |
| Relative due + color | unit | 2 | `(due_on, today)` → text + style (red/yellow/default) | relative colored due |
| Missing due → sem data | unit | 3 | no due_on → "sem data", default style | graceful degrade |
| Project name on card | render/unit | 4 | card line 2 contains the project name from cache | project resolution |
| Click anywhere drills in | unit | 5 | click on any card row resolves to that task index | multi-line click target |
| Selected card highlighted | render (TestBackend) | 6 | all rows of the selected card styled selected | whole-card highlight |

## Related

- ADR: [/adr/0026-task-list-as-cards.md](/adr/0026-task-list-as-cards.md)
- ADR: [/adr/0014-browse-list-project-name-cache-swr.md](/adr/0014-browse-list-project-name-cache-swr.md) (project-name cache)
- BDR: [/bdr/0004-browse-navigation-screen-stack.md](/bdr/0004-browse-navigation-screen-stack.md) (drill-in semantics)
- Issue: [/issues/0025-d2-task-list-cards.md](/issues/0025-d2-task-list-cards.md)
