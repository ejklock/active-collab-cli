---
type: Issue
title: "R6 — browse TUI to parity (screens + loader + non-stacking refresh)"
description: projects → tasks → detail over tokio, with async loader and single-flight refresh.
status: closed
labels: [rust, tui, async, parity]
blocked_by: [6]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R6 — browse TUI to parity (screens + loader + non-stacking refresh)

Build the full interactive browser on the proven R0 shell. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) (requirements 5, 6); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R6 of plan
`rust-rewrite`.

### Scope

Included: a real controller over `tokio`; a screen stack projects → tasks → detail;
an async loading indicator during fetches; a refresh that drops a duplicate while
one is in flight; detail view with meta/artifacts/comments. Kept: mouse/scroll/
bounded-selection behavior from BDR 0001 across every screen.

### Acceptance

- Screen-stack navigation (push/pop) with keyboard and mouse.
- A loader shows during fetches; a refresh while in flight does not enqueue a
  second request (single-flight) — PRD responsiveness NFR.
- Click/scroll/over-scroll never exit the app on any screen.
- Pure update-layer tests for navigation and the single-flight guard.

### Plan

Re-planned after R5 (provisional in plan `rust-rewrite`). Authors BDRs for the
browse navigation and the loader/refresh behavior.
