---
type: ADR
title: ac mcp runs uploads as tasks with per-file progress and a cancel the core honors
description: Uploads run as MCP tasks, report each file as progress, and a task cancel raises a flag the upload core checks so a cancelled upload posts nothing.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-26T17:16:42Z
---

# 0077. ac mcp runs uploads as tasks with per-file progress and a cancel the core honors

## Context

ADR 0076 runs two calls as MCP tasks, `ac_download_attachments` and `ac_write` `time_report`, and treats every command core as uninterruptible: a cancelled task ends at once while its core runs to the end and its result is dropped. Uploads (issue 0107) add `ac_write` `comment` with `attachments` and `ac_write` `attach_task`. An upload of several large files is long, and a comment posted after its task was cancelled is a visible side effect the caller asked not to happen.

## Decision

- `ac_write` `comment` with a non-empty `attachments` and `ac_write` `attach_task` run as tasks when the client declares the tasks extension, beside the two calls of ADR 0076; every other call stays synchronous.
- A long call's core receives an `UploadReporter` (`src/commands/attach.rs`): a channel for step messages and a cancel flag. `Progress::follow` forwards each step as a progress notification (`uploading <i>/<n> <name>`), on the same rising value as the heartbeat, below the total.
- On `tasks/cancel` the server raises the cancel flag before it ends the task. The upload core checks the flag before each file and once after the last, before the comment or task write, so a cancelled upload attaches and posts nothing. Files already uploaded stay on the server as orphan uploads. A call made without the tasks extension raises the same flag on the request's `notifications/cancelled`. A core that does not check the flag (download, hours report) keeps the ADR 0076 behavior.
- This supersedes ADR 0076 only on the list of long calls and on the cancel contract; the skill serving, the task lifetime and the heartbeat stand.

## Alternatives

- Keep the ADR 0076 cancel: simplest, but a cancelled upload can still post the comment.
- Abort the core's thread: not possible for a blocking thread driving `block_on`; it would need every core to be `Send` and spawned.
- Heartbeat only, no per-file step: the client cannot tell a stuck upload from a slow one.

## Consequences

- A cancelled upload never posts; the race left is a cancel that arrives while the final write is already in flight, which completes.
- The `tools/list` size bound rises to 4410 bytes (4374 measured) for the new operation and field.
- Fitness function: `tests/mcp_stdio.rs` `attach_task_runs_as_a_task_and_reports_each_file`, `cancelling_an_upload_task_posts_no_comment` and `cancelling_a_plain_upload_call_posts_no_comment`; `tests/unit/attach.rs` covers the cancel checks at the core.
