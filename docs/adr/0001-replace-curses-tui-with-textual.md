---
type: ADR
title: Replace the curses TUI with Textual
description: Re-platform the TUI rendering/input layer from curses onto Textual for cross-platform mouse and async.
status: Superseded
supersedes:
superseded_by: 0002
tags: [architecture, tui, textual]
timestamp: 2026-06-25T00:00:00Z
---

# ADR 0001 — Replace the curses TUI with Textual

<!-- SUPERSEDED by 0002 (rewrite in Rust): the UI re-platform target changed from
     Textual to Rust/ratatui because Textual addresses cross-platform mouse but
     not single-binary distribution (PEP 668 / homebrew / venv). The curses
     mouse-bug analysis below remains valid evidence. History preserved as-is. -->

- Date: 2026-06-25
- Deciders: project owner + architect
- Superseded by: [ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md)

## Context

The interactive TUI was hand-built on Python's `curses`. The screen-stack
refactor (plan 460) cleaned up the *structure* (a `tui/` package: `App`,
`Screen` protocol, semantic `Event` taxonomy, per-screen modules) but kept
`curses` as the rendering and input layer.

`curses` is the weakest part of the stack for **cross-platform mouse input**:

- On macOS, ncurses does not decode scroll-wheel or click events into
  `KEY_MOUSE`/`BUTTON*`. The code works around this by enabling SGR mouse
  mode (`?1006h`) and hand-parsing raw escape sequences in
  `terminal._getch_with_mouse`.
- That parser only handled scroll (`cb==64/65`). Any other SGR sequence —
  including **every left-button click** (`cb==0`) — fell through to
  `return 27`. `27` is `Esc`, which maps to `Back()`, which pops the screen
  stack. On the root screen this **quit the whole app on any click**, and
  unhandled scroll did the same. (User-reported regression.)
- On Windows `curses` is not in the stdlib at all (`windows-curses`
  dependency), and its mouse support is poor.

Beyond the bugs, two requested features are awkward in `curses`: an
on-screen **loading indicator** and a **non-blocking refresh** that drops
stacked reload requests while one is in flight. Both require an event
loop with first-class async/worker support, which `curses` lacks.

## Decision

Re-platform the TUI rendering and input layer onto **Textual**
(`textual>=0.80`, built on Rich, already a transitive-friendly dependency
since we use `rich`).

- Textual handles mouse click / scroll / hover / resize natively and
  consistently on Linux, macOS, and Windows — no SGR hand-parsing.
- `LoadingIndicator` covers the loader requirement.
- `@work(exclusive=True)` workers run API calls off the UI thread and
  cancel/replace an in-flight refresh, covering the non-blocking-refresh
  requirement.
- Textual's built-in screen stack (`push_screen`/`pop_screen`) replaces our
  hand-rolled `App`/`Screen`/`Transition` machinery.

**Scope preserved (reused as-is):** the non-UI layers stay untouched —
`controller.py` (`BrowseController`, `MyTasksController`), `store.py`/cache,
`models.py`, `client.py`/`http.py`, `i18n.py`, `assets.py`. Only the visual
border (`app.py`, `events.py`, `terminal.py`, `screen.py`, `graph.py`,
`screens/*`) is replaced.

**Seam:** the CLI enters the TUI only via `tui.run(args)` and
`tui.run_mine(...)`. Those facade functions keep their signatures; their
bodies launch the Textual app. `cli.py` does not change.

**Folded-in decisions:**
- The git-branch-from-task feature is removed during the migration (it has
  no Textual equivalent and the owner judged it low-value). `gitbranch.py`,
  `BrowseController.create_task_branch`, the branch i18n keys, and branch
  tests go away at cutover.
- `windows-curses` is dropped from dependencies after cutover.

## Alternatives considered

- **Patch curses in place** (parse SGR clicks, stop mapping unknown Esc to
  Back, clamp scroll, hand-build a loader/debounce). Rejected: smallest
  effort but keeps the per-terminal/per-OS fragility forever and still
  needs bespoke async for the loader/refresh.
- **prompt_toolkit** — capable full-screen apps + mouse, but lower-level;
  more glue for widgets/layout than Textual.
- **urwid** — mature, but weaker Windows story and an older widget model.
- **blessed** — nicer terminal primitives than curses, but you still build
  the event loop and widgets yourself.

Textual is the most batteries-included fit for a widget-rich app (lists,
scrollable detail, clickable links, modals) with the needed async.

## Consequences

- The screens and event loop are rewritten as Textual widgets/screens;
  the large `curses` test suite (`tests/test_tui*.py`) is largely replaced
  by Textual `Pilot` tests (`App.run_test()`).
- Migration is executed as a multi-slice plan through the lean pipeline
  (Coder → QualityGate → Reviewer), one demoable vertical at a time, with
  the old `curses` path kept working until the final cutover slice.
- New runtime dependency: `textual`. Removed at cutover: `windows-curses`.

## Fitness functions

- After cutover, `grep -rn "import curses" src/active_collab/tui/` returns
  nothing (no `curses` left in the TUI layer).
- Mouse click, scroll, back, and quit are covered by Textual `Pilot`
  tests so the reported regression class cannot silently return.
- Data fetches run inside Textual workers (asserted via Pilot), never on
  the UI thread.
