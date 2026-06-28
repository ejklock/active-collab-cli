---
type: Issue
title: "R3 — CLI scaffold + setup commands + bare-invocation"
description: clap-based CLI with setup add/list/remove/test/language and bare-invocation shortcuts.
status: closed
labels: [rust, cli, parity, i18n]
blocked_by: [3]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R3 — CLI scaffold + setup commands + bare-invocation

Build the command surface. Part of [PRD 0001](/prd/0001-rust-tui-cli-parity.md)
(requirement 2); implements [ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md).
Slice R3 of plan `rust-rewrite`.

### Scope

Included: `clap` parser; `setup add|list|remove|test|language`; bare-invocation
normalization (`ac <ref>` → get, no args → current); i18n bootstrap. Kept: output
contract parity with the Python `cli.py`.

### Acceptance

- Each `setup` subcommand matches the Python output contract for the same input.
- Bare-invocation shortcuts resolve as today.
- Exit codes match (e.g. 2 on bad usage / missing instance).

### Plan

Re-planned after R2 (provisional in plan `rust-rewrite`). A command-output-parity
BDR is authored with this slice.
