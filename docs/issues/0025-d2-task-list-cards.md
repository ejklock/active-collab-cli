---
type: Issue
title: "D2 — task list as per-task cards with a relative, colored due date (D2a card shell, D2b project + due)"
description: Replace the single NOME-column task list with a bordered card per task. D2a renders the card shell (#number + name, header removed, whole-card click target/selection). D2b parses due_on from the list payload, resolves the project name from cache, and shows a relative color-coded due date on line 2.
status: open
labels: [tui, ux, task-list, cards]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## D2 — task list as per-task cards

Implements [ADR 0026](/adr/0026-task-list-as-cards.md), observable behavior pinned by
[BDR 0020](/bdr/0020-task-list-cards.md). Two vertical slices.

### Problem

The task list is a single `NOME` column of names. The operator wants a card per task showing
the due date (relative, colored) so urgency is visible at a glance.

### Decision

Render each task as a bordered card (panel_box vocabulary). Parse `due_on` from the existing
`/users/{id}/tasks` payload (no extra fetch); resolve project name from the SWR cache; format
the due date relative to a `today` the shell passes into the view, color-coded (overdue red,
near yellow).

### Slices

**D2a — card shell.** Render each task as a bordered card with `#<task_number> <name>`;
remove the `NOME` header; make the `ClickTarget` span the whole card so a click anywhere
drills in; highlight the whole selected card.
- Files: `src/tui/screens/tasks.rs`, `src/tui/model.rs` (ClickTarget y-range per card),
  `locales/pt_BR.json` (remove NOME header string), `tests/unit/tui_render.rs`.

**D2b — `due_on` data + the pure `relative_due` formatter.** Thread `due_on` through
`MineTask → MineTableRow → TaskRow` (serde default keeps SWR snapshots compatible) and add a
pure `relative_due(due_on, today) -> (text, DueStyle)` formatter (overdue red / near yellow /
default; "hoje"/"amanhã"/"vence em N dias"/"atrasada N dias"/"sem data"). Data + logic only,
unit-tested; no card render yet.
- Files: `src/models.rs`, `src/render.rs` (`MineTableRow.due_on`), `src/commands.rs`
  (`mine_task_to_row`), `src/tui/model.rs` (`TaskRow.due_on` + `relative_due` + `DueStyle`),
  `locales/pt_BR.json` (due labels), tests.

**D2c — render the relative colored due on card line 2.** Consume `relative_due` in the card
render: line 2 shows the colored due text (overdue red, near yellow, else default; "sem data"
when absent). Thread `today` from the shell into the view (the view reads the clock; the card
takes `today` as data so tests stay deterministic).
- Files: `src/tui/screens/tasks.rs` (line 2 + colors), `src/tui/view.rs` (pass `today`),
  `src/tui/model.rs` (drop the D2b `allow(dead_code)` deferrals), `src/tui/theme.rs`
  (`DueStyle → Style`), `tests/unit/tui_render.rs`.

**D2d — project name on line 2.** Resolve `project_id → project name` from the
project-directory cache (ADR 0014) for the mine list and append ` · <project>` to line 2;
omit the segment when the cache has no name (no extra fetch).
- Files: the mine list loader / model name map, `src/tui/screens/tasks.rs`,
  `locales/pt_BR.json` if needed, tests.

**Note — no status segment.** The list is pre-filtered to open tasks (`fetch_open_tasks`),
so a per-card status is uniformly "aberto" and is omitted; line 2 is `<due> · <project>`
(see the ADR 0026 / BDR 0020 amendments).

### Acceptance (full D2)

- No `NOME` header; each task is a bordered card with `#number name` (BDR 0020 Sc.1).
- Line 2 shows a relative, color-coded due (red overdue, yellow near, default else;
  hoje/amanhã/vence em N/atrasada N) and the project name (Sc.2, Sc.4). No status segment —
  the list is pre-filtered to open tasks (see the BDR 0020 amendment).
- A task with no `due_on` shows "sem data", default style (Sc.3).
- A click anywhere on a card drills into that task; the whole selected card is highlighted
  (Sc.5, Sc.6).
- No extra per-task fetch for due/project. Full suite green; clippy `-D warnings`, fmt,
  comment-policy clean; complexity within budget; render/format tests mutation-resistant.

### Notes

If `due_on` is absent from the list payload for an instance, cards degrade to "sem data"
(surfaced, not silently wrong); a follow-up would decide detail-enrichment vs. omission.
