---
type: Issue
title: "Body-link hit-target is emitted structurally; resolve_wrapped_url + inverse-wrap helpers are deleted (slice 2)"
description: The body-line build path emits one OpenUrl(normalized) affordance span per wrapped fragment of an openable URL/email token, resolving and validating the target once at emit time. body_link_cmd_at becomes a positional lookup over DetailContent.affordances. resolve_wrapped_url and its inverse-wrap helpers (logical_position_in_wrap_group, url_at_in_wrap_group) are deleted, retiring the obs-35 latent over-join bug. Observable behavior (Ctrl/Cmd+click on any wrapped fragment opens the complete URL) is unchanged.
status: closed
labels: [tui, render, link, hit-test, affordance, refactor, slice]
blocked_by: 0045
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## Body-link hit-target → structural emission + delete the re-derivation (slice 2 of ADR 0043)

Implements [ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md) decision steps
3–4 (body half + deletion). Preserves [BDR 0014](/bdr/0014-body-link-inline-url-activation.md)
Scenario 7 (click on any wrapped fragment opens the **complete** URL) and Scenario 8 (plain
click reserved) with no observable change.

### Problem

`body_link_cmd_at` (`src/tui/model.rs:1310`) calls `render::resolve_wrapped_url`
(`src/render.rs:230`), which **re-scans the rendered `lines`** and runs inverse-wrap math
(`logical_position_in_wrap_group`, `url_at_in_wrap_group`) to reconstruct the complete URL
from whichever wrapped fragment was clicked — re-deriving what the layout knew **before** it
wrapped the token. That inverse-wrap helper carries a known latent bug (obs 35: wrap-group
continuation detection over-joins a word-boundary line exactly `content_width` wide).

### Decision (from ADR 0043)

Emit the body-link hit-target on `DetailContent.affordances` at layout time (the full token
is known before wrapping), reduce `body_link_cmd_at` to a lookup, and delete the inverse-wrap
re-derivation.

### Scope

Included:

- `src/render.rs` — in the body-line build path (`build_body_lines_with_collector` / the
  wrap step that lays out an inline URL token), push one
  `LocalAffordance { kind: OpenUrl(normalized) }` per wrapped fragment of the token, **only**
  when the token is openable. The `normalize_link_url` + `is_openable_url` + mailto checks
  (currently in `model.rs`) move to emit time; a non-openable `[note]` registers no span.
  **Delete** `resolve_wrapped_url` and its inverse-wrap helpers
  (`logical_position_in_wrap_group`, `url_at_in_wrap_group`) once nothing calls them.
- `src/tui/model.rs` — `body_link_cmd_at` becomes a positional lookup over `affordances`
  (filter to `OpenUrl`, gated on Ctrl/Cmd), returning `Cmd::OpenAsset { instance, url }`.
  The `normalize_link_url` / `is_mailto_url` / `panel_content_width_pub` /
  `resolve_wrapped_url` call chain is removed from this function (helpers relocate to the
  emit site or are deleted if unused).
- `src/richtext.rs` — only if the body wrap path needs the pre-wrap token surfaced to the
  emit site (e.g. through the `LinkCollector`/`StyleRun` seam); keep changes minimal and
  structural.
- Tests: `tests/unit/tui_render.rs` / `tests/unit/render.rs` — buffer/layout-derived: the
  emitted `affordances` carry `OpenUrl(complete_url)` over **every** wrapped fragment of a
  long URL; Ctrl/Cmd+click on any fragment still opens the complete URL; a `mailto:` email
  resolves; a non-openable `[note]` carries no `OpenUrl`; the **obs-35 over-join case** (a
  word-boundary line exactly `content_width` wide) opens the correct URL.

Excluded: the asset half (slice 0045, the dependency); collapsing the three hit-tests into a
`tui/hit_test` module (ADR 0043 leaves this for later).

### Acceptance

- AC1 — structural emission (`verify_by: test`): `build_detail_content` output carries an
  `OpenUrl(complete_url)` affordance span over **every** wrapped fragment of an openable URL
  token; a bare email carries `OpenUrl("mailto:…")`; a non-openable `[note]` carries no
  `OpenUrl` span.
- AC2 — complete-URL click preserved (`verify_by: test`): a Ctrl/Cmd+click on any wrapped
  fragment of a long URL still emits `Cmd::OpenAsset` with the **complete** URL (BDR 0014
  Sc.7), buffer-derived. Existing body-link click specs stay green.
- AC3 — re-derivation deleted (`verify_by: inspection`): `resolve_wrapped_url`,
  `logical_position_in_wrap_group`, and `url_at_in_wrap_group` no longer exist in
  `src/render.rs`; `body_link_cmd_at` resolves via the `affordances` lookup only.
- AC4 — obs-35 case fixed (`verify_by: test`): a body URL whose token wraps so a continuation
  line is exactly `content_width` wide opens the correct, complete URL (the case the deleted
  inverse-wrap helper mis-joined).
- AC5 — plain click reserved (`verify_by: test`): a plain (no Ctrl/Cmd) click on a body link
  does not emit `Cmd::OpenAsset` (BDR 0014 Sc.8).
- CC — clean code (named `OpenUrl`; validation single-homed at the emit site; no
  banners/commented-out; only non-obvious why-comments) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior (emitted payload per fragment + rendered-buffer
  click + obs-35 case) and survive the mutation floor: dropping a fragment span, or emitting
  the wrong/partial url, fails a test (`verify_by: command`).

### Plan

1. `render.rs`: at the body-link wrap/emit site, resolve+validate the token once
   (`normalize_link_url` + `is_openable_url`/mailto) and push `OpenUrl(normalized)` spans —
   one per wrapped fragment — into `affordances`.
2. `model.rs`: rewrite `body_link_cmd_at` as a lookup over `affordances` (OpenUrl,
   Ctrl/Cmd-gated); remove the `resolve_wrapped_url` call chain and relocate/retire the
   now-unused link helpers.
3. `render.rs`: delete `resolve_wrapped_url`, `logical_position_in_wrap_group`,
   `url_at_in_wrap_group` (and any now-dead `*_pub` accessor) once nothing references them.
4. Tests: per-fragment emitted-payload + buffer-derived Ctrl/Cmd+click on a wrapped URL +
   mailto + non-openable negative + the obs-35 exact-width case + plain-click negative.

Observable end-to-end: unchanged — clicking any fragment of a wrapped link still opens the
complete URL; the difference is the URL is emitted once by the layout (not reconstructed on
click), and the fragile inverse-wrap helper (with its latent bug) is gone.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
