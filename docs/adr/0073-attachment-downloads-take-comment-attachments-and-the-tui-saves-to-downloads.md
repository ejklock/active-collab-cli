---
type: ADR
title: Attachment downloads take comment attachments and the TUI saves to Downloads
description: "One download set for the CLI, MCP and TUI: task and comment attachments plus inline images, never anchor links, with the ADR 0066 safety rules; agents default to the OS temp dir and the TUI saves to ~/Downloads."
owner: Evaldo Klock
status: Accepted
supersedes: 0066
timestamp: 2026-11-12T11:47:37Z
---

# 0073. Attachment downloads take comment attachments and the TUI saves to Downloads

## Context

[ADR 0066](/adr/0066-agent-attachment-download-to-local-temp-dir.md) added `--download-attachments` to `ac get` and `ac current`: an agent reads a task's files from a predictable local path instead of a remote URL that needs the instance token. Its download set was the task's `attachments` plus inline `<img src>` sources in the body and comments, never anchor hyperlinks.

[Issue 0076](/issues/0076-download-comment-attachments-in-the-cli-and-from-a-tui-key.md) changed two parts of that decision:

- A file attached to a comment is a file ActiveCollab hosts, like a task attachment, but the 0066 set left it out, so an agent never saw it. Commit 5322585 adds each comment's `attachments` to both the downloadable set and the `assets` field.
- A person in the TUI wants the same files in a folder they browse, not under the OS temp dir. Commit d84ad5d adds the `D` key in the task detail, saving to `~/Downloads`.

[ADR 0072](/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md) exposes the same download to MCP clients as `ac_download_attachments`; it settles the MCP tool shape, not the download set.

## Decision

We will download, for one task, the set `controller::downloadable_assets(task, comments)` returns, the same set from every entry point (`ac get`/`ac current --download-attachments`, the MCP tool `ac_download_attachments`, the TUI key `D`):

- the task's `attachments`, then each comment's `attachments` in comment order;
- inline `<img src>` sources in the task body and the comment bodies;
- deduplicated by URL, first seen first; never anchor-hyperlink assets, which stay in `assets` for reference only.

Every download keeps the safety rules of ADR 0066, unchanged:

- the fetch goes through `ActiveCollabClient::fetch_asset_bytes`, which attaches the token only when the URL's origin (scheme, host and port) equals the instance's ([ADR 0067](/adr/0067-origin-gated-api-token-header-scheme-host-port.md));
- `controller::sanitize_attachment_filename` keeps only the final path component, falls back to `asset_<n>`, and suffixes collisions `_2`, `_3`; the joined path must stay inside the destination after canonicalization;
- each asset has its own outcome (`path` or `error`); one failure never aborts the batch; a body over `MAX_ATTACHMENT_BYTES` (25 MiB) is rejected without a partial write.

The destination depends on the caller:

- CLI and MCP: `--attachments-dir` (MCP `attachments_dir`), or by default `<OS temp dir>/ac-attachments/<project_id>-<task_id>/`, a path an agent predicts without parsing output.
- TUI: `~/Downloads/ac-attachments/<project_id>-<task_id>/`. No home directory means no destination: the status line reports the failure, never a fallback path. The status line shows the home directory as `~`.

The `--json` output shape of ADR 0066 holds: `downloaded_attachments` appears only when a download was asked for.

Rejected alternatives:

- **Keep comment attachments out of the download set.** An agent sees a comment's file in `assets` but cannot read it; comment attachments carry the same host guarantee as task attachments.
- **Add anchor hyperlinks with the comment attachments.** Still an arbitrary-URL fetch, the reason ADR 0066 rejected it.
- **TUI saves to the OS temp dir, as the CLI does.** A person browsing files does not look there, and the OS may clear it.
- **TUI falls back to the temp dir without a home directory.** The user would be told files were saved where they do not look; a reported failure is honest.

## Consequences

**Easier / gained:**
- One download set for the CLI, MCP and TUI: a file visible in a task's `assets` as an attachment is downloadable from every entry point.
- Comment attachments reach agents through `ac get --download-attachments` and `ac_download_attachments`.

**Harder / accepted trade-offs:**
- A task with many commented files downloads more, still capped per asset, not per task.
- Two default destinations (temp dir for agents, `~/Downloads` for people); each is fixed per caller, and the CLI keeps `--attachments-dir`.

## Verification

**Implementation impact:** `src/controller.rs` (`attachment_assets`, `downloadable_assets`, `extract_assets`, `default_attachments_dir`, `download_task_attachments`), `src/tui/mod.rs` (`attachments_destination`, `download_task_outcome`), `src/mcp/mod.rs` (`ac_download_attachments`).

**Verification criteria:**
- `downloadable_assets` returns task attachments, comment attachments and inline images, deduplicated by URL, and no anchor-hyperlink asset (unit, `tests/unit/controller.rs`).
- A traversal name is sanitized and the write path stays inside the destination (unit).
- `attachments_destination` returns `~/Downloads/ac-attachments/<project_id>-<task_id>` with a home directory and nothing without one (unit, `tests/unit/model.rs`).
- `ac_download_attachments` output equals `ac get --download-attachments --json` (`tests/mcp_stdio.rs`).

## Related

- ADR: [/adr/0066-agent-attachment-download-to-local-temp-dir.md](/adr/0066-agent-attachment-download-to-local-temp-dir.md) (superseded by this record)
- ADR: [/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md](/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md) (the MCP download tool)
- ADR: [/adr/0011-agent-json-output-contract.md](/adr/0011-agent-json-output-contract.md), [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md), [/adr/0054-comment-write-outcome-typed-classification.md](/adr/0054-comment-write-outcome-typed-classification.md)
- Issue: [/issues/0076-download-comment-attachments-in-the-cli-and-from-a-tui-key.md](/issues/0076-download-comment-attachments-in-the-cli-and-from-a-tui-key.md)
