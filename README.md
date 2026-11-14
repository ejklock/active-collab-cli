# active-collab-cli — ActiveCollab CLI, terminal UI and MCP server

[![CI](https://github.com/ejklock/active-collab-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/ejklock/active-collab-cli/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/ejklock/active-collab-cli)](https://github.com/ejklock/active-collab-cli/releases/latest)
[![npm: active-collab-agent](https://img.shields.io/npm/v/active-collab-agent?label=npm%20active-collab-agent)](https://www.npmjs.com/package/active-collab-agent)
[![License: MIT](https://img.shields.io/github/license/ejklock/active-collab-cli)](LICENSE)

**Unofficial** command-line client, interactive terminal UI (TUI) and
[MCP](https://modelcontextprotocol.io) server for self-hosted
[ActiveCollab](https://activecollab.com). Read your tasks, comments and
projects, log time, export hours reports and download attachments from the
terminal, or let an AI coding agent (Claude Code, Codex, OpenCode, Pi, Cursor,
GitHub Copilot) do it for you through the `ac mcp` server and the
`active-collab` skill.

One self-contained Rust binary (`ac`, built with ratatui, crossterm and tokio)
for Linux, macOS and Windows. No interpreter or runtime is required.

**Features**

- `ac mine`: your open tasks grouped by due date, with estimate and the hours
  logged this week and in total.
- `ac` / `ac get`: the task for your current git branch, or any task by URL or
  short ref, with comments and attachments.
- `ac browse`: a keyboard-driven TUI over projects, task lists, tasks, comments
  and attachments; comment, log time, move a task to another task list, download
  attachments.
- `ac time log` and `ac time report`: log hours, and export a period's hours as
  XLSX or CSV.
- `--json` on every read command, for scripts and agents.
- `ac mcp`: the same commands as MCP tools (`ac_read`, `ac_write`,
  `ac_download_attachments`), installed in one step by the
  [Claude Code, Codex, OpenCode and Pi plugins](#plugins).
- Several ActiveCollab instances at once; API tokens stored locally in SQLite,
  passwords never stored.
- English and Brazilian Portuguese interface.

> ## ⚠️ Unofficial — not affiliated with ActiveCollab
>
> This is an independent, community-built project. It is **not** an official
> ActiveCollab product and is **not affiliated with, endorsed by, sponsored by,
> or supported by** ActiveCollab or A51 d.o.o. **"ActiveCollab" is a trademark of
> its respective owner** and is used here **only** to describe compatibility with
> the ActiveCollab REST API of your own self-hosted instance. This tool stores no
> credentials beyond a local API token, sends that token only to your configured
> host, and is provided "as is", without warranty. Use at your own risk.

---

## Screenshot

![The ac browse task-detail view, rendered with fictional demo data](docs/assets/detail-demo.svg)

*Interactive TUI — the task-detail view (project, status, description, assets).
The data shown is fictional; no real instance or client data is depicted.*

---

## Quickstart

From zero to your first task in three steps:

```sh
# 1 · Install (macOS / Linux — Windows: see the PowerShell one-liner below)
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.sh | sh

# 2 · Register your self-hosted ActiveCollab instance.
#     Prompts once for your password to exchange an API token — the password is never stored.
ac setup add --name collab --url https://collab.example.com --email you@example.com

# 3 · List the open tasks assigned to you
ac mine
```

To let an AI coding agent use it, add the plugin to your harness (see
[Plugins](#plugins) for Codex, OpenCode and Pi):

```sh
claude plugin marketplace add ejklock/active-collab-cli
claude plugin install active-collab@active-collab
```

Then, day to day:

```sh
ac                  # the task for your current git branch (e.g. feature/665-75159)
ac get 665/75159    # a specific task by short form or full URL
ac browse           # full interactive TUI: projects → tasks → detail → assets
ac mine --json      # machine-readable output for scripts and agents
```

New here? `ac setup add` walks you through it interactively if you omit the flags.
See [Usage](#usage) for every command and flag.

---

## Install

### macOS / Linux (curl one-liner)

```sh
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.sh | sh
```

The script downloads the pre-built `active-collab` binary for your platform from
the latest GitHub Release and places it on your PATH, next to an `ac` symlink —
the short command used throughout this README and by the agent skill. An `ac`
left by an older install is replaced; an `ac` belonging to another program is
kept, with a warning. macOS ships its own `/usr/sbin/ac` (login accounting), so
the installer also warns when that one still wins your PATH lookup.

### Windows (PowerShell one-liner)

```powershell
irm https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.ps1 | iex
```

Installs `active-collab.exe` plus an `ac.cmd` shim, so both `active-collab` and
`ac` work from cmd and PowerShell.

### Manual download

Download the pre-built binary for your platform from the
[Releases page](https://github.com/ejklock/active-collab-cli/releases), place it on your
PATH, and make it executable (`chmod +x active-collab` on Unix). Add the short
alias yourself: `ln -s active-collab ac` (Unix), or an `ac.cmd` forwarder on
Windows.

| Platform | Asset |
|---|---|
| Linux x86\_64 | `active-collab-linux-x86_64` |
| Linux arm64 | `active-collab-linux-arm64` |
| macOS x86\_64 (Intel) | `active-collab-macos-x86_64` |
| macOS arm64 (Apple Silicon) | `active-collab-macos-arm64` |
| Windows x86\_64 | `active-collab-windows-x86_64.exe` |

### Build from source (Docker required)

No local Rust toolchain needed. The crate is at the repo root; Docker provides
the build environment.

```sh
# Development build
docker compose run --rm dev cargo build

# Release binary (placed in target/release/ac)
docker compose build
docker compose run --rm build
```

That binary is a Linux build. `make install` therefore only accepts a Linux host
and refuses elsewhere; on macOS use `make install-native` (needs a host Rust
toolchain) or the release installer above.

---

## Usage

### Setup — manage instances

```sh
# Register an ActiveCollab instance (interactive wizard prompts for missing fields)
ac setup add
ac setup add --name collab --url https://collab.example.com --email me@example.com
# Password is always entered hidden via a prompt — never passed as a flag.

# List configured instances (tokens never shown)
ac setup list

# Remove an instance and its cached tasks
ac setup remove --name collab

# Test connectivity to all (or one) configured instance
ac setup test
ac setup test --name collab

# Show the current display language
ac setup language

# Set the display language (persists to SQLite; survives across invocations)
ac setup language en
ac setup language pt_BR
```

### get — fetch a task by URL or short form

```sh
ac get 665/75159
ac get https://collab.example.com/projects/665/tasks/75159
```

### current — fetch the task from the current git branch

Branch must match `(feature|hotfix|fix)/PROJECT_ID-TASK_ID` (e.g. `feature/665-75159`).

```sh
ac current
```

### mine — list open tasks assigned to you

```sh
ac mine
ac list          # alias
```

When run in a terminal (TTY), `mine` opens an interactive arrow-key list of your
open tasks aggregated across all configured instances. Select a task to view its
detail or open/download its assets. When output is piped or redirected (non-TTY),
`mine` falls back to a plain table suitable for scripts.

Each task shows its estimate, the hours you logged on it this week (Monday to
Sunday) and its total logged hours. An hour value the server does not return
stays blank and never fails the list.

### time report — export your hours for a period as XLSX or CSV

```sh
ac time report --period month|fortnight|week [--format xlsx|csv] [--instance NAME]
ac time report --start 01-08-2026 --end 30-08-2026 [--format xlsx|csv] [--instance NAME]
```

Writes your time records for the period to `hours-report-<start>-<end>.<xlsx|csv>`
in the current directory (dates as `DD-MM-YYYY`), overwriting a file of the same
name; the file is written to a temporary sibling and renamed, so a failed run
leaves an existing report intact. The period is either `--start`/`--end`
(inclusive, both required together) or `--period`, which resolves the current
period from today's local date: `month` is day 1 to the last day of the month,
`fortnight` is 1-15 or 16-last day, `week` is Monday to Sunday.
`--period` cannot be combined with `--start`/`--end`. The format defaults to `xlsx`.

The XLSX file has one sheet, `Hours`: `Total hours for the period` and the total
in row 1, the header `Project`, `Activity`, `Hours`, `Date` in row 3, then one row
per record sorted by date, project and record id; hours are numbers and dates are
Excel dates shown as `dd/mm/yy`. The CSV is `;`-separated UTF-8 with a BOM (opens
in Excel) with the same rows: a `Total hours for the period;<total>` line, an
empty line, the header `Project;Activity;Hours;Date`, then the records, with a
decimal comma (`1,5`).
Run `ac setup add` again if the instance has no stored user id.

In `ac browse`, press `H` on any screen to open the same export as a modal:
pick Monthly, Biweekly, Weekly or Custom (Start/End as `DD-MM-YYYY`, `Tab` moves
between the fields) with `↑/↓` or `j/k`, toggle XLSX/CSV with `f` or `←/→`, and
press `Enter`. It writes the same file as `ac time report` into the current
directory, for the open task's instance (otherwise the first configured one),
and shows the saved path and total; `Esc` closes it.

### browse — interactive TUI

Arrow-key terminal browser for your open tasks. Navigate projects → tasks →
task detail, then open/download the task's assets.

```sh
ac browse
ac browse --instance collab   # required when >1 instance configured
```

The TUI uses ratatui for layout and crossterm for input, giving consistent mouse
click, scroll, and keyboard behavior on Linux, macOS, and Windows. It shows a
loading indicator during fetches and guards against duplicate in-flight refresh
requests (single-flight).

The task detail view shows:

- **Meta table** — a two-column table listing Task, Project, Title, Status,
  Assignee, Start, Due, Estimate, and Logged.
- **Description** — the task body; falls back to `(no description)` when empty.
- **Artifacts panel** — lists each image / attachment / link as `[n] name` with
  its URL.
- **Comments** — one panel per comment (author · date as the panel title).

The detail view scrolls vertically when content exceeds the screen. The TUI
adapts to terminal resize events and guards against too-small terminals without
crashing. The footer shows key-cap style hints (`[key] action`).

Each task you open is written to the local SQLite `ticket_cache` keyed by instance
name; re-opening the same task is instant and offline-tolerant. Press `r` inside
the detail view to bypass the cache and re-fetch from the API.

**Key bindings**

| Screen | Keys |
|---|---|
| Projects / Tasks lists (including `mine`) | `↑`/`↓` move · `Enter` select · `r` refresh · `H` hours report · `q` quit · `Esc`/`b` back (quits from the first screen) · `Ctrl+C` quit |
| Task detail | `↑`/`↓` or `k`/`j` move between comments · `PgUp`/`PgDn` page · `r` refresh · `c` new comment · `t` log time · `e` edit estimate · `s` toggle status · `a` assignee picker · `l` task list picker · `H` hours report · `D` download attachments · `Esc`/`b` back · `q` quit · `Ctrl+C` quit |

### Bare-invocation shortcuts

```sh
ac 665/75159     # same as: ac get 665/75159
ac               # same as: ac current (when branch matches)
```

### Flags

| Flag | Applies to | Effect |
|---|---|---|
| `--instance NAME` | `get`, `current`, `mine`, `browse` | Force a specific configured instance (required when >1 configured) |
| `--short` | `get`, `current` | Print `PROJECT/TASK<TAB>name` only |
| `--no-comments` | `get`, `current` | Omit the comments section |
| `--json` | `get`, `current` | Print raw task JSON (always hits the API, bypasses cache) |
| `--refresh` | `get`, `current` | Bypass the task cache and re-fetch from the API |

---

## AI agents — plugins, MCP server and skill

Three ways to give an AI coding agent your ActiveCollab tasks, from the most to
the least integrated:

| Way | Harnesses | What the agent gets |
|---|---|---|
| [Plugins](#plugins) | Claude Code, Codex, OpenCode, Pi | The `ac mcp` tools and the skill in one install step (Pi: the skill) |
| [MCP server](#mcp-server--ac-mcp) by hand | Any MCP client | The `ac_read`, `ac_write` and `ac_download_attachments` tools |
| [Skill installer](#install-the-skill-into-your-agent-harness) | Claude Code, Codex, OpenCode, Pi, Cursor, GitHub Copilot | The skill, which drives `ac --json` |

All three need `ac` installed first ([Install](#install)), and `ac setup add` run
once for your instance.

### The skill

`ac` ships a self-describing **agent skill** for the `--json` read contract, so an
LLM coding agent can learn how to read your ActiveCollab tasks non-interactively.
The full contract lives in **one place** — inside the binary — and `ac` prints it on
demand:

```sh
ac skill list           # list the available skills
ac skill active-collab  # print the full active-collab contract (schemas, the round-trippable ref, flags)
ac skill                # with one skill registered, prints it directly
```

### Install the skill into your agent harness

Every supported harness loads a small **thin pointer** whose only job is to tell the
agent to run `ac skill active-collab`. Because the pointer carries no schema, it never goes
stale when the `--json` contract changes — only the binary does. Run the installer
from your project root for the harness you use (or `all`):

```sh
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install-skill.sh | sh -s -- --harness cursor
# or wire up every harness at once:
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install-skill.sh | sh -s -- --harness all
```

| Harness | Project file (`--scope project`, default) | User-level file (`--scope global`) |
|---|---|---|
| Claude Code | `.claude/skills/active-collab/SKILL.md` | `~/.claude/skills/active-collab/SKILL.md` |
| pi | `.pi/skills/active-collab/SKILL.md` | `~/.pi/agent/skills/active-collab/SKILL.md` |
| Codex CLI | `.codex/skills/active-collab/SKILL.md` | `~/.codex/skills/active-collab/SKILL.md` |
| OpenCode | `.opencode/skills/active-collab/SKILL.md` | — (install per-project) |
| GitHub Copilot | `.github/skills/active-collab/SKILL.md` | — (install per-project) |
| Cursor | `.cursor/rules/active-collab.mdc` | — (install per-project) |

Claude Code, OpenCode, and pi can also read `.claude/skills/active-collab/SKILL.md`
directly.

**Scope.** By default the installer writes into the **current project**
(`--scope project`); pass `--dir <path>` to target another project. Use
`--scope global` to install once at the **user level** so the skill is available in
every project:

```sh
# install the active-collab pointer once, for every harness that supports a user-level dir
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install-skill.sh | sh -s -- --harness all --scope global
```

Global scope supports Claude Code, pi, and Codex (the harnesses with a standard
user-level skills directory). OpenCode, Copilot, and Cursor have no such directory, so
`--scope global` reports them as unsupported — install those per-project. Running the
installer directly in a terminal (not via `curl | sh`) prompts for project-vs-global; the
piped one-liner stays non-interactive and defaults to project. To install into a specific
pi profile, use `--scope project --dir ~/.pi/agent-profiles/<name>`.

> The skill grants an agent no access beyond the CLI commands you already run
> yourself, including the ones that write (`ac comment`, `ac time log`, `ac task set`).

### Plugins

The Claude Code, Codex and OpenCode plugins install the skill **and** the `ac mcp` server
into a harness in one step (on Pi, the package installs the skill only).
Install `ac` first (`install.sh`, above): each of these plugins starts the `ac` found on your
`PATH`, and needs `ac` 0.12.0 or later (earlier releases lack the `mcp` subcommand).

```sh
# Claude Code
claude plugin marketplace add ejklock/active-collab-cli
claude plugin install active-collab@active-collab

# Codex
codex plugin marketplace add ejklock/active-collab-cli
codex plugin add active-collab@active-collab
```

OpenCode: add the package to the `plugin` list in `opencode.json`:

```json
{ "plugin": ["active-collab-agent"] }
```

Pi: the package gives Pi the skill, which drives `ac` with `--json`:

```sh
pi install npm:active-collab-agent
```

To install from a clone of this repository instead of npm: for Pi, run
`pi install ./packages/active-collab-agent`; for OpenCode, list the absolute path
`"file:///path/to/active-collab-cli/packages/active-collab-agent"` in `plugin`.

`scripts/prove-plugins.sh` installs and removes each plugin from a checkout in a
temporary home, with the harness CLIs it finds on `PATH` (`PI_BIN` names the real `pi`
binary when yours is a wrapper). `install-skill.sh` remains the way to install the skill
for GitHub Copilot and Cursor, which have no plugin here.

### MCP server — `ac mcp`

`ac mcp` serves the same commands as MCP tools over stdio, so an agent calls them
with typed parameters instead of composing shell commands. Each agent session
starts its own `ac mcp` process; all of them share the local cache. Every result is
exactly what the matching `--json` command prints.

| Tool | Operations |
|---|---|
| `ac_read` | `get`, `current`, `mine` |
| `ac_write` | `comment`, `log_time`, `set_task`, `time_report` |
| `ac_download_attachments` | downloads a task's attachments to a local directory (`ref`, `attachments_dir`, `instance`) |

`ac_download_attachments` writes files on the machine running `ac mcp`, so it is
marked neither read-only nor destructive; `ac_read` `get` does not download. See
[ADR 0072](docs/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md).

`ac_write` changes data on your ActiveCollab instance as the logged-in user and is
marked destructive, so a harness that honours MCP tool hints asks before calling it.
The Claude Code, Codex and OpenCode plugins above register the server for you. To register it by hand instead, once per harness:

```sh
# Claude Code
claude mcp add active-collab -- ac mcp
```

```toml
# Codex, in ~/.codex/config.toml
[mcp_servers.active-collab]
command = "ac"
args = ["mcp"]
```

The parameters of each operation are in `ac skill active-collab`. See
[ADR 0070](docs/adr/0070-ac-mcp-serves-mcp-over-stdio-as-an-adapter-over-the-command-cores.md).

---

## Internationalization

The binary ships with English (default) and Brazilian Portuguese (`pt_BR`)
translations for all user-facing output. Translations are embedded at compile time
as JSON catalogs — no external files required at runtime.

**Durable setting** — persist your preferred language to SQLite:

```sh
ac setup language pt_BR   # set
ac setup language          # show current
```

**One-off override** — the `ACTIVE_COLLAB_LANG` environment variable overrides the
stored setting for a single invocation:

```sh
ACTIVE_COLLAB_LANG=pt_BR ac browse
```

**Resolution order:** `ACTIVE_COLLAB_LANG` env var → SQLite setting → `en`.

---

## Configuration

**Database path:** `~/.config/active-collab/active-collab.db`

Override with the `ACTIVE_COLLAB_DB` environment variable:

```sh
ACTIVE_COLLAB_DB=/custom/path/active-collab.db ac get 665/75159
```

---

## Terminal walkthroughs

`qa/*.walkthrough.json` replay four flows (`ac mine` hours, `ac mcp` read
through the MCP Inspector, `ac get --download-attachments`, `ac task set
--list`) with `qa-walkthrough` against a throwaway mock
ActiveCollab (`examples/mock_collab.rs`):

```sh
bash qa/run-walkthroughs.sh
```

The script builds `ac` and the mock natively into `target/native`, installs the
pinned MCP Inspector (2.15.0) under `qa/.run/tools` with `npm ci` from the
lockfile committed in `qa/tools` (the only network fetch),
then checks and replays every walkthrough. Each walkthrough uses its own store,
`HOME` and `TMPDIR` under `qa/.run/<name>/` and the mock's loopback address; it
never opens `~/.config/active-collab` and never reaches your real instance. The
script fails if that store's modification time changes, if a mock outlives the
run, or if another replay holds `qa/.run/replay.lock`.

---

## Security

- The API token is stored only in the local SQLite database with directory
  permissions `0700` and file permissions `0600`.
- The token is transmitted exclusively via the `X-Angie-AuthApiToken` HTTP header —
  never in a URL, never printed, never passed as a process argument.
- The password is **never stored**. Only the token returned from the issue-token
  endpoint is persisted.
- The token is sent only to the configured instance's own host. Requests to asset
  URLs on other hosts carry no token.

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Task not found / HTTP error / parse error |
| 2 | Usage error, unknown instance, no instances configured, branch mismatch |

---

## Development

```sh
# Run all tests (unit + integration, including comment-policy gate)
docker compose run --rm dev cargo test

# Run only the comment-policy gate
docker compose run --rm dev cargo test --test comment_policy

# Lint
docker compose run --rm dev cargo clippy -- -D warnings

# Format check
docker compose run --rm dev cargo fmt --check
```

The Rust version is pinned in `rust-toolchain.toml`; the Docker images and both
workflows read it from there. To raise it, edit that file, rebuild the image
with `docker compose build`, and fix whatever the new clippy lints report, all
in one change.

The TUI core (`src/app.rs` `update`) is a pure function with no terminal or
network dependency — it is unit-tested directly without a TTY.
