---
type: ADR
title: A bare `ac` invocation in a TTY defaults to `mine`
description: When no subcommand is given and both stdin and stdout are a terminal, run `mine` (the personal task view) instead of printing help and exiting 2; non-TTY bare invocation keeps the help+exit-2 contract so pipes and scripts are unaffected.
status: Accepted
supersedes:
superseded_by:
tags: [cli, ux, tty, clap]
timestamp: 2026-06-26T00:00:00Z
---

# 0013. TTY-gated default subcommand (`ac` → `mine`)

## Context

Today a bare `ac` with no subcommand prints help and exits 2 (`src/main.rs` `run`,
the `None` branch), mirroring the Python CLI and pinned by
[BDR 0003](/bdr/0003-cli-command-output-parity.md) Scenario 3. The user wants the
common interactive case — "show me my tasks" — to be the default: `ac` with no
arguments, in a terminal, should open `mine`. Other subcommands keep working
exactly as they do.

`mine` already does its own TTY detection (`std::io::IsTerminal` on stdout+stdin)
before deciding to enter the TUI vs. print a table, so the machinery exists; only
the bare-invocation routing needs to change.

Force: **ergonomics of the interactive entry point.** The constraint is that the
CLI is also used non-interactively (piped, in scripts, in CI), where silently
launching a TUI — or emitting `mine`'s output — into a pipe would be a surprising
regression. So the default must be gated on actually being at a terminal.

## Decision

In `run`, when `cli.command` is `None`:

- **stdout AND stdin are a TTY** → dispatch `Command::Mine(MineArgs::default())`,
  exactly as if the user typed `ac mine`.
- **otherwise** (either stream redirected/piped) → keep the current behavior:
  print help and exit 2.

This is a routing decision in `run` only; `dispatch_mine` and every other handler
are untouched. The bare-`ac`-as-`get`-ref and bare-`ac`-on-a-task-branch-as-
`current` normalizations ([BDR 0003](/bdr/0003-cli-command-output-parity.md)) run
**before** the parser and are unchanged — they only apply when argv is non-empty
or a task branch matches; the new default covers the remaining "empty argv, no
matching branch, at a TTY" case that previously fell through to help.

This **amends [BDR 0003](/bdr/0003-cli-command-output-parity.md) Scenario 3** and is
re-pinned by the new [BDR 0007](/bdr/0007-bare-invocation-tty-default.md).

## Alternatives considered

- **Always default to `mine` (no TTY gate).** Rejected: piping `ac` (e.g.
  `ac | less`, a CI step, a script that relied on help+exit-2) would either launch
  a TUI into a non-terminal or dump `mine` output unexpectedly. The gate keeps the
  non-interactive contract stable.
- **Default to `browse`.** Rejected: `mine` is the personal landing view; `browse`
  is the explore-everything view. The user said the bare command should be "mine".
- **A configurable default subcommand (setting).** Rejected as over-engineering for
  a single obvious default; can be added later if a second default is ever wanted.
- **Keep help+exit-2 always.** Rejected: that is the status quo the user asked to
  change.

## Consequences

**Positive:** `ac` at a prompt opens the user's tasks — the most common action —
with zero ceremony, while every scripted/piped use keeps the exact prior contract
(help, exit 2). The change is one branch, fully unit-testable by injecting the
TTY-ness decision.

**Accepted trade-offs:** the bare-invocation contract now depends on terminal
detection, so the BDR gains a TTY-vs-not split (two scenarios where there was one).
Tests must cover both arms.

## Related

- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md)
- BDR: [/bdr/0003-cli-command-output-parity.md](/bdr/0003-cli-command-output-parity.md) (amended)
- BDR: [/bdr/0007-bare-invocation-tty-default.md](/bdr/0007-bare-invocation-tty-default.md)
- Issue: [/issues/0011-c1-bare-ac-tty-default-mine.md](/issues/0011-c1-bare-ac-tty-default-mine.md)
