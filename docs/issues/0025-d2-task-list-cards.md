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

**D2b — project name + relative colored due.** Parse `due_on` into `MineTask`/`TaskRow`;
resolve project name from cache into the row; add a pure `relative_due(due_on, today) ->
(text, style)` formatter (overdue red / near yellow / default; "hoje"/"amanhã"/"vence em N
dias"/"atrasada N dias"/"sem data"); render card line 2 `due · project · status`; thread
`today` from the shell into the view.
- Files: `src/models.rs`, `src/tui/model.rs` (TaskRow due_on + project_name), the list loader
  (`src/commands.rs`/controller), `src/render.rs` or `src/tui/screens/tasks.rs` (relative-due
  formatter + line 2), `src/tui/view.rs` (pass `today`), `locales/pt_BR.json`, tests.

### Acceptance (full D2)

- No `NOME` header; each task is a bordered card with `#number name` (BDR 0020 Sc.1).
- Line 2 shows a relative, color-coded due (red overdue, yellow near, default else;
  hoje/amanhã/vence em N/atrasada N), the project name, and status (Sc.2, Sc.4).
- A task with no `due_on` shows "sem data", default style (Sc.3).
- A click anywhere on a card drills into that task; the whole selected card is highlighted
  (Sc.5, Sc.6).
- No extra per-task fetch for due/project. Full suite green; clippy `-D warnings`, fmt,
  comment-policy clean; complexity within budget; render/format tests mutation-resistant.

### Notes

If `due_on` is absent from the list payload for an instance, cards degrade to "sem data"
(surfaced, not silently wrong); a follow-up would decide detail-enrichment vs. omission.
