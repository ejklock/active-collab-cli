---
name: active-collab
description: Read ActiveCollab task data — a task, your assignments, comments, or projects — as machine-readable JSON from the `ac` CLI, non-interactively without the TUI. Use when an agent or script needs to fetch a task by id or URL, list the logged-in user's open tasks, read the task for the current git branch, or browse projects, and wants structured JSON instead of the interactive terminal UI. Covers `ac get`, `ac current`, `ac mine`, and `ac browse` with `--json` — the curated minified schemas, the round-trippable `ref`, and the cache/`--no-comments` flags. Also covers posting a comment with `ac comment`, whose body is plain text by default (a blank line starts a new paragraph, a newline is a line break) and can be posted as raw HTML unchanged with the opt-in `--html` flag.
---

# ac --json — agent read contract

The `ac` binary exposes a single curated, **minified** JSON contract for
agent/LLM and script consumers across all four read commands. `--json` is
**non-interactive**: on `mine` and `browse` it prints the JSON and exits without
launching the TUI, even on a terminal. The fields are derived from the same
renderers as the human output, so JSON and text never drift.

Authoritative decision: [ADR 0011](../../../docs/adr/0011-agent-json-output-contract.md).
Observable behavior + test matrix: [BDR 0010](../../../docs/bdr/0010-agent-json-output-contract.md).

## Prefer the MCP tools

When the `active-collab` MCP server (`ac mcp`) is connected, its tools
`ac_read`, `ac_write` and `ac_download_attachments` are among your tools,
possibly with a harness prefix such as `mcp__active-collab__`. Call them
instead of running `ac` in a shell: they take typed parameters and return
exactly what the matching `--json` command prints.

| Tool | Operations | Instead of |
|---|---|---|
| `ac_read` | `get` · `current` · `mine` | `ac get` · `ac current` · `ac mine` with `--json` |
| `ac_write` | `comment` · `log_time` · `set_task` · `time_report` | `ac comment` · `ac time log` · `ac task set` · `ac time report` |
| `ac_download_attachments` | no `operation`; takes `ref` | `ac get --download-attachments --json` |

The parameters of each operation are in the section `MCP server — ac mcp` below.

## Otherwise use the CLI

When those tools are not available, run the `ac` commands documented below.
`ac browse` has no MCP operation, so it always runs in the CLI.

## Commands

| Command | Emits | TUI? |
|---|---|---|
| `ac get <ref> --json` | one task object | n/a |
| `ac current --json` | one task object (task on the current git branch) | n/a |
| `ac mine --json` | `{count, tasks[]}` (tasks assigned to you) | never launches |
| `ac browse --json` | `{projects[]}` (open tasks grouped by project) | never launches |

All output is a **single line** (`serde_json::to_string`, compact). Pipe to a
formatter (`| jq .`) only for human reading — agents should parse the line directly.

## The `ref` — chain browse/mine → get

Every task in every schema carries `"ref": "PROJECT_ID/TASK_ID"`, the exact form
`ac get` accepts. Discover with `mine`/`browse`, then fetch detail:

```bash
ac mine --json                 # → {"count":1,"tasks":[{"ref":"665/75159",...}]}
ac get 665/75159 --json        # → the full task object for that ref
```

## Schemas

### `get` / `current` — one task object

```json
{"ref":"665/75159","instance":"work","project_id":665,"task_id":75159,
 "name":"...","status":"open","assignee":"Jane Doe","assignee_id":12,
 "project_name":"...","start_on":"2026-01-02","due_on":"2026-01-09",
 "estimate_hours":8,"logged_hours":3,
 "task_list_id":8852,"task_list":"Go live","labels":[{"id":707,"name":"STATUS:GO LIVE"}],
 "parents":[{"ref":"817/78972","project_id":817,"task_id":78972,"task_number":36,"name":"...",
   "status":"open","task_list_id":8852,"task_list":"Go live","labels":[{"id":707,"name":"STATUS:GO LIVE"}]}],
 "children":[],
 "url":"https://collab.example.com/projects/665/tasks/75159",
 "description":"plain text (HTML stripped)",
 "assets":[{"name":"spec.pdf","url":"https://..."}],
 "comments":[{"author":"John","author_id":7,"created_on":"2026-01-03 14:22","body":"plain text"}],
 "downloaded_attachments":[{"name":"spec.pdf","url":"https://...","path":"/tmp/ac-attachments/665-75159/spec.pdf","error":null}]}
```

- `status` is the literal `"open"` or `"completed"` (from `is_completed`), never a
  translated human label.
