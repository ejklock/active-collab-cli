---
type: ADR
title: Assets render inline in the globally-scrollable detail content (retire the fixed asset panel and its height cap)
description: Fold the Anexos/Artefatos list out of the fixed bottom panel and into the single globally-scrollable detail content so every attachment is reachable by scrolling, never silently clipped. The asset_panel layout stays the single composition source (ADR 0028) but feeds inline content lines plus a scroll-aware line-to-asset-index map instead of a capped fixed block; Ctrl/Cmd+click resolves the asset through the same offset-aware translation the body links use. Removes the ASSET_PANEL_MAX_ROWS ceiling; carries forward the D1d breathing room and the D1f italic hint inline.
status: Accepted
supersedes: [0024]
superseded_by:
tags: [tui, ux, ratatui, detail, assets, scroll, layout]
timestamp: 2026-06-27T00:00:00Z
---

# 0029. Assets render inline in the globally-scrollable detail content

## Context

The Detail screen renders its content in **one global scroll** region — header
meta, description body, and comments — with a **fixed Artifacts panel** pinned to
the bottom for the task's assets. That fixed panel is the last remnant of the
sectioned-panel experiment ([ADR 0010](/adr/0010-detail-sectioned-panels-focus-scroll.md),
**reverted**; its standing lesson is *"always one global scroll for a read view"*).

The fixed panel has a **named height ceiling** `ASSET_PANEL_MAX_ROWS = 14`
([ADR 0024](/adr/0024-asset-card-breathing-room.md)). A task with more attachments
than fit under that ceiling has the overflow **silently clipped** — no scroll, no
indicator, no way to reach them. On ActiveCollab a task can easily carry more than
five or six attachments (screenshots, documents), so this is a plausible loss of
access to real data, not a pathological edge. The architecture-review of the asset
panel ([ADR 0028](/adr/0028-asset-panel-single-layout-source.md)) and its opus
review flagged it: at the cap only ~5–6 short assets are clickable and a "tenth
asset" is clipped and unreachable.

The Detail screen already has the machinery to make everything reachable: the body
links (V5/D1c, [ADR 0020](/adr/0020-body-links-inline-url-native-click.md)) are
part of the scrollable content and are clicked through a **scroll-aware hit-test**
(`body_link_cmd_at`: `line_idx = offset + (row − text_top)`). Assets are the only
clickable element NOT in that scroll flow.

Force: **no silent loss of access** (correctness/UX) plus **alignment with the
project's own decision** — a read view has one global scroll (ADR 0010 reverted).
The asset list should live in that scroll like everything else.

## Decision

Fold the asset list **into the globally-scrollable detail content** and retire the
fixed bottom panel. Every attachment becomes reachable by scrolling; nothing is
clipped.

### 1. The asset section is scrollable content, not a fixed panel

`build_detail_content` (`src/render.rs`) appends, after the body and comments, an
**asset section**: a localized `Anexos`/`Artefatos` header line, the per-asset
`[n] ↗ label` rows (the [ADR 0023](/adr/0023-asset-label-derivation.md) label, the
[ADR 0024](/adr/0024-asset-card-breathing-room.md) breathing room — a blank row
between consecutive assets — carried forward inline), and the italic Ctrl/Cmd+click
footnote ([ADR 0027](/adr/0027-asset-open-hint-in-card.md)) as the last line. These
lines join the same `lines`/`line_styles` arrays the body and comments use, so the
existing single `Paragraph` + global scroll + scrollbar render them with no special
case. There is **no bordered card / fixed chunk** anymore — the box gives way to an
inline section (the user accepted the inline treatment).

### 2. `asset_panel` stays the single composition source — repurposed (amends ADR 0028)

[ADR 0028](/adr/0028-asset-panel-single-layout-source.md)'s principle holds: the
asset-row composition lives in **one** place, `src/tui/screens/asset_panel.rs`'s
pure `layout(assets, width) -> Vec<PanelRow>`. What changes is what the module
feeds and what it drops:

- **Retained:** `PanelRow` and `layout` (the one composition both the rendered
  lines and the click map derive from — the single-source invariant survives).
- **Retired:** `apply_cap`, `height`, the block `render`, the fixed-panel
  `index_at`, and the `ASSET_PANEL_MAX_ROWS` ceiling — all artifacts of a capped
  fixed panel that no longer exists. **This subsumes the deferred "Note 1"
  apply_cap refinement: with no cap there is nothing to truncate.**
- **Added:** a converter from the `Vec<PanelRow>` to the content lines + their
  styles (asset rows carry the link style, the hint carries italic/dim), and a
  pure **section line → asset-index** map so a clicked content line resolves to an
  asset. Both derive from the one `layout` vector, so render and hit-test still
  cannot drift.

### 3. Asset clicks become scroll-aware, sharing the body-link translation

