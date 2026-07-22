---
type: ADR
title: "ac get/current --download-attachments: fetch task assets to a local temp dir for agent/LLM analysis"
description: Add an opt-in --download-attachments flag (with --attachments-dir override) to `ac get`/`ac current`. When set, every downloadable task asset — real file attachments plus inline <img> sources from the body/comments — is fetched over the existing host-gated authenticated client seam and written to a local directory (default a per-task path under the OS temp dir), so an agent (e.g. Claude Code via the active-collab skill) can Read the files directly instead of only seeing a remote URL. Excludes arbitrary body hyperlinks (anchor-text links are not necessarily files) from the download set. Filenames are sanitized against path traversal before writing, downloads are size-capped, and one asset's failure never aborts the batch — each asset gets an independent success/error outcome. --json gains an additive downloaded_attachments[] field only when the flag is passed; the undecorated schema is untouched.
status: Accepted
supersedes:
superseded_by:
tags: [cli, attachments, assets, agent, llm, download, security]
timestamp: 2026-07-21T00:00:00Z
---

# 0066. `ac get`/`ac current --download-attachments`: local temp-dir download for agent analysis

## Context

`ac get`/`ac current --json` already exposes `assets: [{name, url}]` (ADR 0011 / the
curated agent contract, `.claude/skills/active-collab/SKILL.md`). An agent (Claude Code,
via the `active-collab` skill) reading a task today only sees the **remote URL** of an
attachment or inline image — it cannot inspect the pixels/content without a separate,
manual fetch, and ActiveCollab attachment URLs require the instance's auth token
(host-gated, ADR 0033/0054), so a naive `curl` from outside the CLI's own seam won't work.

The TUI already has a use for the same bytes: [ADR 0065](/adr/0065-image-attachment-viewer-modal-overlay.md)
fetches image attachments over this same authenticated seam to render them in-terminal
(issue 0059, not yet implemented — `Cmd::LoadImage` is still a no-op). This ADR does **not**
build that: it is a separate, CLI-facing capability — the equivalent bytes, written to disk
instead of decoded into a terminal graphics protocol, so a multimodal LLM agent driving the
CLI headlessly can `Read` the file itself (an image, a PDF, a text export, …) as part of its
own analysis, without a terminal graphics protocol at all.

Two things make this more than "just fetch and write a file":

