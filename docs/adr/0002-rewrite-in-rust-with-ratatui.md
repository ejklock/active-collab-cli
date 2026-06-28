---
type: ADR
title: Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker
description: Replace the Python curses/Textual app with a single-binary Rust rewrite to fix cross-platform mouse, distribution, and async defects.
status: Accepted
supersedes: 0001
superseded_by:
tags: [architecture, rewrite, tui, distribution, rust]
timestamp: 2026-06-25T00:00:00Z
---

# 0002. Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker

<!-- Status lives in frontmatter. This ADR supersedes 0001 (Textual migration):
     the UI re-platform *target* changed from Textual to Rust/ratatui. The
     mouse-bug root-cause analysis in 0001 stays valid as evidence. -->

## Context

`active-collab-cli` is a Python CLI + interactive TUI for ActiveCollab. Its TUI
was hand-built on `curses`, refactored into a `tui/` package (plan 460), then a
migration onto **Textual** was started ([ADR 0001](/adr/0001-replace-curses-tui-with-textual.md)).
Three classes of defect — not one — forced a deeper decision than "swap the
widget toolkit":

1. **Cross-platform mouse is broken at the input layer.** On macOS, ncurses does
   not decode scroll/click into `KEY_MOUSE`. The workaround hand-parsed SGR
   escape sequences and only handled scroll; **every left-click (`cb==0`) fell
   through to `return 27`** (Esc) → `Back()` → pop the screen stack → on the root
   screen this **quit the whole app on any click**. Windows `curses` is not even
   in the stdlib.

2. **Distribution is the recurring pain.** `ac browse` broke at runtime with
   `ModuleNotFoundError: No module named 'textual'` because the `ac` entry point
   runs under the homebrew `python@3.12`, while the dependency was installed only
   in a `.venv`. This is the PEP 668 "externally-managed-environment" /
   homebrew-python / venv friction in its purest form: a Python TUI cannot be
   handed to a user as one runnable thing.

3. **Async is bolted onto a sync-designed stack.** The Textual migration
   surfaced a `sqlite3.ProgrammingError` — the cache connection is thread-bound
   (`store.py` opens it without `check_same_thread=False`), so a worker thread
   crashed. The test suite missed it because the TUI tests use a fake controller
   that never touches real SQLite (a test-effectiveness gap).

[ADR 0001](/adr/0001-replace-curses-tui-with-textual.md) (Textual) fixes #1 but
leaves #2 entirely (still a Python app needing an interpreter + a dependency
install) and only patches around #3 (worker threads over a sync core). The
load-bearing driver that Textual cannot satisfy is **single-binary, cross-OS
distribution**. The project owner decided to re-platform the whole application,
not just the UI.

There is no local Rust toolchain on the dev host (`cargo`/`rustc` absent), so the
build itself must be containerized.

## Decision

We will **rewrite the entire application in Rust** and **build/ship it via
Docker**.

**Stack (rejections recorded below):**

- **TUI / input:** `ratatui` + `crossterm` — native, consistent mouse / scroll /
  resize on Linux, macOS, and Windows; no SGR hand-parsing.
- **Async runtime:** `tokio` — network runs as tasks that deliver results as
  messages; the loader and the non-stacking refresh fall out of the message loop.
- **HTTP:** `reqwest` with `rustls-tls` (no system OpenSSL dependency).
- **JSON / models:** `serde` + `serde_json`.
- **Cache + config store:** `rusqlite` (`bundled` SQLite — no system libsqlite),
  preserving the existing on-disk schema contract (instances, settings, task
  cache).
- **CLI:** `clap` (derive) — mirrors the existing subcommands and bare-invocation
  shortcuts.
- **Errors:** `anyhow` (app) + `thiserror` (library boundaries).
- **Paths:** `directories` for cross-platform config/cache locations.

