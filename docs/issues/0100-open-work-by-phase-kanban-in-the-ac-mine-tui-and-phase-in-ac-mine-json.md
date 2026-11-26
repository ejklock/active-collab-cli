---
type: Issue
title: "Open work by phase: kanban in the ac mine TUI and phase in ac mine --json"
description: A TUI kanban view of the logged-in user's open tasks, grouped into board phases derived from STATUS labels and task list names, so open work across projects reads at a glance.
status: in-progress
blocked_by: []
timestamp: 2026-11-25T20:45:57Z
---

## 0100. Open work by phase: kanban in the ac mine TUI and phase in ac mine --json

### Problem

`ac mine` groups open tasks by due date, and the ActiveCollab board shows one project at a time. Nobody sees at a glance, across projects, which of their tasks are to do, in progress, in review or ready to release. Task list names differ per project (`Doing`, `⚙️ Doing Geral`, `To Test`, `🔍In review`, `✅ Ready to go`) and about half the open tasks carry no `STATUS:` label (7 of 14 on 2026-11-25, from `ac mine --json`).

### Scope

Included: a kanban view inside the `ac mine` TUI over the user's own open tasks, the rule that assigns each task to a phase, and the phase vocabulary.

Excluded: a new CLI verb or flag, other users' tasks, a project-wide board.

### Decision



- The phase comes from a hybrid rule: a `STATUS:` label decides when present; otherwise a keyword in the task list name; otherwise the task falls into TO DO. Not taken: task list only, `STATUS:` label only.
- The kanban screen lives in the TUI, and the same phase reaches agents through `ac mine --json` and `ac_read mine` (owner, 2026-11-25, widening the earlier "TUI only"). Not taken: `ac mine --by phase` as a separate view, a new verb (`ac work`/`ac board`).
- `ac mine --json` always carries `phase` (stable English key: `to_do`, `in_progress`, `in_review`, `ready_for_release`) and `flagged` on every task, and a top-level `phases` array of `{key, count}` beside `buckets`; additive. Not taken: only with a `--by phase` flag. Known cost: `phase` follows the local `ac setup board` mapping, so two machines with different mappings can report different phases for one task.
- `ac mine --phase <key>` (and `phase` on `ac_read mine`) keeps only tasks in that phase and combines with `--label` and `--task-list`. Not taken: leaving the filtering to the agent.
- It shows only the user's own open tasks, the data `ac mine` already loads, with no extra request. Not taken: whole project, both.
- The TUI layout is a kanban, columns side by side. Not taken: stacked sections, project then phase.
- Four columns after the Jira status categories: TO DO, IN PROGRESS, IN REVIEW, READY FOR RELEASE ("Go live" is an internal term). Not taken: Jira's three categories (go live would mix into IN PROGRESS), three without review.
- A task whose label and list match no column, including lists that are not phases (Acompanhamento, Sync, Boilerplate) and Backlog lists, goes to TO DO. Not taken: a fifth BACKLOG column, hiding them behind a count, an OTHER column.
- `STATUS:` labels that mean waiting or blocked (`AGUARDANDO …`, `BLOQUEADO`, `STAND-BY`) flag the card (`flagged` in JSON), as Jira flags an impediment. A waiting label decides the phase only when it also holds a phase keyword (`AGUARDANDO VALIDAÇÃO` → IN REVIEW, `AGUARDANDO DEPLOY` → READY FOR RELEASE); otherwise the card keeps the column of its list.
- The keyword mapping from list names and labels to columns is editable through `ac setup board` (show, `set <column> --lists … --labels …`, `reset`), in the show-or-set shape of `ac setup language` and `theme`. The settings hold only the keyword lists the user set, laid field by field over the built-in keywords of the running version, so a release that improves a built-in reaches every field the user did not set; `reset` clears every override. Matching trims, ignores case and matches by keyword. Not taken: fixed in code, a TOML file opened in `$EDITOR`.
- Column titles follow the i18n language like the rest of the TUI. Not taken: always English.
- `v` in the `ac mine` TUI toggles between the due-date view and the kanban, and the choice is remembered for the next launch. Not taken: always open by due date, kanban as default.

### Plan


