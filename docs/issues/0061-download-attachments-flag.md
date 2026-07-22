---
type: Issue
title: "ac get/current --download-attachments — fetch task assets to a local temp dir for agent analysis"
description: Implement ADR 0066 end-to-end. Add ActiveCollabClient::fetch_asset_bytes (host-gated seam reuse). Add controller::downloadable_assets (attachments + inline <img> only, excludes anchor hyperlinks), controller::sanitize_attachment_filename (pure, path-traversal-safe), controller::download_task_attachments (per-asset outcome, size-capped, containment-checked writes). Wire --download-attachments / --attachments-dir into DisplayArgs, DisplayFlags, and do_get_task's json/human output paths for both get and current. Update the active-collab skill doc in the same change.
status: open
labels: [cli, attachments, agent, security, slice]
blocked_by:
tracker:
timestamp: 2026-07-21T00:00:00Z
---

## `--download-attachments` — local temp-dir download for agent analysis

Implements [ADR 0066](/adr/0066-agent-attachment-download-to-local-temp-dir.md).

### Problem

`ac get`/`ac current --json` exposes attachment/image `assets: [{name,url}]`, but an agent
reading a task cannot inspect the actual content (screenshot, PDF, …) — the URL requires the
CLI's own host-gated auth token to fetch, and there is no path from "I see the URL" to "I can
`Read` the file."

### Decision (from ADR 0066)

- **Downloadable set** (`controller::downloadable_assets`): `task.attachments` entries +
  inline `<img src>` from body/comments only. Anchor-hyperlink assets (`href_with_text_re`)
  are excluded — they are not necessarily files.
- **Fetch** (`ActiveCollabClient::fetch_asset_bytes`): thin wrapper over the existing
  host-gated `Http::authed_get` — no new auth logic.
- **Sanitize + write** (`controller::sanitize_attachment_filename`,
  `controller::download_task_attachments`): strip directory components from the untrusted
  `name`, reject empty/`.`/`..`, fall back to `asset_{n}` (preserving extension) on an empty
  result; collision-suffix on-disk (`_2`, `_3`, …); enforce a `MAX_ATTACHMENT_BYTES` cap;
  verify the final write path canonicalizes inside `dest_dir` before writing. Each asset gets
  an independent `DownloadedAsset { name, url, path: Option<String>, error: Option<String> }`
  outcome — one failure never aborts the batch.
- **Default destination:** `std::env::temp_dir().join("ac-attachments").join(format!("{pid}-{tid}"))`;
  `--attachments-dir <DIR>` overrides it.
- **Output:** `--json` splices `downloaded_attachments: [...]` into the existing task object
  only when the flag is passed (existing undecorated schema test untouched); human output
  (non-`--short`) prints one summary line; `--short` still runs the download (side effect
  only, no extra print).

### Scope

Included:

- `src/client.rs` — `fetch_asset_bytes`.
- `src/controller.rs` — `downloadable_assets`, `sanitize_attachment_filename`,
  `DownloadedAsset`, `download_task_attachments`, default-dir helper; refactor the `<img>`
  capture loop out of `assets_from_html` into a shared helper (no behavior change to
  `extract_assets`).
- `src/commands/task.rs` — `DisplayFlags` gains `download_attachments`/`attachments_dir`;
  `do_get_task` runs the download after a successful `load_task` and splices/prints the
  result.
- `src/cli.rs` — `--download-attachments` (bool), `--attachments-dir <DIR>` (Option<String>)
  on `DisplayArgs`.
- `src/main.rs` — thread the two new fields into `DisplayFlags` in `dispatch_get` and
  `dispatch_current`.
- `.claude/skills/active-collab/SKILL.md` — document the new flag pair and the
  `downloaded_attachments` JSON field (maintenance rule).
- Tests: extend `tests/unit/client.rs`, `tests/unit/controller.rs`, `tests/unit/cli.rs`,
  `tests/unit/commands.rs`.

