---
type: Issue
title: "Image viewer slice 1 — image-asset classification, ViewImage affordance, DetailOverlay::ImageViewer, pure load lifecycle"
description: Slice 1 of the image attachment viewer. Classify image assets (png/jpg/jpeg/gif/webp/bmp by derived filename) and emit a structural ViewImage affordance. Add the DetailOverlay::ImageViewer { asset, status } variant (extends ADR 0047). Wire the pure lifecycle — activating ViewImage sets Loading and emits Cmd::LoadImage; Msg::ImageLoaded → Ready; Msg::ImageLoadErr → Error; Esc/q → None. NO ratatui-image yet — view() renders a placeholder/error only. Fully unit-testable in the pure core.
status: open
labels: [tui, images, attachments, overlay, slice]
blocked_by:
tracker:
timestamp: 2026-07-14T00:00:00Z
---

## Image viewer — slice 1: classification + overlay + pure lifecycle

Slice 1 of [ADR 0065](/adr/0065-image-attachment-viewer-modal-overlay.md). Delivers the
pure, unit-testable half of the viewer (no `ratatui-image` yet — placeholder render). Slice 2
([issue 0059](/issues/0059-image-viewer-shell-render.md)) adds the shell protocol + real
render.

### Problem

Image attachments only open externally (Ctrl/Cmd+click → OS, [ADR 0025](/adr/0025-asset-activation-ctrl-cmd-click.md)),
forcing a context switch out of the terminal to glance at a screenshot.

### Decision (from ADR 0065 §1–3)

- **Classify:** an asset whose derived filename ([ADR 0023](/adr/0023-asset-label-derivation.md))
  ends in `png/jpg/jpeg/gif/webp/bmp` (case-insensitive) is an image asset.
- **Affordance:** image assets emit a structural `AffordanceKind::ViewImage` span into the
  `affordances` registry ([ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md)).
  Non-image assets keep ADR 0025 external open, unchanged.
- **Overlay variant:** add `DetailOverlay::ImageViewer { asset: ImageAssetRef, status:
  ImageStatus }` with `ImageStatus = Loading | Ready | Error(String)` — pure metadata only, no
  pixels/protocol. Mutually exclusive with compose/confirm by construction (ADR 0047).
- **Pure lifecycle:** activate `ViewImage` → `overlay = ImageViewer { status: Loading }` +
  emit `Cmd::LoadImage { asset }`; `Msg::ImageLoaded` → `Ready`; `Msg::ImageLoadErr(r)` →
  `Error(r)`; `Esc`/`q` → `None`.
- **Placeholder render only:** `view()` renders a centered placeholder (`Carregando…` /
  filename) for `Loading`/`Ready` and the message for `Error`, over the dimmed backdrop. No
  `ratatui-image` in this slice.

### Scope

Included:

- `src/render/detail_render.rs` — image-extension predicate + `AffordanceKind::ViewImage`
  emission.
- `src/tui/hit_test.rs` — `ViewImage` target resolves to "open image viewer".
- `src/tui/model.rs` — `DetailOverlay::ImageViewer` variant, `ImageStatus`, `ImageAssetRef`,
  `Cmd::LoadImage`, `Msg::ImageLoaded`/`ImageLoadErr`, the lifecycle arms, `Esc`/`q` routing
  (extend the overlay match — the compiler forces it).
- `src/tui/view.rs` — the placeholder/error render path for the viewer overlay.
- Tests: `tests/unit/model.rs`, `tests/unit/tui_render.rs`.

Excluded: `ratatui-image`/`image` deps, byte fetch, decode, protocol, tmux fallback, real
image render, teardown redraw — all slice 2 (issue 0059). No change to non-image asset open.

### Acceptance

- AC1 — classification (unit): the predicate accepts `png/jpg/jpeg/gif/webp/bmp`
  (case-insensitive) and rejects `pdf/docx/txt/…`. (`verify_by: test`)
- AC2 — affordance (unit/render): an image asset row emits a `ViewImage` affordance; a
  non-image asset does not (it keeps the external-open affordance). (`verify_by: test`)
- AC3 — open (unit): activating `ViewImage` sets `overlay = ImageViewer { status: Loading }`
  and emits `Cmd::LoadImage { asset }`. (`verify_by: test`)
- AC4 — lifecycle (unit): `Msg::ImageLoaded` → `Ready`; `Msg::ImageLoadErr(r)` → `Error(r)`;
  `Esc` and `q` → `overlay = None`. (`verify_by: test`)
- AC5 — mutual exclusion (unit/compile): the `DetailOverlay` match is exhaustive; no state
  holds `ImageViewer` together with compose/confirm. (`verify_by: test`)
- AC6 — placeholder render (`TestBackend`): `Loading` renders the placeholder + filename over
  the dimmed backdrop; `Error` renders the message. (`verify_by: test`)
- CC — clean code: no superfluous comments / banners / commented-out code. (`verify_by: inspection`)
- CX — complexity budget: cyclomatic ≤ 10 (≤ 8 new), cognitive ≤ threshold. (`verify_by: command`)
- TE — tests assert observable behavior (classification result, emitted affordance, overlay
  transitions, buffer-derived placeholder) and survive the mutation floor. (`verify_by: command`)

### Plan

1. `detail_render.rs`: image-extension predicate + `ViewImage` affordance emission.
2. `model.rs`: `ImageAssetRef`, `ImageStatus`, `DetailOverlay::ImageViewer`, `Cmd::LoadImage`,
   `Msg::ImageLoaded`/`ImageLoadErr`; lifecycle arms; `Esc`/`q` routing.
3. `hit_test.rs`: `ViewImage` → open viewer.
4. `view.rs`: placeholder/error render path.
5. Tests: classifier table, affordance emission, open→Loading+Cmd, lifecycle transitions,
   exhaustive-overlay, placeholder render.
6. Update `architecture.md` overlay/state prose + diagram (maintenance rule).

Observable end-to-end: activate an image attachment and a centered `Carregando…` panel opens
over the dimmed thread; `Esc` closes it. (Real pixels arrive in slice 2.)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
</content>