**Architecture — The Elm Architecture (TEA):** a pure `Model` + `update(Msg) ->
Model` + `view(&Model, Frame)`, with `tokio` tasks feeding `Msg`s over a channel.
The core (state transitions, selection/scroll math, in-flight-refresh guard) is a
pure function tested **without a terminal or network**, directly closing the
test-effectiveness gap that hid the SQLite crash.

**Build & distribution — Docker:** a multi-stage `Dockerfile` (cargo builder →
minimal runtime producing a single static binary) plus a `docker-compose.yml`
with a `dev` service (`cargo` + `cargo-watch`, source mounted, TTY for the TUI)
and a release/`build` target that emits the binary. This is the answer to the
"no local toolchain" constraint and to driver #2: the deliverable is one file.

**Migration shape:** spike first — slice **R0** scaffolds the Cargo crate + Docker
and proves a `ratatui` task list with working mouse/scroll (the exact regression
that started this). Then slice to parity (config/SQLite → HTTP/API client →
`setup`/`get`/`current`/`mine`/`browse` → i18n → asset open/download) through the
lean pipeline, **keeping the Python app working until the Rust binary reaches
parity**, then cut over. The new crate lives in `rust/` during the transition and
is promoted to the repo root at cutover.

**Dropped:** the git-branch-from-task feature is not reimplemented (already
decided as low-value).

## Consequences

**Easier / gained:**
- Single static binary — distribution is a file copy; PEP 668 / homebrew / venv
  friction (driver #2) disappears.
- Native cross-platform mouse / scroll / resize via `crossterm` (driver #1).
- Real async via `tokio`: on-screen loader, non-stacking refresh, and no
  thread-bound-DB footgun (driver #3).
- A pure, terminal-free testable core (TEA) — behavior is exercised directly, not
  behind a fake.
- Fast startup, low memory, smaller runtime attack surface (no interpreter).

**Harder / accepted trade-offs:**
- A full rewrite: the ~1100-test Python suite and working code are discarded;
  HTTP/API client, SQLite cache, models, i18n, asset download, and config are all
  re-implemented in Rust.
- Rust build/iteration cost; contributors need Docker (or a local `rustup`).
- The Docker build produces a **Linux** binary; a native macOS binary still needs
  a local `rustup` toolchain or a CI matrix — tracked as a follow-up, not solved
  here.
- The existing Python quality-gate Docker image does not cover Rust; gates become
  `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test` run in-container,
  with the Reviewer as the judgment backstop.
- Cargo pulls from `crates.io`, which is outside the default command sandbox
  allowlist; build/test commands run with the sandbox relaxed.

**Now forbidden:**
- No new feature work on the Python TUI; the Python path is maintenance-only
  until cutover, then removed.

**Fitness functions (these keep the decision true):**
- `cargo clippy -- -D warnings` and `cargo fmt --check` are clean — enforced as a
  gate on every slice.
- The curses "click-quits-app" / "over-scroll-quits-app" regression class is
  pinned by **unit tests on the pure `update()`**: over-scrolling past the last
  row never panics and never exits; a mouse click selects the clicked row; `Quit`
  is the *only* message that sets `should_quit`.
- The core is terminal- and network-free: `update()` is a pure function unit-
  tested without a TTY — the structural guard against the test-effectiveness gap
  that hid the SQLite-thread crash.
- The release Docker stage emits exactly one executable; the binary is static
  (`ldd` reports "not a dynamic executable" for the musl target) — distribution
  is a single-file copy.
- Parity: each re-implemented command has a behavior test asserting the same
  observable output contract as the Python version it replaces.

**Follow-ups:**
- Native macOS / Windows release builds (local `rustup` or CI matrix).
- Secret-at-rest: the token is currently stored in plaintext SQLite (parity);
  evaluate OS keychain (`keyring`) or app-key encryption in a dedicated ADR.
- Rust-aware quality gate (clippy/test) wired into the pipeline.

# References

[1] Superseded decision: [ADR 0001 — Replace the curses TUI with Textual](/adr/0001-replace-curses-tui-with-textual.md)
