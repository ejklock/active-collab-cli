---
type: ADR
title: "Curated, minified JSON output for agent/LLM consumption (--json contract)"
description: Replace the raw pretty-printed --json dump of get/current with a single curated, minified JSON contract shared by get/current/mine/browse, derived from the same helpers as the human renderer so JSON and text never drift, with --json on mine/browse forced non-interactive.
status: Accepted
supersedes:
superseded_by:
tags: [cli, json, agent, llm, contract]
timestamp: 2026-06-26T00:00:00Z
---

# 0011. Curated, minified JSON output for agent/LLM consumption

This record supersedes the raw `--json` dump behaviour of `get` / `current`
(a behavior, not a prior ADR).

## Context

The `ac` binary is primarily an interactive TUI plus a few human-readable
CLI commands. We want a second consumer class — LLM agents and scripts — to read
task data non-interactively without scraping the TUI or the human tables.

Two earlier behaviours were inadequate for that consumer:

- `get` / `current` had a `--json` flag that dumped the **raw ActiveCollab API
  payload, pretty-printed**. It is verbose (high token cost), unstable (coupled to
  the upstream API shape), bypassed the cache, and ignored `--no-comments`.
- `mine` printed only a human table; `browse` was TUI-only with no
  non-interactive output at all.

Force: **a second consumer class (agents/scripts)** needs a stable, low-token,
non-interactive contract — a present integration need, not a hypothetical.

## Decision

Introduce a single **curated, minified JSON contract** shared by all read
commands, exposed through the existing `--json` flag.

1. **Curated schema, not raw.** Each command emits a small, stable object
   containing only the fields an agent needs. Fields are derived with the same
   helpers as the human renderer (`html_to_text`, `fmt_date`, `fmt_ts`,
   `fmt_hours`, and `controller::extract_assets`) so JSON and text never drift.
2. **Minified.** One line, `serde_json::to_string` (compact). No pretty-printing —
   token-efficient.
3. **`--json` is uniform** across `get`, `current`, `mine`, and `browse`. For
   `mine` and `browse`, `--json` forces non-interactive output and never launches
   the TUI, regardless of whether stdout is a TTY.
4. **Pure serialization module.** All shaping lives in `src/agent_json.rs` as pure
   functions over domain values (`serde_json::Value` task + comments,
   `MineTableRow`, `ProjectGroup`), unit-tested without network.
5. **`ref` is round-trippable.** Every task carries `"ref": "PROJECT_ID/TASK_ID"`,
   the exact form `get` accepts, so an agent can chain `browse`/`mine` → `get`.

## Schemas

### `get` / `current` — one task object

```json
{"ref":"665/75159","instance":"work","project_id":665,"task_id":75159,
 "name":"...","status":"open","assignee":"Jane Doe","assignee_id":12,
 "project_name":"...","start_on":"2026-01-02","due_on":"2026-01-09",
 "estimate_hours":8,"logged_hours":3,
 "url":"https://collab.example.com/projects/665/tasks/75159",
 "description":"plain text (HTML stripped)",
 "assets":[{"name":"spec.pdf","url":"https://..."}],
 "comments":[{"author":"John","author_id":7,"created_on":"2026-01-03 14:22","body":"plain text"}]}
```

- `status` is the literal `"open"` or `"completed"` (from `is_completed`) — not the
  translated human label.
- `assignee` is the resolved name or `null`; `assignee_id` is the id or `null`.
- `start_on` / `due_on` are `null` when absent.
- `comments` is `[]` when `--no-comments` is set or there are none.
- The curated path is cache-aware and honours `--refresh` and `--no-comments`.

### `mine` — assigned tasks

```json
{"count":2,"tasks":[
  {"ref":"665/75159","instance":"work","project_id":665,"task_number":75159,"task_id":75159,"name":"..."}]}
```

### `browse` — projects with their open tasks

```json
{"projects":[
  {"project_id":665,"project_name":"...","instance":"work","task_count":3,
   "tasks":[{"ref":"665/75159","task_number":75159,"task_id":75159,"name":"..."}]}]}
```

## Alternatives considered

- **Keep the raw pretty-printed `--json` dump.** Rejected: verbose (token cost),
  unstable (coupled to the upstream API), cache-bypassing, and ignores
  `--no-comments` — the opposite of a stable agent contract.
- **A separate `--agent` / `--llm` flag distinct from `--json`.** Rejected: a
  second flag for "machine output" is redundant; `--json` already means
  machine-readable, so curating it (rather than adding a flag) is the smaller
  surface.
- **Emit pretty (indented) curated JSON.** Rejected for the default: minified is
  token-efficient for the agent consumer; a human who wants to read it can pipe to
  a formatter.

## Consequences

**Positive:**

- Agents get a stable, documented, low-token contract; a companion skill under
  `.claude/skills/` documents it.
- `agent_json.rs` is pure and fully unit-tested, so the schema is locked by tests
  (a field rename or drop fails a test).
- `--json` on `mine` / `browse` is non-interactive by definition.

**Accepted trade-offs:**

- The raw pretty-printed dump is gone. Anyone who needs the raw API payload uses
  the upstream API directly; this CLI's JSON is the curated contract.
- `--json` on `mine` / `browse` does an extra fetch instead of opening the TUI.

## Related

- ADR: [/adr/0006-promote-crate-to-repo-root.md](/adr/0006-promote-crate-to-repo-root.md)
- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md)
- BDR: [/bdr/0010-agent-json-output-contract.md](/bdr/0010-agent-json-output-contract.md)
- BDR: [/bdr/0003-cli-command-output-parity.md](/bdr/0003-cli-command-output-parity.md)
- Issue: [/issues/0015-u21-agent-json-output.md](/issues/0015-u21-agent-json-output.md)
