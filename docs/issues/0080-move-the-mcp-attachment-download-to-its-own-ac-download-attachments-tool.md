---
type: Issue
title: Move the MCP attachment download to its own ac_download_attachments tool
description: ac mcp serves the attachment download as a third tool with honest annotations, and ac_read get no longer downloads.
status: closed
labels: [agent, mcp]
timestamp: 2026-11-12T00:34:15Z
---

## 0080. Move the MCP attachment download to its own ac_download_attachments tool

### Problem

`ac_read` carries `readOnlyHint: true` (src/mcp/mod.rs:51), but its `get` operation with `download_attachments` writes files to a caller-chosen `attachments_dir`. MCP annotations belong to a tool, so a harness that trusts the hint writes files without asking. [ADR 0072](/adr/0072-ac-mcp-exposes-downloads-as-a-third-tool-with-honest-annotations.md) moves the download to its own tool.

### Scope

Included:

- A third tool, `ac_download_attachments`, with `readOnlyHint: false`, `destructiveHint: false`, `idempotentHint: true`; parameters `ref` (required), `attachments_dir` and `instance` (optional); it returns what `ac get <ref> --download-attachments --json` prints.
- `ac_read` `get` drops `download_attachments` and `attachments_dir`; passing them is an `invalid_params` error.
- The `active-collab` skill, `README.md` and `docs/architecture.md` show three tools.

Excluded: comment attachments ([issue 0076](/issues/0076-download-comment-attachments-in-the-cli-and-from-a-tui-key.md)), the CLI flags, which stay unchanged.

### Decision

- A tool of its own, as the owner chose (over an operation in `ac_read` or in `ac_write`); see ADR 0072.
- Default destination as the CLI's: the per-task temp dir when `attachments_dir` is absent.

### Acceptance

- AC1: `tools/list` returns exactly `ac_read`, `ac_write` and `ac_download_attachments`, each with the hints of ADR 0072. (`verify_by: test`)
- AC2: `ac_download_attachments` returns bytes identical to `ac get <ref> --download-attachments --attachments-dir <dir> --json` against the same mocked API, and writes the same files. (`verify_by: test`)
- AC3: `ac_read` `get` with `download_attachments` or `attachments_dir` is `invalid_params`; a missing `ref` on the new tool is `invalid_params`. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
