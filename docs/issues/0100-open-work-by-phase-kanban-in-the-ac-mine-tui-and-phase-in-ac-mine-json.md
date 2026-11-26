---
type: Issue
title: "Open work by phase: kanban in the ac mine TUI and phase in ac mine --json"
description: A TUI kanban view of the logged-in user's open tasks, grouped into board phases derived from STATUS labels and task list names, so open work across projects reads at a glance.
status: closed
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

- `v` in the `ac mine` TUI opens a view modal with three choices: due-date list, horizontal kanban (phases side by side as columns, cards stacked in each) and vertical kanban (one vertical list grouped under phase headers, cards stacked at full width; corrected by the owner on 2026-11-25 after seeing cards laid left to right); the choice is remembered for the next launch (owner, 2026-11-25, replacing the earlier two-way `v` toggle). Not taken: `v` cycling through the views; the reverse naming of horizontal and vertical.
- Below about 100 terminal columns the horizontal kanban shows the focused column with tabs carrying each column's count; the vertical kanban already uses the full width and draws unchanged. Not taken: tabs in both, switching to the vertical kanban automatically, squeezed columns.

- In the vertical kanban each phase is a bordered box with its title and count in the top border (an empty phase shows an empty box); the cards inside drop their own border, keep the due-bucket bar and are separated by a thin dashed line; one blank line between boxes (owner, 2026-11-25, after seeing headers run into the previous phase's cards). Not taken: a colored full-width title band with a blank line, nested boxes keeping the card borders.

### Plan




Checked against the code on 2026-11-25, after issue 0098 merged (PR #59, released in v0.12.7). The slices run one after another in the main checkout: they share `src/cli.rs`, `src/main.rs` and the TUI model. The pure rule ships with its first caller (the JSON), because a module with no caller fails `clippy -D warnings` as dead code.

- S1: the phase rule in a new `src/board.rs` (`Phase`, `BoardMapping` with built-in keyword defaults, `place(labels, task_list, mapping) -> Placement {phase, flagged}`), wired into `ac mine --json` and `ac_read mine` (`phase`, `flagged`, `phases`), the `--phase` flag and the `ac_read mine` `phase` argument, with the built-in mapping; skill and README (AC1–AC4) | files: `src/board.rs`, `src/main.rs`, `src/agent_json.rs`, `src/commands/mine.rs`, `src/render/mod.rs`, `src/cli.rs`, `src/mcp/mod.rs`, `.claude/skills/active-collab/SKILL.md`, `README.md` | after: none
- S2: `ac setup board` (show, `set <column> --lists … --labels …`, `reset`), stored as one JSON value under a `settings` key through `SettingsRepository`, in the shape of `setup_theme` (src/commands/setup.rs:125); `ac mine --json`, `ac_read mine` and the TUI read it, defaults when unset (AC5) | files: `src/board.rs`, `src/commands/setup.rs`, `src/cli.rs`, `src/main.rs`, `src/mcp/mod.rs`, `locales/pt_BR.json`, skill, README | after: S1
- S3: the kanban in the `ac mine` TUI: `TaskRow` (src/tui/model.rs:66) and the mine snapshot writer (src/tui/mod.rs:~480, today writing empty labels and list) carry labels and the list name so a seeded launch shows the right columns; a kanban view of `Screen::Tasks` with column focus (←/→) and card selection (↑/↓); `v` toggles and the choice persists in the settings; `Enter` opens the selected card's task; column titles through `t()` (AC6) | files: `src/tui/model.rs`, `src/tui/mod.rs`, `src/tui/events.rs`, `src/tui/view.rs`, a new kanban screen under `src/tui/screens/`, `locales/pt_BR.json` | after: S2

- S4: `v` opens a view modal (due-date list, horizontal kanban, vertical kanban) built on the existing modal primitive; the vertical kanban is one list grouped under phase headers with cards stacked (the list layout), ↑/↓ between cards across phases, ←/→ between phases; `mine_view` stores `list`, `kanban_horizontal` or `kanban_vertical` (the unreleased `kanban` value reads as horizontal) | files: `src/tui/model.rs`, `src/tui/view.rs`, `src/tui/footer.rs`, `src/tui/events.rs`, `src/tui/mod.rs`, a new vertical kanban screen, `locales/pt_BR.json` | after: S3
- S5: below about 100 columns the horizontal kanban shows the focused column with count tabs; `l` on a card in any mine view opens the task detail with the task list picker, and the mine screen reloads when the task moved; the kanban footers fit 80 columns (AC7) | files: the kanban screens, `src/tui/model.rs`, `src/tui/events.rs`, `src/tui/footer.rs` | after: S4

Cheap choices already taken: cards in each column keep the due-date view's order; the card's colored bar is its due bucket; no drag between columns (a column does not say which list of the project to move to), `l` moves the list instead; `r` reuses the mine revalidation; no extra request; when labels point to several phases the furthest phase wins; an open task in a `Done` list counts as READY FOR RELEASE; a label that matches only flag keywords does not decide the phase.

### Acceptance



- AC1: each task falls into the phase its `STATUS:` label gives, else its list keyword, else TO DO; waiting and blocked labels set the flag. Cases from the real lists and labels of 2026-11-25 (`Doing`, `⚙️ Doing Geral`, `To Test`, `🔍In review`, `✅ Ready to go`, `Go live`, `Acompanhamento`; `STATUS:GO LIVE`, `STATUS: GO LIVE`, `STATUS:GOLI`, `STATUS:AGUARDANDO CLIENTE`, `STATUS:DESENVOLVIMENTO`). (`verify_by: test`)
- AC2: `ac mine --json` carries `phase` and `flagged` on every task and a `phases` array of `{key, count}`; every key it printed before keeps its name, type and value. (`verify_by: test`)
- AC3: `ac mine --phase <key> --json` keeps only that phase and combines with `--label` and `--task-list`; an unknown key exits 2 listing the valid keys. (`verify_by: test`)
- AC4: the MCP `ac_read` `mine` operation returns the same `phase`, `flagged` and `phases` as `ac mine --json`, takes a `phase` argument that filters the same way (an unknown key is an invalid-params error), lists it with a description in the tool schema, and the served skill documents the fields, the argument and the key vocabulary. (`verify_by: test`)
- AC5: `ac setup board` shows the mapping, `set` changes one column's list and label keywords, `reset` restores the defaults; the next `ac mine --json`, `ac_read mine` and TUI launch use the new mapping. (`verify_by: test`)
- AC6: in the `ac mine` TUI, `v` opens a view modal offering the due-date list, the horizontal kanban (four phase columns side by side) and the vertical kanban (one list grouped under the four phase headers, cards stacked), titles in the i18n language, and the next launch opens in the last chosen view. (`verify_by: test`)
- AC7: below about 100 terminal columns the kanban shows the focused column with tabs carrying each column's count; `Enter` opens the task detail and `l` opens the task list picker from a card. (`verify_by: test`)

### Checkpoint

- 2026-11-25: S1–S3 committed (0943e8f); issue 0101 fixed (ca98c15). S4 (view modal, vertical kanban as bordered phase boxes) approved through three rounds; after round 3 one blocking test gap remained (the top-cut test cannot fail; production correct). The owner chose to close S4 and track the gap as issue 0105 instead of a fourth round. Not taken: a fourth test-only round.
- Next: issue 0104 (raw due-date keys in English) as its own commit, then S5 (narrow tabs for the horizontal kanban, `l` on a card, reload after a list move), then pull request and release.

### Outcome

Implemented on branch `feat/ek-0100-open-work-phase-kanban` (2026-11-25/26) in five reviewed slices, plus three bug fixes found on the way:

- S1: `phase`, `flagged` and `phases` in `ac mine --json` and `ac_read mine`; `--phase` and the `ac_read` `phase` argument; the rule in `src/board.rs`.
- S2: `ac setup board` (show, set, reset); the settings hold only the user's overrides over the built-in keywords.
- S3: the horizontal kanban in the `ac mine` TUI, remembered across launches.
- S4: `v` opens a view modal (due-date list, horizontal kanban, vertical kanban); the vertical kanban draws one list of bordered phase boxes.
- S5: `l` on a mine task opens its detail with the task list picker; Back after a successful edit reloads the mine screen; the horizontal kanban turns into tabs below 100 columns.
- Fixes: `r` on the mine screen (issue 0101), English due-date text (issue 0104).

Proof: live `ac mine --json` on 2026-11-25 placed the owner's 14 open tasks as to_do 4, in_progress 4, in_review 0, ready_for_release 6; `ac mine --phase in_progress` returned the 4. Each slice was reviewed with planted mutants; final run `cargo test -- --test-threads=1` green (2101 unit tests and every integration suite), clippy `--all-targets -D warnings`, `fmt --check`, `comment_policy`, the skill stub check, the Node package tests and `se-gates check` green.

Left open, tracked: 0102 (a failed mine reload blanks the screen and cache), 0103 (`r` on a browse task list), 0105 (the top-cut test cannot fail; accepted by the owner), 0106 (an edit reply after Back does not reload), the crate-wide test language lock (memory task locale-test-race).
