---
type: Issue
title: "R5 — mine/list command (table + TUI entry)"
description: Aggregate open tasks across instances; table for non-TTY, TUI for TTY.
status: closed
labels: [rust, cli, tui, parity]
blocked_by: [5]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R5 — mine/list command (table + TUI entry)

Implement the assigned-tasks listing. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) (requirement 4); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R5 of plan
`rust-rewrite`.

### Scope

Included: aggregate open tasks across configured instances; render a table when
stdout is not a TTY; launch the browse TUI when it is. Kept: the non-TTY table
contract from the Python `_render_mine_table`.

### Acceptance

- Non-TTY output is a table matching the Python contract.
- TTY launches the browse TUI.
- `--instance` limits to one instance.

### Plan

Re-planned after R4 (provisional in plan `rust-rewrite`).
