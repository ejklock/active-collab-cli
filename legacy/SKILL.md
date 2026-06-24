---
name: active-collab
description: 'Fetch ActiveCollab self-hosted (REST API v1) tasks from the command line. Supports multi-instance SQLite-backed configuration with email+password token exchange (password never stored). Entry points: get (by URL or short form), current (from git branch), mine (open tasks assigned to you via /users/{id}/tasks). Python 3 stdlib only.'
---

# ActiveCollab Skill

Fetch ActiveCollab tasks from one or more configured self-hosted instances. Auth uses the ActiveCollab issue-token endpoint — you supply email+password once during setup, the returned token is stored and transmitted exclusively via the `X-Angie-AuthApiToken` header.

## Prerequisites

- Python 3.10+ in PATH (stdlib only — no pip install)
- Script path: `~/.claude/skills/active-collab/scripts/active_collab.py`
- Git in PATH (for `current` subcommand)

## Data model

All state is stored in a local SQLite database:

| Table | Contents |
|---|---|
| `instances` | Name, base URL, email, token, user_id |
| `ticket_cache` | Fetched task fields, cached per (instance, project_id, task_id) |

**DB path:** `~/.config/active-collab/active-collab.db`

Override with `ACTIVE_COLLAB_DB=/path/to/other.db`.

The database directory is created with mode `0700` and the file with mode `0600`. The token lives only in SQLite and is transmitted exclusively via the `X-Angie-AuthApiToken` HTTP header — never printed, never in a URL, never in process args. The **password is never written anywhere** — only the returned token is persisted.

## Setup

```bash
# Interactive wizard — prompts for each missing field
active_collab.py setup add

# Non-interactive (all flags supplied)
active_collab.py setup add \
  --name collab \
  --url https://collab.myorg.com \
  --email me@myorg.com
  # password is always entered hidden via getpass

# List configured instances (tokens never shown)
active_collab.py setup list

# Remove an instance (also clears its task cache entries)
active_collab.py setup remove --name collab

# Test connectivity
active_collab.py setup test
active_collab.py setup test --name collab
```

### Interactive wizard behaviour

When `setup add` is run with a tty stdin and fields are omitted, the wizard prompts for each missing field. After a successful save, a connectivity check runs (`GET /api/v1/projects`) and prints `Connectivity: OK` or `Connectivity: FAILED (...)`. A failed check exits 0 — the save already succeeded.

Non-interactive (stdin not a tty): missing required field exits 2 with no prompts and no network calls.

## API response shapes (ActiveCollab 7.2.25)

- **Single task** (`GET /api/v1/projects/{p}/tasks/{t}`): returns a dict with `single` (the task) and `comments` (list) at the top level. `tracked_time` (logged hours, float) is at the top level of the payload — not inside `single`. The script unwraps `single` for display. Comments come inline — no separate comments endpoint is called.
- **Users** (`GET /api/v1/users`): returns a flat list of user objects with `display_name` (preferred), falling back to `first_name`+`last_name`, then `email`. Fetched once per human-render call to resolve the task assignee name.
- **My Work** (`GET /api/v1/users/{user_id}/tasks`): returns a dict with `tasks` list. Used by `mine` — one call instead of a per-project fan-out.
- **Projects** (`GET /api/v1/projects`): returns a bare list of project dicts.

## Task view fields

The human-readable task view shows:

| Field | Source | Notes |
|---|---|---|
| Assignee | `single.assignee_id` resolved via `GET /api/v1/users` | Rendered as `Name (id)`; `(id)` when unresolved; `(unassigned)` when absent |
| Start | `single.start_on` (unix timestamp) | Shown as `YYYY-MM-DD`; omitted when not set |
| Due | `single.due_on` (unix timestamp) | Shown as `YYYY-MM-DD`; omitted when not set |
| Estimate | `single.estimate` (float, hours) | Shown as `<h>h`; whole numbers drop `.0` |
| Logged | `tracked_time` at payload top level (float, hours) | Shown as `<h>h`; whole numbers drop `.0`; persists through cache |

`--short` and `--json` are unaffected: `--short` prints only `PROJECT/TASK<TAB>name`; `--json` prints the raw payload. Neither calls `/api/v1/users`.

## Fetching tasks

```bash
# By short form PROJECT_ID/TASK_ID
active_collab.py get 665/75159

# By full URL
active_collab.py get https://collab.myorg.com/projects/665/tasks/75159

# From the current git branch (feature|hotfix|fix)/PROJECT_ID-TASK_ID
active_collab.py current

# List open tasks assigned to you (fetches GET /api/v1/users/{user_id}/tasks)
active_collab.py mine

# Force a named instance
active_collab.py get 665/75159 --instance collab

# Short one-liner (PROJECT/TASK<TAB>name)
active_collab.py get 665/75159 --short

# Omit comments
active_collab.py get 665/75159 --no-comments

# Raw task JSON (always hits API, bypasses task cache)
active_collab.py get 665/75159 --json

# Force re-fetch and refresh the task cache
active_collab.py get 665/75159 --refresh
```

### Branch pattern for `current`

The branch must match: `(feature|hotfix|fix)/PROJECT_ID-TASK_ID`

Examples that match: `feature/665-75159`, `hotfix/100-200`, `fix/1-99`

On no match or detached HEAD, exits 2 with a message naming the expected pattern.

## Flags reference

| Flag | Effect |
|---|---|
| `--instance NAME` | Force a specific instance (required when >1 instance is configured) |
| `--short` | Print `PROJECT/TASK<TAB>name` only |
| `--no-comments` | Omit comments section |
| `--json` | Print raw task JSON (always calls API) |
| `--refresh` | Bypass task cache and re-fetch |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Task not found / HTTP error / parse error |
| 2 | Usage error, unknown instance, no instances configured, branch mismatch |
