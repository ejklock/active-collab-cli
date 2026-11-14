---
type: Issue
title: Download comment attachments in the CLI and from a TUI key
description: Comment attachments join the downloadable and listed assets, and a TUI key D saves a task's attachments to ~/Downloads/ac-attachments.
status: closed
labels: [cli, tui, attachments, agent, slice]
blocked_by: [0073]
timestamp: 2026-11-11T21:40:56Z
---

## 0076. Download comment attachments in the CLI and from a TUI key

### Problem

Files attached to a comment are invisible to the CLI: `controller::extract_assets` and `controller::downloadable_assets` read `task.attachments` and the inline `<img>` of comment bodies, but never a comment's own `attachments` array, so `--download-attachments` and the TUI asset panel miss them. The TUI also has no way to save a task's attachments. A read-only probe on 2026-11-11 showed each comment carries `attachments` entries with `name`, `download_url`, `mime_type` and `size`; `assets_from_attachments` already reads `download_url`.

### Scope

Included:

- `extract_assets` and `downloadable_assets` also read each comment's `attachments`, deduplicated by URL as today.
- The TUI asset panel lists comment attachments.
- A TUI key, `D` (free today), in the task detail downloads every downloadable asset of the open task to `~/Downloads/ac-attachments/<project_id>-<task_id>/` through the existing `download_task_attachments` (size cap, sanitized names, containment check of ADR 0066); the status line shows "Downloaded N of M attachment(s) to <dir>" or the failure.
- The MCP `ac_download_attachments` tool ([issue 0080](/issues/0080-move-the-mcp-attachment-download-to-its-own-ac-download-attachments-tool.md)) returns comment attachments in `downloaded_attachments`.
- The `active-collab` skill and the TUI footer hint mention the new key.

Excluded: choosing the TUI destination, downloading a single asset from the TUI, uploading attachments.

### Decision

- CLI plus a TUI key, as the owner chose (over the CLI only).
- The TUI writes to `~/Downloads/ac-attachments/<project_id>-<task_id>/`, as the owner chose; the CLI keeps its temp-dir default.
- The download runs off the update core; the core receives its result as a `Msg`.
- Comment attachments come from each comment's own `attachments` array through the existing `assets_from_attachments`, after the task's own attachments, deduplicated by URL in first-seen order as today.
- The pure core builds the asset set with `controller::downloadable_assets(task, comments)` from the open Detail screen and emits one `Cmd` carrying it; the shell resolves the home directory and runs `download_task_attachments` (ADR 0066 safety unchanged). `update` sits at its complexity ceiling, so the new messages join the existing image-viewer family as one asset-effects family instead of a new arm.
- The outcome lives on the Detail screen and shows in its footer status row, below an auth error and above the copied note: downloading, "Downloaded N of M attachment(s) to <dir>", or the failure. `D` while a download runs does nothing; a task with no downloadable asset shows a "no attachments" note and emits no `Cmd`; no home directory is a failure, never a fallback path. The footer row does not wrap, so the shown directory abbreviates the home directory to `~` (`~/Downloads/ac-attachments/<project_id>-<task_id>`), which keeps the whole path visible at 80 columns.

- The live acceptance criterion verifies against the mock ActiveCollab of [issue 0084](/issues/0084-replay-the-live-acceptance-criteria-as-terminal-walkthroughs-against-a-mock-activecollab.md) (owner, 2026-11-12): the owner's `ac` is connected to the real instance, which no check may touch. Risk accepted: the mock returns the JSON shapes `tests/mcp_stdio.rs` mounts, so a real API that diverges from them still passes the walkthrough. Not taken: the mock plus live read-only reads.

### Acceptance

- AC1: a task whose comments carry attachments yields them in `downloadable_assets` and `extract_assets`, once each. (`verify_by: test`)
- AC2: `ac get --download-attachments` writes comment attachments next to task attachments. (`verify_by: test`)
- AC3: `D` in the detail starts the download, and the status line shows the count and directory, or the failure; the transitions live in the pure update core. (`verify_by: test`)
- AC4: against the mock ActiveCollab, a task with a comment attachment downloads it, replayed by a terminal walkthrough. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Plan

- S1: comment attachments join `downloadable_assets` and `extract_assets` (so `ac get --download-attachments`, the TUI asset panel and the MCP `ac_download_attachments` see them) | files: src/controller.rs, tests/unit/controller.rs, tests/unit/commands.rs, .claude/skills/active-collab/SKILL.md | after: none
- S2: `D` in the TUI detail downloads the open task's downloadable assets to `~/Downloads/ac-attachments/<project_id>-<task_id>/` and the footer status row shows the outcome | files: src/tui/model.rs, src/tui/events.rs, src/tui/mod.rs, src/tui/footer.rs, locales/pt_BR.json, tests/unit/model.rs, tests/unit/tui_render.rs, .claude/skills/active-collab/SKILL.md, docs/architecture.md | after: S1 (both edit SKILL.md)