Excluded: a standalone `ac attachments download` subcommand; streaming/Content-Length-based
size gating; downloading anchor-hyperlink assets; any change to the TUI image viewer
(issue 0059, separate feature, separate fetch path).

### Acceptance

- AC1 — path safety (unit): `sanitize_attachment_filename` strips directory components and
  rejects empty/`.`/`..`, falling back to a generated name; `default_attachments_dir` derives
  `{temp}/ac-attachments/{pid}-{tid}`; the final write path is proven to canonicalize inside
  `dest_dir` before any write (an escape attempt is recorded as a per-asset error, never
  written outside it). (`verify_by: test`)
- AC2 — downloadable set (unit): `downloadable_assets` returns `task.attachments` + inline
  `<img>` sources only, deduped by URL; an anchor-hyperlink-only body/comment yields no
  downloadable assets. (`verify_by: test`)
- AC3 — authenticated fetch (unit): `fetch_asset_bytes` attaches the token only when the
  asset URL's host matches the instance host (host-gate reuse); a foreign-host URL gets no
  token attached. (`verify_by: test`)
- AC4 — resilient batch download (unit): `download_task_attachments` returns one
  `DownloadedAsset` outcome per input asset; a non-200 fetch, an oversized body
  (`MAX_ATTACHMENT_BYTES`), and an I/O failure each produce an `error` entry without aborting
  the remaining assets; a successful asset's `path` points inside `dest_dir`.
  (`verify_by: test`)
- AC5 — CLI + output wiring (unit): `--download-attachments`/`--attachments-dir` parse on
  both `get` and `current` and thread into `DisplayFlags`; `--json` output includes
  `downloaded_attachments` only when the flag is passed (the existing undecorated
  `agent_json` schema-lock test stays green unmodified); non-JSON/non-`--short` output prints
  exactly one summary line reflecting success/failure counts. (`verify_by: test`)
- CC — clean code: no superfluous comments / banners / commented-out code. (`verify_by: inspection`)
- CX — complexity budget: cyclomatic ≤ 10 (≤ 8 new), cognitive ≤ threshold. (`verify_by: command`)
- TE — tests assert observable behavior (sanitized names, containment, extracted asset sets,
  host-gate outcome, per-asset batch outcomes, additive JSON field) and survive the mutation
  floor. (`verify_by: command`)

### Plan

1. `client.rs`: add `fetch_asset_bytes` (wraps `authed_get`, no new auth logic).
2. `controller.rs`: add `sanitize_attachment_filename`, `default_attachments_dir`,
   `downloadable_assets` (factor the `<img>` loop out of `assets_from_html` into a shared
   helper), `DownloadedAsset`, `download_task_attachments` (create_dir_all once, per-asset
   fetch → sanitize → collision-suffix → size-check → containment-check → write → outcome).
3. `cli.rs`: add the two flags to `DisplayArgs`.
4. `main.rs`: thread the two new fields into both `DisplayFlags` construction sites.
5. `task.rs`: extend `DisplayFlags`; in `do_get_task`, after a successful `load_task`, run the
   download when requested; splice `downloaded_attachments` into the JSON branch; print one
   summary line in the human branch; leave `--short` silent but still download.
6. `.claude/skills/active-collab/SKILL.md`: document the flags + the additive JSON field.
7. Tests: extend `tests/unit/client.rs`, `tests/unit/controller.rs`, `tests/unit/cli.rs`,
   `tests/unit/commands.rs` per the acceptance criteria above.

Observable end-to-end: `ac get 665/75159 --download-attachments --json` writes every
attachment/inline-image asset to `/tmp/ac-attachments/665-75159/` (or `--attachments-dir`),
and the printed JSON's `downloaded_attachments[]` lists each `{name,url,path,error}` outcome —
an agent can then `Read` the local `path` directly.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
</content>
