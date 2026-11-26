---
type: Issue
title: Attach local files to comments and tasks from the CLI, MCP and TUI
description: Upload local files as attachments to a new comment or directly to a task from ac comment, ac_write, the TUI compose and a task command.
status: in-progress
timestamp: 2026-11-26T16:42:32Z
---

## 0107. Attach local files to comments and tasks from the CLI, MCP and TUI

### Problem

An agent or a person who posts a QA report on a task through `ac comment` or the MCP `ac_write` tool cannot attach the report file or its screenshots: `create_comment` (`src/client.rs`) sends only `{"body": …}`, and no upload call exists in `src/`. Issue 0076 excluded uploads on purpose; the owner drags each file into the browser by hand.

### Scope

Included: attaching local files to a new comment from `ac comment --attach` and `ac_write` comment, directly to a task from `ac task attach` and `ac_write` `attach_task`, and from a path field in the TUI comment compose; MCP task and per-file progress support for the upload calls.

Excluded: attaching files when editing an existing comment, deleting or replacing an attachment, a client-side size limit, a file picker dialog.

### Decision


- Surfaces: CLI `ac comment`, MCP `ac_write` comment, the TUI comment compose, and attaching a file directly to a task (owner, 2026-11-26). Not taken: CLI + MCP only; CLI + MCP + TUI; CLI + MCP + task.
- The CLI takes a repeatable `--attach <path>`; the MCP takes `attachments: [path]`. Not taken: one comma-separated list.
- All or nothing: when any file fails to upload, no comment is posted and the failing file is named. Not taken: posting with the files that uploaded and listing the failures. Known cost: files already uploaded stay orphaned on the server.
- No client-side size limit; the server refuses what it refuses. Not taken: 25 MB, matching `MAX_ATTACHMENT_BYTES` of the download.
- A file goes directly on a task through its own verb, `ac task attach <ref> --attach <path>…`, and the MCP `ac_write` operation `attach_task`. Not taken: `--attach` on `ac task set`.
- A comment body is optional when at least one `--attach` is given; with no file the `no comment body` guard stays. Not taken: body always required. Risk: the ActiveCollab API may reject an empty body (inference, to verify live).
- The MCP uploads any path the user can read. Not taken: only paths under the MCP server cwd. Known cost: a prompt-injected task text can steer an agent into uploading a local secret to the server.
- The TUI compose gets a key that opens a path field; the chosen files list under the text. Not taken: an `@path` syntax inside the body.
- Over MCP, `ac_write` `comment` with `attachments` and `attach_task` run as tasks when the client declares the tasks extension, beside `ac_download_attachments` and `time_report` (owner, 2026-11-26: "não esquecer das tasks MCP e eventos"). This widens the long-call list of ADR 0076 and takes a new ADR that supersedes it on that point.
- Upload progress is per file: a notification `uploading <i>/<n> <name>` as each file starts, the existing heartbeat between them, and the final one at the total. Not taken: the heartbeat alone.
- `tasks/cancel` is honored between files and before the post: a cancelled upload posts nothing; files already uploaded stay orphaned. Not taken: today's behavior, where the core runs to the end and its result is discarded, so the comment could still appear.
- The upload contract (`POST /api/v1/upload-files`, then `attach_uploaded_files` on the comment or task) is proved by a live post on a task the owner names: one comment with two files, and one file attached to the task. Not taken: mock tests alone.

- The live proof runs on task 694/81989 (owner, 2026-11-26). Not taken: 694/74434.
- A failed send no longer strands the TUI compose: from the error state a keystroke, a path edit or a new send returns it to editing, for any failure, network included (owner, 2026-11-26). Not taken: a separate issue.
- The review's smaller items are fixed in this change (owner, 2026-11-26), not in a follow-up: `attach_task` without files is refused as `invalid_params` before any task starts; a file streams from disk instead of being read whole into memory; the TUI path field accepts AltGr characters; a leading `~/` expands to the home directory on every surface; a `notifications/cancelled` on a call made without the tasks extension also stops an upload before its write.

### Plan

- S1: `ac comment --attach` uploads and attaches, all or nothing | files: src/commands/attach.rs, src/commands/comment.rs, src/client.rs, src/http.rs, src/cli.rs, src/main.rs | after: none
- S2: `ac task attach` | files: src/commands/attach.rs, src/cli.rs, src/main.rs | after: S1
- S3: MCP `comment` `attachments`, `attach_task`, task + per-file progress + cancel | files: src/mcp/mod.rs | after: S1, S2
- S4: TUI compose Ctrl+O path field | files: src/tui/model.rs, src/tui/events.rs, src/tui/mod.rs, src/tui/view.rs | after: S1

### Checkpoint

- 2026-11-26: S1–S4 implemented by the session directly (owner: "faça você direto"). Reviewer round 1: changes requested; all findings fixed in this change per the owner's answers (see Decision). Reviewer round 2: approve; its should-fix (call-site test for `~/`) and nits fixed.
- Live proof on 694/81989 with the release build: `ac comment 694/81989 -m … --attach relatorio-teste.html --attach nota-teste.txt --json` returned ok; `ac get 694/81989 --json --refresh` lists both files (attachment ids 63275, 63276; sizes 55 and 23 bytes match the local files). `ac task attach 694/81989 --attach nota-teste.txt --json` returned ok and a new attachment (id 63277) appears. This proves the multipart field `attachment_1` and both write paths. Not proven live: a comment with an empty body (it would post one more test comment).
- Gates: fmt, clippy --all-targets, 2147 tests single-threaded, comment_policy, release build pass. se-gates: quality (arborist, jscpd) pass, arch pass, security pass (3 pre-existing cargo-audit advisories); with the coverage receipt below, se-gates check passes (verdict pass).
- Upload request timeout is 600 s (cheap decision: no client size limit, so the 30 s default would cut large files).
- Owner authorized (2026-11-26) rewriting the Docker `SF:/app/` paths in the local, untracked `coverage/lcov.info` after `cargo llvm-cov`. What works: repository-relative paths (`sed -i 's#^SF:/app/#SF:#' coverage/lcov.info`), then `se-gates qg coverage-receipt`, then `se-gates check` → verdict pass with real coverage. Absolute host paths map to 0% everywhere. Not taken: shipping with the skip; a separate issue.

### Acceptance

- `ac comment <ref> --attach a --attach b` uploads both files and posts one comment carrying both: `tests/unit/attach.rs` `comment_with_two_files_uploads_each_then_posts_their_codes_in_order`.
- Any unreadable path, rejected upload, missing code or 401 posts nothing and names the file: `tests/unit/attach.rs` failure cases.
- `ac task attach <ref> --attach a` attaches to the task; without a file it exits 2 (CLI) or is `invalid_params` (MCP).
- MCP uploads run as tasks, report `uploading <i>/<n> <name>`, and any cancel (task or request) posts nothing: `tests/mcp_stdio.rs`.
- The TUI compose adds files with Ctrl+O, sends them, and keeps the text after a failed send: `tests/unit/model.rs`, `tests/unit/tui_comment_write.rs`, `tests/unit/tui_render.rs`.
- Live proof on task 694/81989: one comment with two files and one file attached to the task, both visible in ActiveCollab.
