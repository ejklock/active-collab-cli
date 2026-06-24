# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-06-24

### Added

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

## [0.3.0] - 2026-06-24

### Added

- `mine` is now interactive in a terminal (TTY): opens an arrow-key list of your
  open tasks aggregated across all configured instances; select a task to view
  detail, create a git branch, or open/download assets. Falls back to the plain
  table when output is piped or redirected.
- Colorized TUI across all screens (`browse` and `mine`): cyan bold header,
  box-drawing `─` separator, `▸` selection marker with cyan/reverse highlight,
  and a styled status bar. Degrades gracefully to `A_BOLD`/`A_REVERSE` on
  terminals without color support (`curses.has_colors()` guard).

### Changed

- `BrowseController.fetch_open_tasks()` is now a public method; `MineController`
  uses it instead of reaching into the private `_client`, removing the
  `SLF001` suppression.
- Extracted `_resolve_browse_instance()` from `browse run()` to reduce its
  cyclomatic complexity (now ≤ 8).
- `__version__` bumped to `0.3.0`.

## [0.2.0] - 2026-06-24

### Added

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

### Changed

- `--instance` now also applies to the `browse` command.

## [0.1.0] - 2026-06-24

### Added

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
