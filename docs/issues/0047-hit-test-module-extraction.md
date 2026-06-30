---
type: Issue
title: "Detail click resolution becomes one deep tui/hit_test module (resolve_detail_click → DetailClickTarget); the five scattered click functions are deleted"
description: Extract affordance_at, body_link_cmd_at, asset_panel_cmd_at, click_to_line_col, and dispatch_affordance_click from src/tui/model.rs into a deep, pure src/tui/hit_test.rs. Its single entry resolve_detail_click maps a Ctrl/Cmd+click to a typed DetailClickTarget via one viewport→line_idx translation (with the lines.len() guard for every kind) and one positional lookup over DetailContent.affordances; handle_click_detail maps the typed target to the TEA effect. Behavior (modifier policy + every existing click spec) is unchanged.
status: closed
labels: [tui, hit-test, affordance, refactor, locality, slice]
blocked_by:
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## Detail click resolution → one deep `tui/hit_test` module (ADR 0044)

### Problem

After [ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md), detail click
resolution is correct but **shallow and scattered** across five functions in the 2100-line
`src/tui/model.rs`:

- The viewport→`line_idx` translation is written **three times** — inline in `affordance_at`
  and `asset_panel_cmd_at`, extracted in `click_to_line_col` but used by only
  `body_link_cmd_at`. `affordance_at`'s copy is even **missing** the `line_idx >= lines.len()`
  guard the other two have.
- The Ctrl/Cmd modifier gate is checked in `handle_click_detail` **and** re-checked inside
  `body_link_cmd_at` and `asset_panel_cmd_at`.
- The "scan `affordances`, unwrap the payload" shape is repeated three times.
- `dispatch_affordance_click` carries a dead `OpenAsset`/`OpenUrl` arm.

### Decision (ADR 0044)

Collapse the five functions into a deep, pure **`src/tui/hit_test.rs`** with one entry that
returns a typed target; the model maps target → effect.

### Scope

- `src/tui/hit_test.rs` (new): `pub enum DetailClickTarget { CommentEdit(i64),
  CommentDelete(i64), OpenUrl(String), OpenAsset(String) }` and `pub fn
  resolve_detail_click(model: &Model, column: u16, row: u16, has_modifier: bool) ->
  Option<DetailClickTarget>` — one private coordinate-translation helper (the former
  `click_to_line_col`, with the `lines.len()` guard for **all** kinds) + one positional
  lookup. Pure: no `Model` mutation, no `Cmd`, no I/O.
- `src/tui/mod.rs`: declare `mod hit_test;`.
- `src/tui/model.rs`: rewrite `handle_click_detail` to call `resolve_detail_click` once and
  map the typed target to the effect (`CommentEdit`/`CommentDelete` →
  `handle_edit/delete_comment_request`; `OpenUrl`/`OpenAsset` → `Cmd::OpenAsset { instance,
  url }`). **Delete** `affordance_at`, `body_link_cmd_at`, `asset_panel_cmd_at`,
  `click_to_line_col`, `dispatch_affordance_click`. Add a named `AffordanceKind::is_row_target`
  predicate (true for `OpenAsset`) consumed by the lookup, so the whole-row asset rule is
  explicit in one place.
- `tests/unit/model.rs`: keep the existing buffer-derived click specs green; relocate/adapt
  the hit-test unit tests to drive `resolve_detail_click` directly (interface-as-test-surface),
  including the row-target asset rule and the out-of-range guard.

### Out of scope

- Emitting `OpenAsset` as a full-row span at the `render.rs` layout site (the alternative in
  ADR 0044) — deferred; `render.rs` is not touched by this slice.
- Any change to the modifier policy, the emitted affordances, or the asset/body/comment
  click behavior. This is a pure restructuring.

### Acceptance criteria

- **AC1** (behavior, test): a Ctrl/Cmd+click resolves to the correct `DetailClickTarget` for
  every kind — comment `[editar]`→`CommentEdit(id)`, `[excluir]`→`CommentDelete(id)`, a body
  link (on **any** wrapped fragment)→`OpenUrl(complete_url)`, an asset row (on **any** row,
  including a wrapped continuation)→`OpenAsset(asset.url)` — asserted on `resolve_detail_click`
  driven from the real `build_detail_content` output, and the existing buffer-derived
  `handle_click_detail` click specs stay green.
- **AC2** (behavior, test): a plain (no Ctrl/Cmd) click resolves to no target — body-area
  click starts a text selection, outside-body click clears it — unchanged from today.
- **AC3** (constraint, inspection): `affordance_at`, `body_link_cmd_at`, `asset_panel_cmd_at`,
  `click_to_line_col`, and `dispatch_affordance_click` no longer exist in `src/tui/model.rs`;
  resolution lives only in `src/tui/hit_test.rs`, with the viewport→`line_idx` translation
  written exactly **once** and the Ctrl/Cmd gate checked exactly once.
- **AC4** (constraint, test): `hit_test::resolve_detail_click` is pure — its tests construct
  an affordances list + click coordinate and assert the `DetailClickTarget` with no `Model`
  mutation and no `Cmd`; the asset whole-row rule (`is_row_target`) and the
  `line_idx >= lines.len()` out-of-range guard are each covered by a test.
- **AC5** (behavior, test): the guard the old `affordance_at` lacked is now applied to comment
  Edit/Delete — a Ctrl/Cmd+click on a row that maps past the last content line resolves to
  no target (no panic, no stale hit).
- **CC** (constraint, inspection): clean code — `DetailClickTarget`, `resolve_detail_click`,
  `is_row_target` are self-describing; no banners/commented-out code; only non-obvious
  why-comments; comment-policy gate green.
- **CX** (constraint, command): complexity within budget — cyclomatic ≤ 10 (≤ 8 for new
  functions), cognitive ≤ 12; the resolver and its helper are each within budget
  (quality-gate arborist).
- **TE** (constraint, command): tests assert observable behavior (the resolved target per
  kind, the plain-click no-op, the row-target and guard cases) and survive the mutation floor
  — swapping a target arm, dropping the modifier gate, or removing the guard fails a test.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0044-detail-click-resolution-as-hit-test-module.md](/adr/0044-detail-click-resolution-as-hit-test-module.md)
- ADR: [/adr/0043-detail-hit-targets-emitted-structurally.md](/adr/0043-detail-hit-targets-emitted-structurally.md) (the registry consumed; foreshadowed this module)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md) (the `src/tui/` module tree)
