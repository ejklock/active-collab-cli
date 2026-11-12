---
type: Issue
title: Download comment attachments in the CLI and from a TUI key
description: Comment attachments join the downloadable and listed assets, and a TUI key D saves a task's attachments to ~/Downloads/ac-attachments.
status: open
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

### Acceptance

- AC1: a task whose comments carry attachments yields them in `downloadable_assets` and `extract_assets`, once each. (`verify_by: test`)
- AC2: `ac get --download-attachments` writes comment attachments next to task attachments. (`verify_by: test`)
- AC3: `D` in the detail starts the download, and the status line shows the count and directory, or the failure; the transitions live in the pure update core. (`verify_by: test`)
- AC4: against collab.base.digital, a task with a comment attachment downloads it. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
