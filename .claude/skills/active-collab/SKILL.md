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
- `description` and comment `body` are plain text (HTML stripped).
- `comments` is `[]` when there are none or `--no-comments` is passed.
- `downloaded_attachments` is **additive**: it only appears when `--download-attachments`
  was passed. Each entry has exactly one of `path` (success) or `error` (failure) set.

### `mine` — assigned tasks

```json
{"count":2,"tasks":[
  {"ref":"665/75159","instance":"work","project_id":665,"task_number":75159,"task_id":75159,"name":"..."}]}
```

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
| `--instance <name>` | limit to one configured instance | `mine`, `browse`, `get`, `current` |
| `--download-attachments` | fetch every task attachment + inline image to a local directory | `get`, `current` |
| `--attachments-dir <DIR>` | override the download destination (default: a per-task path under the OS temp dir) | `get`, `current` |

The `get`/`current` JSON path is **cache-aware** and honours `--refresh` and
`--no-comments`. The human (non-`--json`) output of every command is unchanged.

### Downloading attachments for local analysis

`--download-attachments` fetches every downloadable task asset (real file
attachments plus inline `<img>` sources — never arbitrary body hyperlinks) over
the CLI's own authenticated seam and writes each one to disk, so an agent can
`Read` the file directly instead of only seeing a remote URL:

```bash
ac get 665/75159 --download-attachments --json   # adds "downloaded_attachments"
ac get 665/75159 --download-attachments          # prints one summary line
```

Without `--attachments-dir`, files land in a stable, predictable per-task path:
`$TMPDIR/ac-attachments/<project_id>-<task_id>/`. One failed asset never blocks
the others — each gets its own `path` (success) or `error` (failure) outcome.
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

## Notes for agents

- Parse the single line as one JSON object; do not assume pretty-printing.
- The schema is locked by unit tests in `tests/unit/agent_json.rs` — a field
  rename or drop fails a test, so these shapes are stable.
- Need the raw upstream ActiveCollab API payload? Use the upstream API directly;
  this CLI's `--json` is the **curated contract**, not a passthrough.
