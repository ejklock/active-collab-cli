---
type: Issue
title: "R4 — get + current commands (fetch + render)"
description: Parse task ref/branch, cache-first fetch, render task + comments with display flags.
status: closed
labels: [rust, cli, parity]
blocked_by: [4]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R4 — get + current commands (fetch + render)

Implement task lookup and rendering. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) (requirement 3); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R4 of plan
`rust-rewrite`.

### Scope

Included: parse task ref (URL or `PROJECT/TASK`) and current git branch; cache-first
fetch with `--refresh`; render task + comments; `--json`, `--short`,
`--no-comments`. Kept: render parity with the Python `render`.

### Acceptance

- `get <URL|P/T>` and `current` render parity with the Python output.
- Each flag (`--json`/`--short`/`--refresh`/`--no-comments`) behaves as today.
- A cached `get` succeeds offline (local-first non-negotiable).

### Plan

Re-planned after R3 (provisional in plan `rust-rewrite`).
