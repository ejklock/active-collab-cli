# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This changelog tracks the **Rust crate `ac`** (git tags `v0.1.0`+). The
pre-cutover Python package's history is preserved verbatim under
[Pre-Rust (Python) history](#pre-rust-python-history) for provenance; its
version numbers are a separate line and do not continue into the Rust crate
(issue 0055).

## [0.5.0] - 2026-07-17

### Added

- Comment compose editor adopts `tui-textarea`: caret movement, selection, word
  delete, and undo/redo while composing. The shell keeps key authority
  (`Ctrl+S` submit / `Esc` cancel) and `update()` stays pure (ADR 0064).
- Image attachment viewer overlay (slice 1): raster image assets
  (`png/jpg/jpeg/gif/webp/bmp`) open in an in-TUI viewer with a pure
  Loading/Ready/Error lifecycle and a placeholder render. The byte fetch,
  decode, and `ratatui-image` render land in a follow-up slice (ADR 0065).

### Fixed

- Docker cold builds: `.claude/skills` is copied into the builder stage so the
  embedded agent skill compiles from a clean context.

### Documentation

- The `active-collab` agent skill documents the `ac comment` write path and its
  HTML body formatting rule (one `<p>` per line, `<p>&nbsp;</p>` between
  sections); plain newlines collapse and render glued.

## [0.4.0] - 2026-07-09

### Added

- Semantic design-system tokens: a role-named 13-token palette with three
  palettes (Angie default, Slate & Amber, Nord Frost) in dark and light,
  swappable at runtime (ADR 0060).
- Theme selection persisted in the settings table under `theme`, with an
  `ACTIVE_COLLAB_THEME` env override (env wins) applied at startup, and a new
  `ac setup theme [angie|slate|nord]` command (ADR 0062).
- Accent + bold panel titles rendered structurally over the Details /
  Description / Comments top borders (ADR 0063).
- Detail footer now advertises the up/down arrows alongside `j`/`k` for comment
  navigation (the arrows already worked; only the hint was hidden).

### Changed

- Squared legend/panel corners crate-wide via the single-homed box-drawing
  corner glyphs (ADR 0061).
- No breaking changes; the on-disk SQLite schema is unchanged.

## [0.3.0] - 2026-07-03

### Changed

- **Breaking:** the agent skill is renamed from `ac-json` to `active-collab`;
  update any harness references and re-run `install-skill.sh`.

### Added

- `install-skill.sh` gains `--scope project|global` to control where the skill
  stub is written.

## [0.2.0] - 2026-07-03

### Added

- `ac skill` command serves the embedded agent skill as machine-readable output
  (ADR 0057, BDR 0031).
- `install-skill.sh` distributes the agent skill to six agent harnesses.

### Documentation

- English-only demo screenshot, a Quickstart section, and an
  unofficial / not-affiliated disclaimer in the README.

## [0.1.0] - 2026-07-03

### Added

- First release of `ac` — the ActiveCollab task CLI + TUI as a single
  self-contained Rust binary (ratatui + crossterm); no runtime, interpreter, or
  host dependencies required.
- Interactive TUI: browse projects and tasks, open a task detail with rich-text
  description, comments, and inline attachments; keyboard-driven navigation and
  text selection.
- CLI parity: `ac get`, `ac current`, `ac mine`, `ac browse`, and a
  non-interactive `ac comment`, each with a curated `--json` output for
  agent/LLM consumption.
- Multi-instance with per-instance token host-isolation; a token only ever
  reaches its own host.
- Comment authoring: create, edit, and delete your own comments, with a
  server-truth refresh after each mutation.
- Stale-while-revalidate caching for the task list and the project-name
  directory; a warm refresh only re-fetches what changed.
- Actionable re-auth: a revoked token (HTTP 401) surfaces clear guidance to run
  `ac setup add`.
- Prebuilt binaries for Linux (x86_64), macOS (x86_64 + arm64), and Windows
  (x86_64); `install.sh` (POSIX sh) and `install.ps1` (PowerShell) one-liner
  installers.

## Pre-Rust (Python) history

The entries below are the pre-cutover **Python** package's changelog (the
`__version__` line, curses TUI). Their version numbers are a separate line from
the Rust crate above — the same numbers mean different things — and are kept
verbatim for provenance (issue 0055). The Python package was removed entirely in
the Rust cutover (Rust `[0.1.0]`).

### [1.0.0] (Python) - 2026-06-25

#### Changed

- The Rust (ratatui + crossterm) single binary `ac` is now the shipped
  application. The Python package (`src/active_collab`, `pyproject.toml`,
  `legacy/`) has been removed entirely.
- The Cargo crate has been promoted to the repo root. `Cargo.toml`, `src/`,
  `tests/`, `locales/`, `Dockerfile`, and `docker-compose.yml` now live at the
  top level (previously under `rust/`). Build commands are unchanged:
  `docker compose run --rm dev cargo build|test|clippy|fmt`.
- The on-disk SQLite schema (`~/.config/active-collab/active-collab.db`) is
  preserved unchanged, so existing users' instance configuration and task cache
  continue to work without migration.

### [0.5.0] (Python) - 2026-06-24

#### Added

- Task detail redesign: structured meta fields rendered as a full-grid rounded
  bordered table (`├──┼──┤` separators, label | value columns, `Details` title
  embedded in the top border). Task name stays in the frame title, not the grid.
- Assignee field in the meta grid now renders as `Name (id)` when the user name
  is known, or `(id)` alone when only the ID is available.
- Artifacts panel: a rounded `Artifacts` box in the detail view lists each image,
  attachment, and link as `[n] name` with its real URL on an indented line.
  Terminal emulators that auto-link URLs make them clickable without any escape
  codes.
