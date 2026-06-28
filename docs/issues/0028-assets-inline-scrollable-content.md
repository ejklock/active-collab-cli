---
type: Issue
title: "Assets render inline in the globally-scrollable detail content (retire the fixed asset panel + cap)"
description: Fold the Anexos/Artefatos list out of the fixed bottom panel and into the single globally-scrollable detail content so every attachment is reachable by scrolling; replace the fixed-panel asset click with the scroll-aware (offset) hit-test the body links use; retire ASSET_PANEL_MAX_ROWS, apply_cap, the panel height and block render; carry the D1d breathing room and the D1f italic hint inline.
status: closed
labels: [tui, ux, detail, assets, scroll, layout]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## Assets inline in the scrollable detail content

Implements [ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md),
observable behavior pinned by [BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md)
(which supersedes BDR 0018 + BDR 0021). Amends [ADR 0028](/adr/0028-asset-panel-single-layout-source.md)
(asset_panel repurposed) and supersedes [ADR 0024](/adr/0024-asset-card-breathing-room.md)
(cap removed). Surfaced while attacking the ADR 0028 review notes: attachments
beyond the `ASSET_PANEL_MAX_ROWS` ceiling were silently clipped and unreachable.

### Problem

The asset list lives in a fixed bottom panel capped at `ASSET_PANEL_MAX_ROWS = 14`;
attachments beyond the cap are silently clipped — no scroll, no indicator. The
detail body links are already part of the global scroll and clicked through a
scroll-aware hit-test; assets are the only clickable element left out of that flow.

### Decision

Fold the assets into the single globally-scrollable content (a localized
`Anexos`/`Artefatos` header, the `[n] ↗ label` rows with breathing room, an italic
Ctrl/Cmd footnote), retire the fixed panel and its cap, and make the asset click
scroll-aware by sharing the body-link `offset + (row − text_top)` translation. See
ADR 0029 for the full decision and rejected alternatives.

### Scope

Included:

- `src/render.rs` — `build_detail_content` appends the asset section (header + rows
  + per-link blank separators + italic hint) to `lines`/`line_styles`, styled (link
  style on the rows, italic/dim on the hint), and exposes the asset section's line
  range / line→asset-index map on `DetailContent`.
- `src/tui/screens/asset_panel.rs` — retain `PanelRow` + `layout`; add the
  `PanelRow → (line, style)` converter and the pure section line→asset-index map;
  remove `apply_cap`, `height`, the block `render`, the fixed-panel `index_at`, and
  `ASSET_PANEL_MAX_ROWS`.
- `src/tui/screens/detail.rs` — `draw_detail` renders one scrollable content region
  (drop the `[Min(0), Length(panel_h)]` split and the `asset_panel::render` call);
  `DetailParams` drops the fixed-panel path.
- `src/tui/screens/mod.rs` — drop the `asset_panel_render_height` re-export.
- `src/tui/model.rs` — `asset_panel_cmd_at` becomes scroll-aware (offset + section
  map) or is merged into the detail click path; `detail_max_offset` and
  `is_in_body_area` drop the panel-height term.
- `tests/unit/*` — section composition (`Vec<PanelRow>` + map), all-reachable scroll
  bound, scroll-aware click (with offset), TestBackend render derived from the real
  buffer; repoint/retire the fixed-panel tests.

Excluded: the body links (D1c, unchanged); the label derivation (D1b, unchanged);
other screens; a `+N more` indicator (rejected per ADR 0029).

### Acceptance

- The asset section (header + `[n] ↗ label` rows + per-link blank separators +
  italic Ctrl/Cmd footnote) renders at the end of the scrollable content, with no
  separate bordered panel (BDR 0022 Sc.1, 3, 4).
- A task with more attachments than fit on screen exposes every attachment by
  scrolling — none clipped; the max scroll offset reaches the last asset row
  (Sc.2). Empty list → no section, scroll bound excludes any panel term (Sc.7).
- Ctrl/Cmd/Super+click on a visible asset row at viewport row R, scroll offset O,
  emits `Cmd::OpenAsset` for the asset at content line `O + (R − text_top)` (Sc.5);
  header/blank/footnote rows and plain clicks emit nothing (Sc.6).
- `ASSET_PANEL_MAX_ROWS`, `apply_cap`, the panel `height`, and the block `render`
  are gone; `asset_panel::layout` remains the single composition source the rendered
  lines and the click map both derive from.
- Full suite green; `clippy -D warnings`, `fmt`, comment-policy clean; complexity
  within budget (cyclomatic ≤ 10 / ≤ 8 new); tests mutation-resistant; geometry
  derived from the real rendered buffer.

### Plan

Sliced (see the persisted plan): (1) asset_panel produces the inline section's
styled lines + the pure line→asset-index map (cap/height/block-render retired),
unit-tested behind the existing panel; (2) cut over — `build_detail_content` splices
the section, `draw_detail` renders one scroll region, `model` geometry + asset click
go scroll-aware, fixed-panel path removed. Each slice stays green.

### Resolution

Closed. Delivered in five slices, each reviewed (opus) and green:

- **S1** (`c41ebf3`) — `asset_panel::section_lines` (header + 1:1 `layout()` mapping,
  Bold/Italic emphasis runs) and `asset_index_for_section_row` (row→owning-asset map),
  both deriving from the one `layout()` vector. Additive.
- **S2a** (`85e1b71`) — removed the fixed-panel-coupled tests ahead of the cutover
  (test-only, suite green) so the atomic cutover fit the per-slice file budget.
- **S2b** (`7e11c85`) — the atomic cutover: `build_detail_content` splices
  `section_lines`; `draw_detail` renders one scrollable region; `asset_panel_cmd_at`
  is scroll-aware; `detail_max_offset` / `is_in_body_area` / `body_link_cmd_at` drop
  the panel-height term.
- **S3** (`3dbd099`, `3e6ea6d`) — deleted the dead fixed-panel code, the three
  orphaned `theme` fns, and the obsolete tests.

**Design deviation from the planned scope:** the asset section's start is **recomputed**
on demand (`asset_section_start = lines.len() − section_lines(assets, width).len()`)
rather than stored as a field on `DetailContent` / `Screen::Detail`. Both the render
splice and the click hit-test derive the section from the **same** asset list and the
**same** `asset_panel::inline_content_width(inner_width)`, so they cannot drift — this
kept `build_detail_content`'s signature and the `Screen::Detail` shape unchanged (no
out-of-scope test/construction churn). The `mod.rs` `asset_panel_render_height`
re-export was removed in S2b (when its last caller went) rather than S3.

Final integration: `cargo test` 864 unit + 31 comment-policy = **895 passed**,
warning-free; `clippy -D warnings` / `fmt --check` / comment-policy clean.