1. **Attachment metadata is not trustworthy input.** An asset's `name` comes from the
   ActiveCollab payload (`attachments[].name`, or a derived label/basename — `controller.rs`
   `derive_asset_label`/`url_basename`) — effectively attacker-influenced content (anyone who
   can attach a file or paste a body link to a project you're a member of controls it). Using
   it verbatim as a filename is a path-traversal vector (`../../etc/passwd`,
   `..\..\..\whatever`).
2. **Not every "asset" is a file worth downloading.** `extract_assets` (used for the existing
   `assets` JSON field) unions three sources: `task.attachments`, inline `<img src>`, and
   `<a href>…</a>` hyperlinks with anchor text found in the body/comments HTML. The third
   source is an arbitrary URL a comment author chose to link to (another task, a Google Doc,
   an external site) — not a file ActiveCollab is hosting. Fetching those as if they were
   attachments would write garbage (an HTML page) to disk under a misleading name, and turns
   the flag into an open fetch-arbitrary-URL primitive.

## Decision

Add an **opt-in** `--download-attachments` flag (plus `--attachments-dir <DIR>` to override
the destination) to `ac get` and `ac current` (both already share `DisplayArgs`). Nothing
changes for callers who don't pass it.

### 1. Downloadable-asset set — narrower than `assets`

A new pure extraction function, `controller::downloadable_assets(task, comments) -> Vec<Asset>`,
returns only:

- entries from `task.attachments` (`assets_from_attachments`), and
- inline `<img src>` sources in the task body / comment bodies,

deduplicated by URL (same `Asset { name, url }` shape as `extract_assets`). It **excludes**
`href`-with-anchor-text hyperlinks — those stay visible in the existing `assets` JSON field
for reference, but are never fetched as files. (`assets_from_html`'s `<img>`-capture loop is
factored into a small shared helper so this doesn't duplicate the regex walk already used by
`extract_assets`.)

### 2. Fetch over the existing authenticated seam

A new `ActiveCollabClient::fetch_asset_bytes(&self, url: &str) -> Result<(u16, bytes::Bytes)>`
is a thin wrapper over `Http::authed_get`, reusing the **already-host-gated** token
attachment (ADR 0033/0054: the token is attached only when the request URL's host matches
the configured instance host). No new auth logic — the same guard that protects the ADR 0065
image fetch protects this one.

### 3. Sanitize before writing — path traversal is a first-class threat

`controller::sanitize_attachment_filename(name, fallback_index) -> String` is a **pure**
function: it keeps only the final path component of `name` (rejecting embedded `/`, `\`, and
any resulting `.`/`..`/empty component), and falls back to `asset_{fallback_index}` (with the
original extension preserved when present) when sanitization empties the name. On-disk name
collisions within one destination directory are resolved by suffixing `_2`, `_3`, ….

As defense in depth, the write path itself is checked after joining
`dest_dir.join(sanitized_name)`: the resulting path must canonicalize to a location **inside**
`dest_dir`. Any escape (a sanitizer gap, a symlink trick) is treated as a per-asset error, not
written, and never aborts the rest of the batch.

### 4. One asset's failure never aborts the batch

`controller::download_task_attachments(client, assets, dest_dir) -> Vec<DownloadedAsset>`
(`DownloadedAsset { name, url, path: Option<String>, error: Option<String> }`) downloads
assets independently: a 404, a host-gate rejection, an oversized body (`MAX_ATTACHMENT_BYTES`,
a fixed cap — this ADR does not add streaming/Content-Length gating), or an I/O error on one
asset produces an `error` entry for that asset only; every other asset still gets its own
outcome. `dest_dir` is created (`create_dir_all`) once, up front.

### 5. Destination directory

Default: `std::env::temp_dir().join("ac-attachments").join(format!("{project_id}-{task_id}"))`
— a stable, per-task path so repeated runs land in the same place (and an agent can predict
it without parsing output). `--attachments-dir <DIR>` overrides it outright.

### 6. Output shaping — additive, not a schema break

- `--json`: the existing `task_object` (pure, `agent_json.rs`) is unchanged; `do_get_task`
  splices `"downloaded_attachments": [{"name","url","path","error"}, …]` into the built
  `Value` **only when `--download-attachments` was passed**. Omitted otherwise — the existing
  undecorated schema-lock test (`tests/unit/agent_json.rs`) is untouched.
- Human (non-JSON, non-`--short`) output: one summary line — counts and, for any failures, the
  asset name + reason (i18n'd via `t()`, `presenter.rs` convention).
- `--short`: unaffected — the download still runs (side effect only), but the one-line
  `PROJECT/TASK<TAB>name` contract stays exactly as-is.

## Alternatives considered

- **Download everything `extract_assets` returns (including anchor hyperlinks).** Rejected —
  turns the flag into an arbitrary-URL fetcher; an anchor link is not necessarily a file, and
  ActiveCollab doesn't attach a token-gated host guarantee to a link some other user pasted
  into a comment.
- **A separate `ac attachments download <ref>` subcommand instead of a flag.** Rejected for
  this slice — an agent already calls `get`/`current` to read the task; a flag on the same
  call avoids a second round trip and keeps the ADR 0011 read contract as the single entry
  point. Nothing here blocks adding a dedicated subcommand later if a standalone use case
  shows up.
- **Reuse `Cmd::LoadImage`'s (future) fetch path from issue 0059.** Rejected — that path is
  TUI-shell-owned, decodes into a `ratatui-image` protocol, and doesn't exist yet (still a
  no-op). This ADR's fetch is CLI-side, writes raw bytes to disk, and has no rendering
  concern; sharing `fetch_asset_bytes` at the `ActiveCollabClient` layer is enough reuse
  between the two features without coupling them.
- **Trust `attachments[].name` verbatim.** Rejected — untrusted content used as a filesystem
  path is a textbook path-traversal vector; sanitize-then-verify-containment is the standard
  mitigation and costs one small pure function plus a canonicalization check.
- **Abort the whole download on the first failed asset.** Rejected — a single broken/expired
  attachment link (common in older tasks) would make the flag useless for every other asset
  on the same task; per-asset outcomes keep it useful under partial failure.

## Consequences

**Easier / gained:**
- An agent driving `ac` can `Read` a task's images/attachments directly from a predictable
  local path instead of only ever seeing a remote URL it cannot fetch without the CLI's own
  auth seam.
- The narrower "downloadable" set keeps the feature from becoming a generic URL-fetch
  primitive; the existing `assets` field is untouched for reference.
- Partial-failure-tolerant: one dead attachment link never blocks the rest.

**Harder / accepted trade-offs:**
- A second, narrower asset-extraction function (`downloadable_assets`) alongside
  `extract_assets` — deliberate (different trust/purpose boundary), not accidental
  duplication; the `<img>`-capture loop is shared to avoid regex duplication.
- A fixed size cap (no streaming/Content-Length short-circuit) — acceptable for the current
  `Http` seam; revisit if real attachments regularly exceed it.
- One more opt-in flag pair on an already-flag-heavy `DisplayArgs` — mitigated by keeping the
  default path fully automatic (no required argument).

**Follow-ups:**
- Issue 0061 implements this slice end-to-end (client seam, extraction, sanitize/write,
  CLI flags, output shaping).
- `.claude/skills/active-collab/SKILL.md` gains a `--download-attachments` section in the
  same change (maintenance rule).

## Verification

**Implementation impact:** `src/client.rs` (`fetch_asset_bytes`), `src/controller.rs`
(`downloadable_assets`, `sanitize_attachment_filename`, `DownloadedAsset`,
`download_task_attachments`, default-dir helper), `src/commands/task.rs` (`DisplayFlags`
fields, orchestration, output splice), `src/cli.rs` (`--download-attachments`,
`--attachments-dir`), `src/main.rs` (flag threading for `get`/`current`).

**Verification criteria:**
- `sanitize_attachment_filename` rejects traversal/empty/dot-only names and falls back to a
  generated name; the final write path is proven to stay inside `dest_dir` (unit).
- `downloadable_assets` returns attachments + inline images only, never anchor-hyperlink
  assets, deduped by URL (unit).
- `fetch_asset_bytes` attaches the token only on a same-host URL (host-gate reuse, unit).
- `download_task_attachments` yields an independent outcome per asset; one failure doesn't
  abort the batch; oversized bodies are rejected without a partial write (unit).
- `--json` adds `downloaded_attachments` only under the flag; the undecorated schema test is
  unaffected (unit).

## Related

- ADR: [/adr/0011-agent-json-output-contract.md](/adr/0011-agent-json-output-contract.md) (the `--json` contract this extends additively)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md), [/adr/0054-comment-write-outcome-typed-classification.md](/adr/0054-comment-write-outcome-typed-classification.md) (the host-gated authenticated seam reused for the fetch)
- ADR: [/adr/0065-image-attachment-viewer-modal-overlay.md](/adr/0065-image-attachment-viewer-modal-overlay.md) (the TUI-side fetch-and-render use of the same attachment bytes; separate concern, shares the client seam)
- ADR: [/adr/0023-asset-label-derivation.md](/adr/0023-asset-label-derivation.md) (the label/basename derivation `sanitize_attachment_filename` must not trust verbatim)
- Issue: [/issues/0061-download-attachments-flag.md](/issues/0061-download-attachments-flag.md)
</content>
