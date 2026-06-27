---
type: ADR
title: Render the task list as per-task cards with a relative, colored due date
description: Replace the single-column NOME task list with a bordered card per task showing the task number and name, the project, and a relative, color-coded due date. The due date is parsed from the existing list payload (no extra per-task fetch); the relative-to-today computation takes the current date as data so the pure core stays pure.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, task-list, cards]
timestamp: 2026-06-27T00:00:00Z
---

# 0026. Task list as per-task cards with a relative, colored due date

## Context

The Minhas Tarefas / browse task list renders each task as a row under a single `NOME`
column (`draw_tasks`, a wrapped-name table). The operator asked to drop that column and
show **a card per task with the due date**: *"pode remover essa coluna nome e criar card
pra cada task com informação de data due sei la."*

Two facts shape the decision:

- **The due date is not currently parsed from the list payload.** `MineTask::from_api`
  reads only `id`, `task_number`, `name`, `is_completed`, `is_trashed`, `project_id`; the
  full `Task` (with `due_on`) is fetched only on detail. ActiveCollab's `/users/{id}/tasks`
  task objects do carry `due_on` — it is simply unparsed. So the card can show the due date
  by **parsing the field already in the response**, with no extra per-task request.
- **A relative due date ("vence em 3 dias", colored when near/overdue) depends on "today",**
  which is non-deterministic. The pure TEA core must not read the clock; the current date is
  passed in as **data** by the shell and consumed only in the view.

Force: **scannability of a work list** — the operator wants to see *what is due and how
urgent*, not just a column of names.

## Decision

Render each task as a **bordered card** (reusing the `panel_box` visual vocabulary already
used by the detail panels and the Anexos card), removing the `NOME` column header. Delivered
as slices **D2a** (card shell) and **D2b** (project + due date).

### 1. Card content

Two content lines inside the card border:

- **Line 1:** `#<task_number>  <name>` (name wraps within the card width).
- **Line 2:** the **relative due date** (colored), the **project name**, and the task
  status, separated by `·` — e.g. `Vence em 3 dias · ProForce · aberto`.

### 2. Due date parsed from the list payload

`MineTask` (and the list `TaskRow`) gain a `due_on` field parsed from the existing
`/users/{id}/tasks` response (the `due_on` value is a unix timestamp or ISO string, already
normalized by `fmt_date`). **No extra per-task fetch.** When a task has no `due_on`, the card
shows a neutral "sem data" rather than a date.

### 3. Relative, color-coded due date computed in the view

The view receives the current date (`today`) from the shell and formats the due date
relative to it: **overdue → red**, **due within a small window → yellow**, otherwise the
default style. The relative phrasing ("hoje", "amanhã", "vence em N dias", "atrasada N dias")
lives in the i18n catalog. The pure `update` never computes this — it only holds the
`due_on` data; the view turns it into colored text.

### 4. Project name on the card

The card resolves `project_id` → project name from the existing project-name cache
(ADR 0014 SWR directory), so Minhas Tarefas (tasks spanning projects) shows each task's
project without a new request.

### 5. Multi-line click target

A card spans multiple buffer rows; its `ClickTarget` `y_start..y_end` covers the whole card,
so a click anywhere on the card selects/drills into that task. Selection highlight styles the
whole card.

## Alternatives considered

- **Keep a column layout, just add a due-date column.** Rejected: the operator asked for
  cards, and a colored relative due reads better as a card's second line than as a cramped
  column on a narrow terminal.
- **Fetch each task's detail to get the due date.** Rejected: `due_on` is already in the list
  payload; N detail fetches on every list paint would be slow and pointless. (If a future
  field genuinely needs the detail, that is a separate, explicit decision.)
- **Absolute date only (DD/MM/YYYY).** Rejected by the operator in favor of a relative,
  colored due ("vence em 3 dias", red when overdue) for at-a-glance urgency. The absolute
  date can still be the detail view's job.

## Consequences

**Positive:** the work list shows urgency at a glance; the card reuses the established box
vocabulary; due date and project come for free from data already fetched/cached.

**Accepted trade-offs:** cards are taller than single rows, so fewer tasks are visible at
once (the operator chose the boxed card knowingly). The relative/colored due adds a `today`
input threaded into the view and color logic. If `due_on` turns out absent from the list
payload for some instances, those cards degrade to "sem data" (surfaced, not silently wrong)
and a follow-up would decide whether to enrich from detail.

## Related

- ADR: [/adr/0014-browse-list-project-name-cache-swr.md](/adr/0014-browse-list-project-name-cache-swr.md) (project-name cache the card reuses)
- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) (unified list styling)
- ADR: [/adr/0024-asset-card-breathing-room.md](/adr/0024-asset-card-breathing-room.md) (card padding vocabulary)
- BDR: [/bdr/0020-task-list-cards.md](/bdr/0020-task-list-cards.md)
- Issue: [/issues/0025-d2-task-list-cards.md](/issues/0025-d2-task-list-cards.md)
- Architecture: [/architecture.md](/architecture.md)