- `assignee` is the resolved name or `null`; `assignee_id` is the id or `null`.
- `start_on` / `due_on` are `null` when absent.
- `task_list_id` is the id of the task list (board column) the task sits in, or `null`.
- `task_list` is that list's name, or `null` when unknown.
- `labels` is the task's labels as `{id, name}` objects in ActiveCollab's order, `[]` when none.
- `parents` and `children` are the task's ActiveCollab dependencies (a deploy task lists the
  tasks it ships as parents). Each entry has `ref`, `project_id`, `task_id`, `task_number`,
  `name`, `status`, `task_list_id`, `task_list` (name, `null` when it could not be resolved)
  and `labels`, in ActiveCollab's order; trashed tasks are left out. Both are `[]` when the
  task has none and `null` when the lookup was skipped (`--no-dependencies`, or `no_dependencies`
  on the MCP `get`) or failed, so `null` never means "no dependencies". They are looked up on
  every `--json` read (never cached); the human and `--short` output do not show them.
- `description` and comment `body` are plain text (HTML stripped).
- `comments` is `[]` when there are none or `--no-comments` is passed.
- `downloaded_attachments` is **additive**: it only appears when `--download-attachments`
  was passed. Each entry has exactly one of `path` (success) or `error` (failure) set.

### `mine` — assigned tasks

```json
{"count":2,"tasks":[
  {"ref":"665/75159","instance":"work","project_id":665,"task_number":75159,"task_id":75159,"name":"...",
   "due_on":"2026-11-13","due_bucket":"this_week","estimate_hours":"4","logged_week_hours":"3.5","logged_total_hours":"7.25",
   "task_list_id":8852,"task_list":"Go live","labels":[{"id":707,"name":"STATUS:GO LIVE"}]}],
 "buckets":[{"key":"overdue","count":0,"estimate_hours":null,"logged_week_hours":"0","logged_total_hours":"0"}]}
```

- `due_on` is `YYYY-MM-DD` or `null`.
- `task_list_id`, `task_list` and `labels` mean the same as in `get`: the list id (or `null`),
  the list name (or `null`) and the `{id, name}` labels (`[]` when none).
- `due_bucket` is one of `overdue`, `today`, `this_week` (after today, up to Sunday of the
  current Monday–Sunday week), `later` or `none` (no due date); `tasks` keep their order.
- `buckets` always has five entries in the order `overdue`, `today`, `this_week`, `later`,
  `none`, each `{key,count,estimate_hours,logged_week_hours,logged_total_hours}`.
  `estimate_hours` is `null` when no task in the bucket has an estimate;
  `logged_week_hours` and `logged_total_hours` are `null` when any task's value in the
  bucket is unknown, and `"0"` for an empty bucket.