- Clickable links: mouse-clicking any `http`/`https` URL in the detail view opens
  it in the browser via `curses.BUTTON1_CLICKED` hit-testing. Only `http`/`https`
  schemes are accepted; all other schemes are ignored.
- Digit hotkeys `1`–`9` in the detail view open the matching artifact in the
  browser via `controller.open_asset`; out-of-range digits are safe no-ops.
- Detail footer now includes a `[1-9] open` cap when artifacts are present.
- Footer redesign: hint bar uses key-cap style (`[key] action`) rendered on the
  default terminal background — no colored bar.
- i18n (en + pt_BR): lightweight in-code dict catalog via `__()` helper; locale
  resolved at startup with precedence `ACTIVE_COLLAB_LANG` env → SQLite
  `language` setting → `en`. All user-facing strings in `render.py`, `cli.py`,
  and `tui.py` are now wrapped, including help text in argparse. Works inside
  a PyInstaller `--onefile` binary with zero bundling overhead.
- `active-collab setup language [code]`: show or set the display language. With
  no argument it prints the current language code; with `en` or `pt_BR` it
  validates the code and persists it to the SQLite settings table so the choice
  survives across invocations.
- Interactive Settings screen in `browse`: press `[s]` from any list screen to
  open the Settings panel. Arrow-key picker for **Language** (`en` / `pt_BR`)
  and **Active instance** (when multiple instances are configured). Both choices
  persist to SQLite; language changes take effect immediately without restarting.
- The `browse` TTY-guard error (`"Error: 'browse' requires an interactive terminal
  (TTY)."`) is now routed through `__()` with a `pt_BR` catalog entry.
- `__version__` bumped to `0.5.0`.

### [0.4.0] (Python) - 2026-06-24

#### Added

- Task detail view now renders in a rounded frame with `#{num} — {name}` embedded
  in the top border and a bottom hint bar inside the frame.
- Each comment is rendered in its own rounded sub-box with the author and date in
  the box's top border and the body wrapped inside.
- Vertical scrolling in the task detail view: `↑`/`↓` (or `k`/`j`) scroll one
  line; `PgUp`/`PgDn` scroll one viewport. Offset is clamped so scrolling never
  goes past the end.
- Responsive layout: the detail view recomputes its layout on `KEY_RESIZE` and
  guards against too-small terminals without crashing.
- `__version__` bumped to `0.4.0`.

### [0.3.0] (Python) - 2026-06-24

#### Added

- `mine` is now interactive in a terminal (TTY): opens an arrow-key list of your
  open tasks aggregated across all configured instances; select a task to view
  detail, create a git branch, or open/download assets. Falls back to the plain
  table when output is piped or redirected.
- Colorized TUI across all screens (`browse` and `mine`): cyan bold header,
  box-drawing `─` separator, `▸` selection marker with cyan/reverse highlight,
  and a styled status bar. Degrades gracefully to `A_BOLD`/`A_REVERSE` on
  terminals without color support (`curses.has_colors()` guard).

#### Changed

- `BrowseController.fetch_open_tasks()` is now a public method; `MineController`
  uses it instead of reaching into the private `_client`, removing the
  `SLF001` suppression.
- Extracted `_resolve_browse_instance()` from `browse run()` to reduce its
  cyclomatic complexity (now ≤ 8).
- `__version__` bumped to `0.3.0`.

### [0.2.0] (Python) - 2026-06-24

#### Added

- `browse` — interactive curses TUI: arrow-key navigation through your open
  tasks grouped by project, task detail view, git-branch creation, and asset
  open/download.
- Branch creation from task detail: names the branch
  `<type>/<project_id>-<task_id>` (type `feature`/`fix`/`hotfix`, default
  `feature`) off `master`; never overwrites an existing branch.
- Asset extraction (`assets` module): image, attachment, and link URLs from the
  task body, comments, and `attachments[]`, deduped and order-preserved.
- Asset open-in-browser and download actions. The `X-Angie-AuthApiToken` header
  is attached on download **only** when the asset URL's scheme and host match
  the configured instance; foreign hosts are fetched without credentials.
- `render_task_to_str` — string-returning task formatter reused by the TUI with
  no change to existing CLI output.
- `windows-curses` as a Windows-only (`sys_platform == "win32"`) dependency so
  the TUI works on Windows; runtime stays stdlib-only on macOS and Linux.

#### Changed

- `--instance` now also applies to the `browse` command.

### [0.1.0] (Python) - 2026-06-24

#### Added

- Initial release extracted from ai-configs as a standalone package.
- Layered package architecture: `http`, `config`, `models`, `store`, `client`, `render`, `cli`.
- Multi-instance SQLite-backed configuration (`~/.config/active-collab/active-collab.db`).
- `setup add/list/remove/test` subcommands for instance management.
- `get` — fetch a task by URL or `PROJECT_ID/TASK_ID` short form.
- `current` — fetch the task from the current git branch (pattern: `(feature|hotfix|fix)/PROJECT_ID-TASK_ID`).
- `mine` / `list` — list open tasks assigned to the authenticated user.
- Bare-invocation shortcuts: bare `PROJECT/TASK` arg maps to `get`; empty invocation on a matching branch maps to `current`.
- Flags: `--instance`, `--short`, `--json`, `--no-comments`, `--refresh`.
- Entry points: `active-collab`, `ac`, and `python -m active_collab`.
- Security: token stored at `0600`, DB dir at `0700`; transmitted only via `X-Angie-AuthApiToken` header; password never persisted.
- Cross-platform binary distribution via GitHub Actions PyInstaller matrix (Linux x86\_64, macOS x86\_64, macOS arm64, Windows x86\_64).
- `install.sh` (POSIX sh) and `install.ps1` (PowerShell) one-liner installers.
- 216 unit tests covering all modules.