Checked against the code on 2026-11-25, after issue 0098 merged (PR #59, released in v0.12.7). The slices run one after another in the main checkout: they share `src/cli.rs`, `src/main.rs` and the TUI model. The pure rule ships with its first caller (the JSON), because a module with no caller fails `clippy -D warnings` as dead code.

- S1: the phase rule in a new `src/board.rs` (`Phase`, `BoardMapping` with built-in keyword defaults, `place(labels, task_list, mapping) -> Placement {phase, flagged}`), wired into `ac mine --json` and `ac_read mine` (`phase`, `flagged`, `phases`), the `--phase` flag and the `ac_read mine` `phase` argument, with the built-in mapping; skill and README (AC1–AC4) | files: `src/board.rs`, `src/main.rs`, `src/agent_json.rs`, `src/commands/mine.rs`, `src/render/mod.rs`, `src/cli.rs`, `src/mcp/mod.rs`, `.claude/skills/active-collab/SKILL.md`, `README.md` | after: none
- S2: `ac setup board` (show, `set <column> --lists … --labels …`, `reset`), stored as one JSON value under a `settings` key through `SettingsRepository`, in the shape of `setup_theme` (src/commands/setup.rs:125); `ac mine --json`, `ac_read mine` and the TUI read it, defaults when unset (AC5) | files: `src/board.rs`, `src/commands/setup.rs`, `src/cli.rs`, `src/main.rs`, `src/mcp/mod.rs`, `locales/pt_BR.json`, skill, README | after: S1
- S3: the kanban in the `ac mine` TUI: `TaskRow` (src/tui/model.rs:66) and the mine snapshot writer (src/tui/mod.rs:~480, today writing empty labels and list) carry labels and the list name so a seeded launch shows the right columns; a kanban view of `Screen::Tasks` with column focus (←/→) and card selection (↑/↓); `v` toggles and the choice persists in the settings; `Enter` opens the selected card's task; column titles through `t()` (AC6) | files: `src/tui/model.rs`, `src/tui/mod.rs`, `src/tui/events.rs`, `src/tui/view.rs`, a new kanban screen under `src/tui/screens/`, `locales/pt_BR.json` | after: S2
- S4: below about 100 columns the focused column with count tabs; `l` opens the task list picker from a card (AC7) | files: the kanban screen, `src/tui/model.rs`, `src/tui/events.rs` | after: S3

Cheap choices already taken: cards in each column keep the due-date view's order; the card's colored bar is its due bucket; no drag between columns (a column does not say which list of the project to move to), `l` moves the list instead; `r` reuses the mine revalidation; no extra request; when labels point to several phases the furthest phase wins; an open task in a `Done` list counts as READY FOR RELEASE; a label that matches only flag keywords does not decide the phase.

### Acceptance

- AC1: each task falls into the phase its `STATUS:` label gives, else its list keyword, else TO DO; waiting and blocked labels set the flag. Cases from the real lists and labels of 2026-11-25 (`Doing`, `⚙️ Doing Geral`, `To Test`, `🔍In review`, `✅ Ready to go`, `Go live`, `Acompanhamento`; `STATUS:GO LIVE`, `STATUS: GO LIVE`, `STATUS:GOLI`, `STATUS:AGUARDANDO CLIENTE`, `STATUS:DESENVOLVIMENTO`). (`verify_by: test`)
- AC2: `ac mine --json` carries `phase` and `flagged` on every task and a `phases` array of `{key, count}`; every key it printed before keeps its name, type and value. (`verify_by: test`)
- AC3: `ac mine --phase <key> --json` keeps only that phase and combines with `--label` and `--task-list`; an unknown key exits 2 listing the valid keys. (`verify_by: test`)
- AC4: the MCP `ac_read` `mine` operation returns the same `phase`, `flagged` and `phases` as `ac mine --json`, takes a `phase` argument that filters the same way (an unknown key is an invalid-params error), lists it with a description in the tool schema, and the served skill documents the fields, the argument and the key vocabulary. (`verify_by: test`)
- AC5: `ac setup board` shows the mapping, `set` changes one column's list and label keywords, `reset` restores the defaults; the next `ac mine --json`, `ac_read mine` and TUI launch use the new mapping. (`verify_by: test`)
- AC6: in the `ac mine` TUI, `v` toggles between the due-date view and the four-column kanban with titles in the i18n language, and the next launch opens in the last view. (`verify_by: test`)
- AC7: below about 100 terminal columns the kanban shows the focused column with tabs carrying each column's count; `Enter` opens the task detail and `l` opens the task list picker from a card. (`verify_by: test`)