- `estimate_hours`, `logged_week_hours` (your hours on the task this Monday–Sunday,
  rounded to 2 decimals) and `logged_total_hours` (the task's total logged hours) are
  strings formatted like `get`'s hours; each is `null` when unknown or not estimated.

### `browse` — projects with their open tasks

```json
{"projects":[
  {"project_id":665,"project_name":"...","instance":"work","task_count":3,
   "tasks":[{"ref":"665/75159","task_number":75159,"task_id":75159,"name":"..."}]}]}
```

## Flags

| Flag | Effect | Applies to |
|---|---|---|
| `--json` | curated minified JSON; non-interactive | `get`, `current`, `mine`, `browse` |
| `--no-comments` | omit the `comments` array (emits `[]`) | `get`, `current` |
| `--refresh` | ignore the cache and re-fetch | `get`, `current` |
| `--no-dependencies` | skip the dependencies lookup (`parents` and `children` are `null`) | `get`, `current` |
| `--instance <name>` | limit to one configured instance | `mine`, `browse`, `get`, `current` |
| `--label <name>` | keep tasks carrying the label (exact name, case-insensitive); repeat to require every one | `mine` |
| `--task-list <name\|id>` | keep tasks in the list: all digits = list id, else the list name (case-insensitive, any project); an unmatched name yields `count` 0 | `mine` |
| `--download-attachments` | fetch every task attachment, comment attachment and inline image to a local directory | `get`, `current` |
| `--attachments-dir <DIR>` | override the download destination (default: a per-task path under the OS temp dir) | `get`, `current` |

The `get`/`current` JSON path is **cache-aware** and honours `--refresh` and
`--no-comments`. The human (non-`--json`) output of every command is unchanged.

### Downloading attachments for local analysis

`--download-attachments` fetches every downloadable task asset (real file
attachments of the task and of its comments, plus inline `<img>` sources — never arbitrary body hyperlinks) over
the CLI's own authenticated seam and writes each one to disk, so an agent can
`Read` the file directly instead of only seeing a remote URL:

```bash
ac get 665/75159 --download-attachments --json   # adds "downloaded_attachments"
ac get 665/75159 --download-attachments          # prints one summary line
```

Without `--attachments-dir`, files land in a stable, predictable per-task path:
`$TMPDIR/ac-attachments/<project_id>-<task_id>/`. One failed asset never blocks
the others — each gets its own `path` (success) or `error` (failure) outcome.
In the TUI, `D` on a task detail saves the same set to `~/Downloads/ac-attachments/<project_id>-<task_id>/`.
See [ADR 0066](../../../docs/adr/0066-agent-attachment-download-to-local-temp-dir.md).

## Writing a comment — `ac comment`

`ac comment [TASK_REF] -m "<body>"` posts a comment to a task as the logged-in
user. Omit `TASK_REF` to resolve the task from the current git branch; omit
`-m/--message` to read the body from stdin. `--json` prints a curated minified
write result; `--instance <name>` forces a configured instance.

```bash
ac comment 665/75159 -m "Lorem ipsum dolor sit amet."   # explicit ref
ac comment -m "Lorem ipsum dolor sit amet."             # ref from git branch
ac comment 665/75159 < body.txt                          # body from stdin
```

### Default: plain text, encoded to paragraphs and line breaks

By default `ac comment` treats the body as **plain text** and encodes it
before posting: a blank line starts a new paragraph (`<p>…</p>`), a single
newline inside a paragraph becomes a line break (`<br>`), and `<`, `>` and `&`
are escaped, so they show up literally in the rendered comment. Consecutive
paragraphs render with **no visible gap** — ActiveCollab's comment view sets
`p { margin: 0 }` — so a blank line in your plain text does not read as a
blank line in the UI.

```bash
ac comment 665/75159 -m "Section one.

Section two."
```

### Raw HTML — `--html`

Pass `--html` to send the body **unchanged**, with no encoding: newlines carry
no meaning, and `<`, `>` and `&` are not escaped. Use it when the comment
needs markup the default encoder cannot produce — bold text or a visible gap
between paragraphs. Only these tags are verified against ActiveCollab's
renderer: `<p>`, `<strong>`, and a spacer paragraph `<p>&nbsp;</p>` for a
visible gap (a bare `<p></p>` can be collapsed).

```bash
ac comment 665/75159 --html -m "<p><strong>Section one</strong></p><p>Lorem ipsum.</p><p>&nbsp;</p><p>Section two.</p>"
ac comment 665/75159 --html < body.html
```

## MCP server — `ac mcp`

`ac mcp` serves the same commands as MCP tools over stdio, so a harness calls
them with typed parameters instead of composing shell commands. Each session
spawns its own `ac mcp` process; the SQLite cache is shared between them. Every
operation returns exactly what the matching `--json` command prints, and a
failure comes back as an `isError` result with the CLI's error text. See
[ADR 0070](../../../docs/adr/0070-ac-mcp-serves-mcp-over-stdio-as-an-adapter-over-the-command-cores.md).

Three tools (ADR [0072](../../../docs/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md)); `ac_read` and `ac_write` select a command with `operation`; every operation takes an
optional `instance`. `mine` without `instance` lists every configured instance;
every other operation needs `instance` only when several instances are configured:

| Tool | Hint | Operations (parameters) |
|---|---|---|
| `ac_read` | read-only | `get` (`ref`, `no_comments`, `refresh`, `no_dependencies` to skip the parents/children lookup) · `current` (optional `branch`, else the git branch of the server's working directory) · `mine` (optional `labels`, an array of label names that must all match, and `task_list`, a list id or name; same rules as `--label` and `--task-list`) |
| `ac_write` | destructive | `comment` (`ref`, `body`, `html`) · `log_time` (`ref`, `hours`, `date`, `summary`, `job_type`) · `set_task` (`ref`, `status`, `assignee`, `estimate`, `list`) · `time_report` (`period` or `start` + `end`, `format`) |
| `ac_download_attachments` | writes files, idempotent | `ref`, `attachments_dir` (default: the per-task temp path), `instance`; no `operation` field. Fetches the attachments of the task and of its comments, plus inline images. Returns what `ac get --download-attachments --json` prints. `ac_read` `get` does not download. |

`ac task set <ref> --list <id|name>` (and `set_task` `list`) moves a task to another task list of its project: an all-digits value is a list id, any other value a list name matched exactly, ignoring case. An unknown or ambiguous list exits 2, prints the candidates and sends no write. The `--json` result then carries `task_list_id` and `task_list` (the list name). In the interactive TUI task detail, `l` opens a searchable picker that moves the task to another list.

`time_report` has no JSON form: it returns the human line and writes the file in
the server's working directory. When the client declares the MCP tasks extension,
`ac_download_attachments` and `ac_write` `time_report` return a task; cancelling it
ends it `cancelled` at once, but the command still finishes in the background, its
result is dropped and the files it wrote stay. On `ac_read` and `ac_download_attachments`, an
unknown `operation`, a missing required parameter or an unknown parameter is a
JSON-RPC `invalid_params` error.

Register it in Claude Code:

```bash
claude mcp add active-collab -- ac mcp
```

Register it in Codex, in `~/.codex/config.toml`:

```toml
[mcp_servers.active-collab]
command = "ac"
args = ["mcp"]
```

## Notes for agents

- Parse the single line as one JSON object; do not assume pretty-printing.
- The schema is locked by unit tests in `tests/unit/agent_json.rs` — a field
  rename or drop fails a test, so these shapes are stable.
- Need the raw upstream ActiveCollab API payload? Use the upstream API directly;
  this CLI's `--json` is the **curated contract**, not a passthrough.
