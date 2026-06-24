# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
