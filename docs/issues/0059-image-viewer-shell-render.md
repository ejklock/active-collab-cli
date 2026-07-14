---
type: Issue
title: "Image viewer slice 2 — ratatui-image shell protocol: fetch + decode + render, tmux half-block fallback, teardown redraw"
description: Slice 2 of the image attachment viewer. Add ratatui-image + image deps. In the shell, handle Cmd::LoadImage by fetching the attachment bytes over the authenticated client seam, decoding, and building a ratatui-image StatefulProtocol held in a shell-owned image_state side table (never in the Model). Render the protocol into the near-full detail-content-area viewer rect when the overlay is ImageViewer(Ready). Force half-blocks under tmux/screen. On close, drop image_state and force a full redraw. Verified by manual QA (graphics protocols are not buffer-assertable).
status: open
labels: [tui, images, ratatui-image, shell, slice]
blocked_by: [0058]
tracker:
timestamp: 2026-07-14T00:00:00Z
---

## Image viewer — slice 2: `ratatui-image` shell protocol + render

Slice 2 of [ADR 0065](/adr/0065-image-attachment-viewer-modal-overlay.md), on top of
[issue 0058](/issues/0058-image-viewer-overlay-lifecycle.md)'s pure lifecycle. Adds the
non-pure half — the actual image fetch, protocol, and render — entirely in the shell, so the
Model stays pure.

### Problem

Slice 1 renders a placeholder. This slice makes `Cmd::LoadImage` fetch and decode the image
and draw real pixels, without leaking non-pure protocol state into the pure core.

### Decision (from ADR 0065 §3–5)

- **Deps:** add `ratatui-image` and `image` to `Cargo.toml`.
- **Fetch (shell):** `dispatch_cmds` handles `Cmd::LoadImage` by fetching the attachment bytes
  over the authenticated client seam (host-gated token —
  [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md) /
  [ADR 0054](/adr/0054-comment-write-outcome-typed-classification.md)), decoding with `image`.
- **Protocol side table (shell):** build a `ratatui-image` `StatefulProtocol` and store it in
  a shell-owned `image_state: Option<ImageOverlayState>` — **never in the Model**. On success
  send `Msg::ImageLoaded` (signal only, no bytes); on failure `Msg::ImageLoadErr(reason)`.
- **Render (shell):** when `overlay` is `ImageViewer { status: Ready, .. }` and `image_state`
  is `Some`, render the protocol (`Resize::Fit`) into the near-full detail-content-area viewer
  rect (dim + `Clear` backdrop), with filename + `Esc/q fechar` hint.
- **Protocol selection + fallback:** build the `Picker` via `Picker::from_query_stdio()`;
  force the half-block protocol when `$TMUX` / `$STY` is set.
- **Teardown:** when the overlay leaves `ImageViewer` (close/mutation/reload), drop
  `image_state` and force a full redraw (graphics protocols don't all self-clear).

### Scope

Included:

- `Cargo.toml` — `ratatui-image`, `image`.
- `src/tui/mod.rs` / `dispatch_cmds` (or the async task module) — `Cmd::LoadImage` fetch +
  decode + `Picker`/`StatefulProtocol` build; `image_state` ownership + teardown + forced
  redraw; the `$TMUX`/`$STY` half-block selector.
- `src/tui/view.rs` — render the protocol into the viewer rect for `ImageViewer(Ready)`
  (replaces slice 1's placeholder for the Ready case).
- Tests: the pure fallback selector (`tests/unit/…`); manual QA for real rendering.

Excluded: classification, affordance, overlay variant, pure lifecycle (all slice 1); the
`chafa` backend (deferred — LGPL/GPL); inline/thumbnail rendering in the thread.

### Acceptance

- AC1 — fallback selector (unit, env-derived): the protocol selector returns the half-block
  backend when `$TMUX` (or `$STY`) is set, regardless of detected terminal. (`verify_by: command`)
- AC2 — fetch error path (unit): a failed/again-unauthorized fetch yields `Msg::ImageLoadErr`
  → the overlay shows `Error` (drives the ADR 0054 typed outcome). (`verify_by: test`)
- AC3 — no non-pure state in the Model (inspection/compile): the `StatefulProtocol` and
  decoded bytes live only in the shell `image_state`; the Model's `ImageViewer` variant
  carries only `asset` + `status`. (`verify_by: inspection`)
- AC4 — teardown (inspection/unit): closing the viewer drops `image_state` and triggers a full
  redraw. (`verify_by: inspection`)
- AC5 — manual QA (smoke): a real image attachment renders in a graphics terminal
  (Kitty/iTerm2/WezTerm); the same attachment degrades to half-blocks under tmux; closing the
  viewer leaves no smear. (`verify_by: inspection`)
- CC — clean code: no superfluous comments / banners / commented-out code. (`verify_by: inspection`)
- CX — complexity budget: cyclomatic ≤ 10 (≤ 8 new), cognitive ≤ threshold. (`verify_by: command`)
- TE — the pure fallback selector and error path assert observable behavior and survive the
  mutation floor; the render itself is manual-QA (graphics escapes are not buffer-assertable).
  (`verify_by: command`)

### Plan

1. `Cargo.toml`: add `ratatui-image`, `image`.
2. Shell: `Cmd::LoadImage` handler — fetch bytes (auth seam) → decode → build
   `Picker`/`StatefulProtocol`; store in `image_state`; send `ImageLoaded`/`ImageLoadErr`.
3. Shell: `$TMUX`/`$STY` → force half-block `Picker`.
4. `view.rs`: render the protocol (`Resize::Fit`) into the viewer rect for `Ready`.
5. Shell: teardown — drop `image_state` + force full redraw on overlay leave.
6. Tests: fallback selector, fetch-error path; manual QA across a graphics terminal + tmux.

Observable end-to-end: activate an image attachment on Kitty/iTerm2 and the screenshot renders
in a centered viewer; the same under tmux shows the half-block fallback; `Esc` closes cleanly.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- Manual QA: run the release binary against a task with an image attachment on a graphics
  terminal and inside tmux.
</content>