`asset_panel_cmd_at`'s fixed-panel hit-test (`panel_top = viewport_rows −
panel_h`) is replaced by the **offset-aware** translation the body links already
use: a Ctrl/Cmd/Super+click at viewport `row` maps to content line `offset + (row −
text_top)`; if that line falls in the asset section, the section map yields the
asset index and emits `Cmd::OpenAsset { url: asset.url }`. The Ctrl/Cmd gate and
the plain-click-does-nothing reservation (V6 selection) are unchanged. Because the
asset URL lives on the `Asset` (not as a `[url]` token in the text), the asset path
uses the section index map rather than `resolve_wrapped_url` — but the
row→line-index step is the identical machine.

### 4. Scroll bounds drop the panel term

`detail_max_offset` and `is_in_body_area` (`src/tui/model.rs`) stop subtracting the
fixed-panel height: the whole inner area is now one scrollable body, and the max
offset is simply `lines.len() − text_viewport_height`. The asset rows extend
`lines`, so scrolling naturally reaches them.

### Guard

The breathing room ([BDR 0018](/bdr/0018-asset-card-breathing-room.md)) and the
italic hint ([BDR 0021](/bdr/0021-asset-open-hint-in-card.md)) are preserved as
observable behavior, re-pinned inline by the new [BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md)
(which supersedes the fixed-panel-geometry scenarios of 0018/0021). Full suite
green; `clippy -D warnings`, `fmt`, comment-policy clean; complexity within budget;
the section composition and the scroll-aware click are tested (the `Vec<PanelRow>`
and the line→index map are the test surfaces, plus a TestBackend render derived
from the real buffer).

## Alternatives considered

- **Viewport-responsive cap** (`height = min(natural, viewport − MIN_BODY)`), the
  escape hatch ADR 0024 deferred. Lighter (keeps the fixed panel), but it **still
  clips** when the attachments exceed even the full-panel size — it does not
  deliver "every attachment reachable", which is the requirement. Rejected as a
  half-measure.
- **A panel with its own independent scroll.** Rejected: it re-litigates the
  **reverted** [ADR 0010](/adr/0010-detail-sectioned-panels-focus-scroll.md)
  (per-section focus + independent scroll), whose standing lesson is one global
  scroll for a read view. Folding into the global scroll is the aligned mechanism.
- **A `+N more` overflow indicator on the last visible panel row.** A cheap signal
  against silent loss, but it still does not make the hidden attachments
  *reachable* — it only advertises that they exist. Rejected in favor of full
  access; kept on record as a smaller fallback if the inline fold were ever undone.
- **Keep the fixed panel, accept the clipping** (the review's non-blocking
  read). Rejected here because the user asked for full access: silently losing
  access to attachments is a correctness/UX defect worth the structural change.

## Consequences

**Positive:** every attachment is reachable by scrolling — no silent clipping. The
Detail screen is now uniformly one global scroll (completes the ADR 0010-reverted
direction). Asset clicks reuse the body-link scroll-aware translation, so there is
one click model for everything in the content. The asset geometry leaves the model
(the panel height term disappears from `detail_max_offset`/`is_in_body_area`).
ADR 0028's single-composition-source survives (one `layout`, render + click map
derive from it).

**Accepted trade-offs:** the bordered Artifacts *card* becomes an inline section
(no box) — a visible change the user accepted in exchange for full access. Asset
clicks now depend on scroll position (correct, and consistent with body links).
A churny diff across `render.rs`, `detail.rs`, `model.rs`, `asset_panel.rs`, and
the tests; sliced and verified per slice. The asset-row link styling and the hint's
italic/dim must be reproduced through the content styling path (a known integration
point), pinned by the new BDR.

## Related

- ADR: [/adr/0028-asset-panel-single-layout-source.md](/adr/0028-asset-panel-single-layout-source.md) (amended — the layout source is retained and repurposed; the cap/height/block-render/panel-hit-test are retired)
- ADR: [/adr/0024-asset-card-breathing-room.md](/adr/0024-asset-card-breathing-room.md) (**superseded** — the height ceiling is removed; the breathing room is carried forward inline)
- ADR: [/adr/0010-detail-sectioned-panels-focus-scroll.md](/adr/0010-detail-sectioned-panels-focus-scroll.md) (reverted — the "one global scroll" lesson this fold completes)
- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (the scroll-aware click machine the assets reuse)
- ADR: [/adr/0027-asset-open-hint-in-card.md](/adr/0027-asset-open-hint-in-card.md) (the italic hint, now an inline footnote)
- BDR: [/bdr/0022-assets-inline-scrollable-detail-content.md](/bdr/0022-assets-inline-scrollable-detail-content.md)
- Issue: [/issues/0028-assets-inline-scrollable-content.md](/issues/0028-assets-inline-scrollable-content.md)
- Architecture: [/architecture.md](/architecture.md)
