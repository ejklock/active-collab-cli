---
type: PRD
title: ActiveCollab task CLI + TUI in Rust (parity rewrite)
description: Re-deliver the existing CLI/TUI capabilities as a single-binary Rust app with working cross-platform mouse and async.
status: Completed
superseded_by:
tags: [rewrite, tui, cli, rust, distribution]
timestamp: 2026-06-25T00:00:00Z
---

# 0001. ActiveCollab task CLI + TUI in Rust (parity rewrite)

<!-- Status lives in frontmatter. This PRD specifies the capability set the Rust
     rewrite must deliver; the HOW is ADR 0002, the observable behaviors are the BDRs. -->

## Problem / Motivation

The existing Python CLI/TUI works, but three problems make it painful to ship and
use:

1. **It cannot be handed to a user as one runnable thing.** `ac browse` broke at
   runtime with `ModuleNotFoundError: No module named 'textual'` because the entry
   point ran under a different Python than the one holding the dependency (PEP 668
   externally-managed / homebrew-python / venv friction).
2. **Mouse input is broken in the terminal.** On macOS the curses layer mis-parsed
   click events so that any click — or over-scrolling past the end — quit the whole
   app.
3. **Async is fragile.** Bolting worker threads onto a synchronous core surfaced a
   thread-bound SQLite crash.

The underlying need: a tool that **installs as a single file**, has **reliable
cross-platform input**, and a **real async model** for a responsive UI.

## Goals

- A single self-contained binary a user can run with nothing else installed.
- Mouse, scroll, and keyboard that behave consistently across Linux, macOS, and
  Windows — and never quit the app on a click or an over-scroll.
- Feature parity with the current CLI: `setup`, `get`, `current`, `mine`/`list`,
  `browse`, plus the bare-invocation shortcuts.
- A responsive TUI with a visible loading indicator and a refresh that does not
  stack duplicate in-flight requests.

## Non-goals

- Writing to ActiveCollab (create/edit/comment) — read/browse only.
- Re-implementing the git-branch-from-task helper (removed).
- Encrypting secrets at rest in this phase (tokens stay plaintext in SQLite;
  follow-up ADR).
- Pre-built native macOS/Windows binaries in this phase (Docker → Linux binary;
  follow-up).

## Requirements

1. The app ships as one binary with no runtime interpreter or dependency install.
2. `setup add|list|remove|test|language` manage instances and the display
   language with the same observable output contract as the Python CLI.
3. `get <ref>` and `current` fetch (cache-first) and render a task + comments;
   `--json`, `--short`, `--refresh`, `--no-comments` behave as today.
4. `mine`/`list` shows open tasks assigned to the user — a table when output is
   not a TTY, the interactive TUI when it is.
5. `browse` opens the TUI: navigate projects → tasks → detail with keyboard AND
   mouse; over-scroll and click never exit the app.
6. The TUI shows a loading indicator during fetches; a refresh while one is in
   flight does not enqueue a second.
7. An instance's token is sent only to that instance's host (asset URLs on other
   hosts get no token).
8. Display language switches between English and Brazilian Portuguese.

## Quality requirements (NFRs)

| Quality attribute | Scenario (source · stimulus · artifact · environment · response · measure) | Verified by |
|---|---|---|
| Deployability | An operator · copies the release binary · to a clean host · with no language runtime installed · and runs it · succeeding with zero extra install steps | Release Docker stage emits one static binary; `ldd` reports "not a dynamic executable" (musl) — ADR 0002 fitness function |
| Usability (input robustness) | A user · clicks or over-scrolls past the end · in any TUI screen · on Linux/macOS/Windows · the selection clamps and the app stays open · 0 unintended exits | Unit tests on the pure `update()` (BDR 0001); `verify_by: test` |
| Responsiveness | A user · triggers a refresh while one is in flight · in the browse TUI · under normal use · no second request is enqueued · at most 1 in-flight fetch per group | `@work`/task-group guard with a test asserting single-flight (BDR for browse) |
| Security (token isolation) | The client · issues a request to a non-instance host (e.g. an asset CDN) · from any command · the request carries no instance token · 0 tokens leaked cross-host | Negative test asserting no `Authorization` header off-host; `verify_by: test` |
| Maintainability (testable core) | A developer · runs the core test suite · against the TUI logic · with no TTY/network · all state transitions are covered · core builds/tests headless | `cargo test` on the pure `update()`; ADR 0002 fitness function |

## Acceptance criteria

- A user on a clean host runs the binary with no Python/interpreter present.
- Clicking and over-scrolling in every TUI screen never exits the app.
- Each CLI command's output matches the Python contract for the same input.
- A request to a non-instance host carries no token (asserted by test).
- The core TUI logic test suite runs with no terminal.

## Success metrics

- Install/run friction: from "binary present" to "running" in 1 step (was: install
  Python + deps + correct interpreter).
- Zero reported "app quit on click/scroll" regressions after cutover (was: the
  motivating bug).
- 100% of the planned commands reach output parity with the Python CLI before the
  Python package is removed.

## Behavior (BDRs)

- [BDR 0001 — Task list navigation: mouse, scroll, and bounded selection](/bdr/0001-task-list-navigation.md)
- Further BDRs (command output parity, browse loader/refresh, token isolation,
  i18n) are authored as their slices land.

## Open questions

- Secret-at-rest: OS keychain (`keyring`) vs app-key encryption vs status quo
  (plaintext)? → follow-up ADR.
- Native macOS/Windows release builds: local `rustup` vs CI matrix? → follow-up ADR.

## Decision log

- [ADR 0002 — Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker](/adr/0002-rewrite-in-rust-with-ratatui.md)

## Related

- Constitution: [/constitution.md](/constitution.md)
- Issues: [/issues/index.md](/issues/index.md)
