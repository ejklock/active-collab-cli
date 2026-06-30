---
type: Issue
title: "Asset hit-target is emitted structurally into the affordance registry; asset_panel_cmd_at becomes a lookup (slice 1)"
description: build_detail_content emits one OpenAsset(asset.url) affordance span per asset content row (every wrapped continuation line of an asset carries the same url), using the layout's own row→idx knowledge. asset_panel_cmd_at stops re-calling section_lines()/asset_index_for_section_row() at click time and becomes a positional lookup over DetailContent.affordances. Observable click behavior (Ctrl/Cmd+click on any asset row opens asset.url) is unchanged. AffordanceKind gains OpenAsset(String) (and OpenUrl(String), used by slice 0046).
status: closed
labels: [tui, render, assets, hit-test, affordance, refactor, slice]
blocked_by:
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## Asset hit-target → structural emission (slice 1 of ADR 0043)

Implements [ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md) decision
steps 1–2 and 4 (asset half). Preserves [BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md)
Scenario 1 (asset opens on Ctrl/Cmd+click) with no observable change.

### Problem

`asset_panel_cmd_at` (`src/tui/model.rs:1384`) re-derives the row→asset map **at click
time** by re-calling `asset_panel::section_lines(assets, width).len()` and
`asset_index_for_section_row(assets, width, interior_row)` — rebuilding what
`build_detail_content` already computed when it spliced the asset rows into `lines`. The
two callers agree only by the shared `inline_content_width` formula
([ADR 0028](/adr/0028-asset-panel-single-layout-source.md)/0029), not by a single emitted
artifact. This is the same seam [ADR 0032](/adr/0032-asset-row-link-style-structural.md)
closed for *style*, still open for the *hit-target*.

### Decision (from ADR 0043)

Emit the asset hit-target on the existing `DetailContent.affordances` registry at layout
time; reduce the click path to a lookup.

### Scope

Included:

- `src/render.rs` — `AffordanceKind` gains `OpenUrl(String)` and `OpenAsset(String)` (the
  `OpenUrl` variant is defined here but **populated** by slice 0046; defining both now keeps
  the enum stable across the two slices). Where `build_detail_content` splices
  `asset_panel::section_lines`, push one `LocalAffordance { kind: OpenAsset(asset.url) }`
  per asset **content** row — wrapped continuation rows of the same asset all carry that
  asset's url; header/separator/pad/hint rows emit no affordance. Reuse the layout's
  `PanelRow::Asset { idx }` / `asset_index_for_section_row` knowledge (do not re-pattern the
  text).
- `src/tui/model.rs` — `asset_panel_cmd_at` becomes a positional lookup over
  `affordances` (filter to `OpenAsset`, gated on Ctrl/Cmd), returning
  `Cmd::OpenAsset { instance, url }`. The click-time `section_lines`/`asset_index_for_section_row`
  re-derivation is removed. `affordance_at` / `dispatch_affordance_click` keep resolving
  `Edit`/`Delete` on a **plain** click (unchanged).
- Tests: `tests/unit/tui_render.rs` (and/or `tests/unit/model.rs`) — buffer/layout-derived:
  the emitted `affordances` carry `OpenAsset(url)` over each asset row incl. a wrapped
  continuation line; Ctrl/Cmd+click on any asset row (incl. continuation) still emits
  `Cmd::OpenAsset` with the right url; a plain click on an asset row does **not** open it;
  header/hint rows carry no `OpenAsset`.

Excluded: body-link emission and the deletion of `resolve_wrapped_url` + inverse-wrap
helpers (slice 0046); collapsing the three hit-tests into one `tui/hit_test` module (ADR
0043 leaves this for later).

### Acceptance

- AC1 — structural emission (`verify_by: test`): `build_detail_content` output carries an
  `OpenAsset(asset.url)` affordance span over every asset content row; a wrapped asset
  (continuation line) carries the **same** url on each fragment; non-asset rows
  (header/separator/pad/hint) carry no `OpenAsset` span.
- AC2 — click preserved end-to-end (`verify_by: test`): a Ctrl/Cmd+click on any asset row,
  including a wrapped continuation line, still emits `Cmd::OpenAsset { url }` for the right
  asset (buffer-derived coords). Existing asset-click specs stay green.
- AC3 — no click-time re-derivation (`verify_by: inspection`): `asset_panel_cmd_at` no
  longer calls `section_lines`/`asset_index_for_section_row`; it resolves via the
  `affordances` lookup only.
- AC4 — plain click reserved (`verify_by: test`): a plain (no Ctrl/Cmd) click on an asset
  row does not emit `Cmd::OpenAsset` (BDR 0014 Sc.8 / V6 selection reserved).
- CC — clean code (named `OpenAsset`; no magic offsets; no banners/commented-out; only
  non-obvious why-comments) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior (emitted payload + rendered-buffer click) and
  survive the mutation floor: swapping the emitted url, or dropping a per-fragment span,
  fails a test (`verify_by: command`).

### Plan

1. `render.rs`: add `OpenUrl(String)` + `OpenAsset(String)` to `AffordanceKind`; update its
   doc comment.
2. `render.rs`: in `build_detail_content`'s asset splice, push `OpenAsset(asset.url)` spans
   per asset content row using the layout row→idx map (one span per wrapped fragment).
3. `model.rs`: rewrite `asset_panel_cmd_at` as a lookup over `affordances` (OpenAsset,
   Ctrl/Cmd-gated); delete the click-time `section_lines`/`asset_index_for_section_row` use.
4. Tests: emitted-payload assertions + buffer-derived Ctrl/Cmd+click (incl. wrapped) +
   plain-click negative + header/hint negative.

Observable end-to-end: unchanged — every attachment still opens on Ctrl/Cmd+click; the
difference is that the click map is now emitted once by the layout, not rebuilt on click.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
