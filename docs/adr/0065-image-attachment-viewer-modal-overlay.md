---
type: ADR
title: Image attachments open in a dedicated full-area viewer overlay (ratatui-image) — pure lifecycle in the Model, non-pure protocol state quarantined in the shell
description: Add an in-TUI viewer for image attachments. Image assets (png/jpg/jpeg/gif/webp/bmp by filename) gain a structural ViewImage affordance; activating it opens a new DetailOverlay::ImageViewer variant (extending ADR 0047's typed overlay, mutually exclusive with compose/confirm by construction). The viewer renders as a near-full detail-content-area overlay (not the ~70% text modal) via ratatui-image, whose protocol picker auto-detects Kitty/iTerm2/Sixel and falls back to Unicode half-blocks — forced to half-blocks under tmux/screen. The load-bearing split: the pure Model holds only the viewer's pure lifecycle (asset ref + Loading|Ready|Error status); the non-pure ratatui-image StatefulProtocol + decoded bytes live in a shell-owned side table keyed to the active overlay, built from bytes fetched over the authenticated client seam, and torn down (with a forced full redraw) on close. Non-image assets keep ADR 0025 external open.
status: Accepted
supersedes:
superseded_by:
tags: [tui, images, attachments, overlay, ratatui, ratatui-image, viewer, tea]
timestamp: 2026-07-14T00:00:00Z
---

# 0065. Image attachments open in a dedicated full-area viewer overlay

## Context

Task attachments today are opened **externally**: a Ctrl/Cmd+click on an asset row hands
the URL to the OS ([ADR 0025](/adr/0025-asset-activation-ctrl-cmd-click.md)), which for an
image means leaving the terminal for a browser. For the common case — glancing at a
screenshot pasted into a comment thread — that context switch is heavy.

Terminals *can* render raster images, via graphics protocols (Kitty, iTerm2 inline images,
Sixel) with a universal Unicode half-block fallback. The Rust/ratatui ecosystem wraps all
of these in **`ratatui-image`** (protocol auto-detection, font-size↔cell mapping,
non-blocking encode, half-block fallback). This ADR decides how an in-TUI image viewer fits
the app's architecture, given three hard constraints:

1. **The TEA core is pure.** `update(Model, Msg) -> (Model, Vec<Cmd>)` touches no terminal
   and no async ([ADR 0007](/adr/0007-tui-module-structure.md),
   [ADR 0008](/adr/0008-async-event-loop-with-eventstream-and-select.md)). A `ratatui-image`
   `StatefulProtocol` is the opposite of pure — it holds encode state, queries the
   terminal, is not `PartialEq`/`Clone`-friendly, and mutates as it resizes. It **cannot**
   live in the Model.
2. **Overlays are one typed value.** The Detail read view has exactly one active overlay,
   encoded as `DetailOverlay` ([ADR 0047](/adr/0047-detail-overlay-as-one-typed-state.md)),
   whose author anticipated growth: *"New overlay modes … extend the enum, with the
   compiler forcing every match to handle them."*
3. **Graphics protocols smear when scrolled and don't all self-clear**, and multiplexers
   (tmux/screen) strip the escapes. A viewer that owns a stable rect and forces a full
   redraw on close is far more robust than pixels interleaved into the scrolling thread.

## Decision

We will add an **image viewer as a dedicated full-area overlay**, splitting its state so the
**pure lifecycle lives in the Model** and the **non-pure `ratatui-image` protocol lives in
the shell**.

### 1. Image-asset classification + a `ViewImage` affordance

An asset whose derived filename ([ADR 0023](/adr/0023-asset-label-derivation.md)) ends in a
viewable raster extension (`png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp`) is an **image asset**.
Image assets gain a structural `AffordanceKind::ViewImage` span, emitted into the one
`affordances` registry at layout time exactly like the existing asset/URL hit-targets
([ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md)). Activating it opens the
viewer. **Non-image assets are unaffected** — they keep ADR 0025 external open. (Whether an
image asset *also* keeps an external-open affordance is left to the issue's UX; the default
is: image → view in-TUI, everything else → external.)

### 2. A new `DetailOverlay::ImageViewer` variant — pure lifecycle only

```rust
enum ImageStatus { Loading, Ready, Error(String) }
struct ImageAssetRef { url: String, label: String }   // pure, testable metadata

enum DetailOverlay {
    None,
    Compose(Compose),
    ConfirmDelete { comment_id: i64 },
    ImageViewer { asset: ImageAssetRef, status: ImageStatus },   // new
}
```

The variant carries **no pixels and no protocol** — only the asset reference and the
lifecycle status. It is mutually exclusive with compose/confirm *by construction* (ADR
0047's whole point): you cannot view an image while composing. Opening the viewer sets
`overlay = ImageViewer { asset, status: Loading }` and emits `Cmd::LoadImage { asset }`;
`Esc`/`q` sets `overlay = None`.

### 3. The load-bearing split — protocol state lives in the shell

```mermaid
stateDiagram-v2
  [*] --> Loading: activate ViewImage → Cmd::LoadImage
  Loading --> Ready: Msg::ImageLoaded (shell built the Protocol)
  Loading --> Error: Msg::ImageLoadErr(reason)
  Ready --> [*]: Esc/q → overlay=None (shell drops Protocol + full redraw)
  Error --> [*]: Esc/q → overlay=None
```

The flow keeps every non-pure concern out of `update()`:

- **`update()` (pure):** emits `Cmd::LoadImage`, and on the reply flips
  `status` to `Ready` / `Error(reason)`. That is *all* it does — no bytes, no protocol.
- **The shell (`dispatch_cmds`):** handles `Cmd::LoadImage` by fetching the attachment bytes
  over the **authenticated client seam** (the host-gated token —
  [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md) /
  [ADR 0054](/adr/0054-comment-write-outcome-typed-classification.md); ActiveCollab
  attachments require the token), decoding with the `image` crate, and building a
  `ratatui-image` `StatefulProtocol`. It stores that protocol in a **shell-owned side table**
  (`image_state: Option<ImageOverlayState>`), then sends `Msg::ImageLoaded` (a signal, *no
  bytes*) or `Msg::ImageLoadErr(reason)`.
- **`view()`:** when `overlay` is `ImageViewer { status: Ready, .. }` **and**
  `shell.image_state` is `Some`, it renders the protocol into the viewer rect;
  `Loading` shows a placeholder/spinner; `Error` shows the message.
- **Close:** `overlay = None` → the shell **drops `image_state` and forces a full redraw**
  (graphics protocols don't all self-clear).

This mirrors precisely how the shell already owns terminal-coupled concerns the pure core
must not touch — mouse-capture toggling ([ADR 0021](/adr/0021-app-managed-text-selection-clipboard.md))
and the "which keys are text" decision ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md)).
The Model stays a pure, unit-testable state machine; the pixels are quarantined.

### 4. Rendering: a near-full-area viewer, not the text modal

The viewer reuses the modal **backdrop** idea (dim + `Clear`) but sizes to the **whole
detail content area**, not the ≈70 % text box of ADR 0039 — graphics need a generous,
stable rect. The image is fit with `Resize::Fit` (aspect preserved), with the filename and
an `Esc/q fechar` hint on a bottom line. This is a distinct render path from `render_modal`;
it does **not** change the compose/confirm modal.

### 5. Protocol selection + tmux fallback

The shell builds the `ratatui-image` `Picker` via `Picker::from_query_stdio()`
(auto-detects Kitty / iTerm2 / Sixel, else half-blocks). When `$TMUX` or `$STY` is set, it
**forces the half-block protocol** — the #1 real-world breakage is multiplexers stripping
graphics escapes. Protocol choice is a shell concern; the Model never sees it.

### Guard / fitness function

- **Pure lifecycle (unit, headless):** activating `ViewImage` sets
  `overlay = ImageViewer { status: Loading }` and emits `Cmd::LoadImage`; `Msg::ImageLoaded`
  → `Ready`; `Msg::ImageLoadErr(r)` → `Error(r)`; `Esc`/`q` → `None`. No terminal, no async,
  no bytes in the Model.
- **Mutual exclusion by construction:** there is no representable state with an image viewer
  *and* compose/confirm active — the `DetailOverlay` match is exhaustive (compiler-enforced;
  ADR 0047).
- **Classification (unit):** the image-extension predicate accepts `png/jpg/jpeg/gif/webp/bmp`
  (case-insensitive) and rejects `pdf/docx/…`; a `ViewImage` affordance is emitted for image
  assets only.
- **Fallback (unit, env-derived):** the protocol selector returns the half-block backend when
  `$TMUX` is set, regardless of the detected terminal.
- **Render (`TestBackend`, buffer-derived):** `Loading` renders the placeholder + hint over
  the dimmed backdrop; `Error` renders the message. (Actual graphics-protocol pixels are not
  buffer-assertable — the half-block fallback path and the placeholder/error states are; true
  image fidelity is covered by manual QA, below.)
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, `comment_policy` clean;
  complexity within budget; mutation floor (Reviewer backstop) on the classification
  predicate, the lifecycle transitions, and the fallback selector.

## Alternatives considered

- **Render images inline in the scrollable thread.** Rejected: fights the scroll model
  (graphics place pixels at absolute cells and smear when the text scrolls), needs a protocol
  per image re-encoded on every scroll, and drags non-pure protocol state into the content
  pipeline that the pure core reflows. A stable, isolated rect is the robust shape.
- **A separate top-level `Screen::ImageViewer`.** Rejected: viewing is an interaction
  *within* the open task (shares its context), and it is mutually exclusive with the other
  Detail overlays — the exact case `DetailOverlay` models. A new `Screen` would duplicate the
  task context and complicate the navigation stack (same reasoning ADR 0034 used for compose).
- **Put the `StatefulProtocol` (or the decoded bytes) in the Model.** Rejected: the protocol
  is terminal-coupled and not `PartialEq`/`Clone`; even raw bytes in the Model bloat it and
  break the pure-value contract every existing `tests/unit/model.rs` spec relies on. Pure
  lifecycle in the Model, non-pure protocol in the shell, is the only split that keeps
  `update()` testable.
- **`viuer` instead of `ratatui-image`.** Rejected: `viuer` writes directly to stdout and is
  not built for ratatui's immediate-mode redraw; `ratatui-image` exists specifically to solve
  the immediate-mode case (and adds Sixel).
- **Bundle the `chafa` backend now** (max protocol/format coverage). Deferred: `chafa` is
  LGPL/GPL and a static link has licensing weight against a distributed binary; the built-in
  Kitty/iTerm2/Sixel + half-block set covers the need. Revisit behind a feature flag with
  dynamic linking if coverage demands it (YAGNI).

## Consequences

**Easier / gained:**
- Image attachments are viewable **without leaving the TUI**, on any terminal (best fidelity
  on Kitty/iTerm2/WezTerm; graceful half-block fallback elsewhere).
- The viewer is one more `DetailOverlay` variant — the typed-overlay design pays off exactly
  as ADR 0047 predicted, and the compiler forces every overlay match to handle it.
- The pure/shell boundary is preserved: the Model gains a pure, fully-unit-testable viewer
  lifecycle; all non-pure image state is quarantined in the shell.

**Harder / accepted trade-offs:**
- Two new dependencies (`ratatui-image`, `image`) and a shell-owned side table with a real
  lifecycle (create on load, drop + force-redraw on close).
- The rendered image itself is **not** deterministically testable (graphics protocols emit
  terminal escapes, not buffer cells) — only the fallback/placeholder/error states are
  buffer-assertable; true fidelity relies on **manual QA** across a graphics terminal and a
  tmux session.
- Terminal-specific rough edges remain (some terminals don't self-clear graphics; tmux needs
  passthrough for true graphics) — mitigated by the forced-redraw-on-close and the tmux
  half-block fallback, and documented for the user.

**Follow-ups:**
- Issue (slice 1): classification + `ViewImage` affordance + `DetailOverlay::ImageViewer`
  variant + the pure load lifecycle (placeholder render only).
- Issue (slice 2, blocked by slice 1): the shell protocol — `ratatui-image`/`image` deps,
  fetch+decode+build `Picker`/`StatefulProtocol`, the `image_state` side table, real render,
  tmux half-block fallback, teardown + forced redraw.
- `architecture.md` overlay/state prose + diagram updated when slice 1 lands (maintenance
  rule).

## Verification

**Implementation impact:** `src/render/detail_render.rs` (image classification + `ViewImage`
affordance), `src/tui/hit_test.rs` (`ViewImage` → open viewer), `src/tui/model.rs`
(`DetailOverlay::ImageViewer`, lifecycle arms, `Cmd::LoadImage`), `src/tui/mod.rs` /
`dispatch_cmds` (fetch+decode+protocol, `image_state`, tmux fallback, teardown),
`src/tui/view.rs` (viewer render path), `Cargo.toml` (`ratatui-image`, `image`),
`tests/unit/model.rs` + `tests/unit/tui_render.rs`.

**Verification criteria:**
- The lifecycle state machine (`Loading → Ready/Error → None`) is driven entirely through
  `update()` with no non-pure state in the Model (fitness function: `tests/unit/model.rs`).
- The image-extension predicate accepts the raster set and rejects non-images; `ViewImage` is
  emitted for image assets only (fitness function: classification unit test).
- The protocol selector forces half-blocks under `$TMUX` (fitness function: env-derived unit
  test).
- Manual QA: a real image attachment renders in a graphics terminal and degrades to
  half-blocks under tmux; closing the viewer leaves no smear.

## Related

- ADR: [/adr/0047-detail-overlay-as-one-typed-state.md](/adr/0047-detail-overlay-as-one-typed-state.md) (the typed overlay this extends)
- ADR: [/adr/0025-asset-activation-ctrl-cmd-click.md](/adr/0025-asset-activation-ctrl-cmd-click.md) (external open, kept for non-image assets)
- ADR: [/adr/0023-asset-label-derivation.md](/adr/0023-asset-label-derivation.md) (the derived filename the classifier reads)
- ADR: [/adr/0043-detail-hit-targets-emitted-structurally.md](/adr/0043-detail-hit-targets-emitted-structurally.md) (the affordance registry `ViewImage` joins)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md), [/adr/0054-comment-write-outcome-typed-classification.md](/adr/0054-comment-write-outcome-typed-classification.md) (the authenticated seam that fetches the bytes)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md), [/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md) (the shell-owned overlay/backdrop precedents)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md), [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md) (the pure TEA core the shell split preserves)
- Issues: [/issues/0058-image-viewer-overlay-lifecycle.md](/issues/0058-image-viewer-overlay-lifecycle.md), [/issues/0059-image-viewer-shell-render.md](/issues/0059-image-viewer-shell-render.md)
</content>
