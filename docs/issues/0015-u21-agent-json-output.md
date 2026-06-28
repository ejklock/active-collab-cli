---
type: Issue
title: "U21 — curated minified --json contract for get/current/mine/browse + agent skill"
description: Add a pure src/agent_json.rs that shapes the ADR 0011 schemas; rewire get/current --json to the curated cache-aware path; add --json to mine/browse forcing non-interactive output; document the contract as a companion skill.
status: open
labels: [cli, json, agent, llm, contract]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## U21 — agent JSON output contract

Give agents/scripts a stable, low-token, non-interactive read contract. Implements
[ADR 0011](/adr/0011-agent-json-output-contract.md); pins
[BDR 0010](/bdr/0010-agent-json-output-contract.md). The human paths
([BDR 0003](/bdr/0003-cli-command-output-parity.md)) are unchanged.

### Scope

Included: a pure `src/agent_json.rs` (no network) shaping the three ADR 0011
schemas from `serde_json::Value` task + comments, `MineTableRow`, and
`ProjectGroup`, reusing `html_to_text`/`fmt_date`/`fmt_ts`/`fmt_hours`/
`controller::extract_assets`; rewiring `get`/`current` `--json` from the raw
pretty dump to the curated, cache-aware, minified line; adding `--json` to
`MineArgs`/`BrowseArgs` and a non-interactive branch that prints the curated line
without launching the TUI; and a companion `.claude/skills/` skill documenting the
contract. Excluded: any change to the TUI rendering or the human text output
(reuse only).

### Acceptance

- `get`/`current` `--json` emit one minified line matching the ADR 0011 task
  schema, cache-aware, honoring `--refresh`/`--no-comments` (BDR 0010 S1,3,4,7).
- `mine`/`browse` `--json` emit one minified line of their schema and never launch
  the TUI, even on a TTY (BDR 0010 S5,6).
- Every task carries `ref` = `project_id/task_id`, the form `get` accepts (S2).
- The shaping functions are pure and unit-tested (field presence, null handling,
  status/assignee mapping, single-line output).
- Human (non-`--json`) output is unchanged (S8 / BDR 0003 parity).
- A companion skill documents the contract for agents.
- Full suite green, clippy/fmt/comment-policy clean, complexity within budget.

### Plan (slices, persisted plan `tui-agent-json-U21`)

- **J1** — `src/agent_json.rs` + `get`/`current` curated minified JSON (rewire the
  raw dump at `commands.rs` `do_get_task` to `load_task` + `task_object`).
- **J2** — `mine --json`: `--json` on `MineArgs`, `mine_object`, non-interactive
  branch in `dispatch_mine`/`mine_core` before the TUI launch.
- **J3** — `browse --json`: `--json` on `BrowseArgs`, `browse_object`,
  non-interactive branch in `dispatch_browse` before `tui::browse`.
- **J4** — companion `.claude/skills/` skill + this doc trail (authored by the
  architect; ADR 0011 reformatted to OKF + indexed).
