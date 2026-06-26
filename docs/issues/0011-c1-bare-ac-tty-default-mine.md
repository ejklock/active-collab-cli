---
type: Issue
title: "C1 — bare `ac` in a TTY defaults to mine"
description: When no subcommand is given and both streams are a terminal, dispatch mine; otherwise keep help+exit-2.
status: open
labels: [cli, tty, ux]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## C1 — bare `ac` (no args, TTY) runs mine

Make the interactive default "show my tasks". Implements
[ADR 0013](/adr/0013-tty-gated-default-subcommand.md); pins
[BDR 0007](/bdr/0007-bare-invocation-tty-default.md) (amends
[BDR 0003](/bdr/0003-cli-command-output-parity.md) Scenario 3).

### Scope

Included: the `None`-command branch in `src/main.rs` `run` — TTY check
(`std::io::IsTerminal` on stdin+stdout) → dispatch `Mine(default)`, else help+exit
2. Excluded: any change to explicit subcommands or to the BDR 0003 pre-parser
normalizations (ref→get, branch→current), which still run first.

### Acceptance

- Bare `ac` with both streams a TTY dispatches `mine` (BDR 0007 S1).
- Bare `ac` with a non-TTY stream prints help and exits 2 (S2).
- Explicit subcommands are unaffected (S3).
- `get`/`current` normalizations still take precedence over the default (S4–S5).
- The TTY decision is injected for a headless unit test of both arms.

### Plan

Per ADR 0013: route in `run` only; reuse the existing `mine` TTY-vs-table logic
downstream. Add the forward-link amendment note to BDR 0003 Scenario 3.
