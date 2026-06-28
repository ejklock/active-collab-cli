---
type: ADR
title: Asset-row link styling is structural (a RichStyle::Link run emitted by the layout), not text-pattern URL detection
description: After ADR 0023 derived the Anexos/Artefatos label from anchor text / real filename / host, the inline asset row reads `[n] ↗ report.pdf` — it no longer contains a `[URL]` token or a raw URL, so link_segments() finds nothing to style and the row renders as plain text (and reads as not clickable). Fix the seam, not the symptom: the asset row's linkness is structural (the layout knows it is an asset), so section_lines() emits a RichStyle::Link run over the asset token and the RichStyle→Style mapping renders it with theme::link_style(). Text-pattern URL detection stays for body links (ADR 0020), where a URL token genuinely is present.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, assets, richtext, link, regression, ratatui]
timestamp: 2026-06-28T00:00:00Z
---

# 0032. Asset-row link styling is structural, not text-pattern URL detection

## Context

After [ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md) moved the
Anexos/Artefatos assets inline into the globally-scrollable detail content, each asset
renders as a content row `[n] ↗ {label}` (`asset_link_line`, `src/render.rs:426`). The
intended visual contract ([BDR 0022](/bdr/0022-assets-inline-scrollable-detail-content.md)
Scenario 1) is that the asset token is **visibly link-styled** (muted green + underline,
`theme::link_style`) and opens on Ctrl/Cmd+click ([ADR 0025](/adr/0025-asset-activation-ctrl-cmd-click.md)).

The styling was implemented **implicitly**, by routing every detail content line through
`link_segments()` (`src/render.rs:304`) and letting `styled_line_with_runs`
(`src/tui/screens/detail.rs:96`) apply `link_style()` to any segment `link_segments`
flagged as a link. `link_segments` flags a span as a link only when it finds a `[INNER]`
token whose INNER is a URL/email, or a raw `http(s)` URL.

That implicit coupling broke when [ADR 0023](/adr/0023-asset-label-derivation.md) (commit
`3a961d6`) made the asset label **derived** — anchor text, then a real filename, then the
host. The asset row is now `[1] ↗ report.pdf`:

- `[1]` is a bracket token, but INNER `1` is not a URL/email → not flagged.
- `report.pdf` (or a bare host like `example.com`) is not a raw `http(s)` URL → not flagged.
- The real `asset.url` is **not present in the row at all** — only the derived label is.

So `link_segments` finds **no** link span, `styled_line_with_runs` applies no link style,
and the asset row renders as **plain text** — a regression against BDR 0022. Because the
row carries no visible link affordance and opening requires the Ctrl/Cmd modifier, it also
*reads* as "not clickable" even though the click hit-test (`asset_panel_cmd_at`,
`src/tui/model.rs`) is itself structurally intact.

The root problem is the **seam**: an asset row's "linkness" is a **structural** fact (the
layout knows the row is an asset — `PanelRow::Asset`), but it was being **re-derived from
the rendered text** by URL pattern-matching. Once the text stopped containing a URL, the
derivation failed silently.

## Decision

Style asset rows as links **structurally**, the same way the panel already styles its
header (Bold) and hint (Italic) — via an explicit `StyleRun` emitted by the pure layout,
not by re-detecting a URL in the rendered text.

1. **Add a `RichStyle::Link` variant** (`src/richtext.rs`) to the emphasis enum, alongside
   `Bold`/`Italic`/`Code`/`Strike`/`Underline`.
2. **Map `RichStyle::Link → theme::link_style()`** (muted green + underline) at the single
   place `RichStyle` is converted to a ratatui `Style` for content runs (the run-styling
   path used by `styled_line_with_runs` / `split_segment_by_runs`).
3. **Emit the run in `section_lines()`** (`src/tui/screens/asset_panel.rs`): for each
   `PanelRow::Asset`, attach a `StyleRun { start: PANEL_HPAD, len: display_width(text),
   style: RichStyle::Link }` over the asset token — exactly mirroring how `Hint` already
   emits an `Italic` run over its text. The asset row's empty-`StyleRun` vec
   (`asset_panel.rs:96`) and the now-inaccurate doc comment ("link color is applied at
   render time via `asset_index_for_section_row`") are replaced by this real mechanism.

Body links (description/comment URLs, [ADR 0020](/adr/0020-body-links-inline-url-native-click.md))
**keep** the `link_segments` text-detection path — there a URL token genuinely is present
in the text, so detection is correct and structural information is unavailable. The two
mechanisms are complementary: structural styling for the layout-owned asset rows,
text-detection for inline body URLs. `styled_line_with_runs` already composes both (an
asset row has no detected URL segment, so only the `Link` run applies; a body line has no
`Link` run, so only the detected-URL path applies) — no conflict.

### Guard / fitness function

- **Render styling:** a unit test (`tests/unit/tui_render.rs`) asserts that an inline asset
  row's cells carry `theme::link_style()` (fg muted green + UNDERLINED) over the asset
  token — buffer-derived, so it pins the observable color/underline, not an internal flag.
- **Click still opens (end-to-end):** a model test (`tests/unit/model.rs`) asserts that a
  Ctrl/Cmd+click on an inline asset content row emits `Cmd::OpenAsset` with the correct
  url — closing the gap the Explorer found (no such test existed), and proving the
  "not clickable" symptom was the missing affordance, not a broken hit-test.
- **Behavior preserved elsewhere:** body-link styling/click tests and the existing
  asset-geometry tests stay green unchanged.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean;
  complexity within budget.

## Alternatives considered

- **Render-time asset-row detection** (map each content row through
  `asset_index_for_section_row` at render and style asset rows there). Rejected as the
  primary seam: it spreads the asset-vs-not-asset bookkeeping into the renderer (the
  detail content is a flat `Vec<line>`; the renderer would need the asset offset map the
  click path already owns). Emitting a `StyleRun` from the layout keeps the styling
  decision co-located with the layout that knows it is an asset — one home for the fact.
- **Put the URL back into the visible row** so `link_segments` keeps working. Rejected: it
  undoes ADR 0023 (derived labels exist precisely so the reader sees a friendly name, not
  a raw URL), and it keeps the fragile "style is whatever the text pattern happens to
  match" coupling that caused this regression.
- **Leave it** (assets are plain text + an italic hint says "Ctrl/Cmd+click to open").
  Rejected: BDR 0022 Scenario 1 requires the token to be visibly link-styled; without the
  affordance the feature reads as broken (the reported bug).

## Consequences

**Positive:** asset rows are visibly link-styled again (matching body links), the styling
is robust to any label content (filename, host, anchor text) because it no longer depends
on a URL appearing in the text, and the "linkness" fact now has one home (the layout).
Adds one `RichStyle` variant and one mapping arm.

**Accepted trade-offs:** `RichStyle::Link` carries a fixed appearance (`link_style`); if a
future need arises for per-link colors it would need a richer style carrier — out of scope
here. The asset row and body-URL paths remain two mechanisms (structural vs text-detected);
this is intentional (they have different information available) and documented here so it
is not "simplified" back into one fragile path.

## Related

- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (moved assets inline)
- ADR: [/adr/0023-asset-label-derivation.md](/adr/0023-asset-label-derivation.md) (derived label — the change that silently broke text-pattern styling)
- ADR: [/adr/0025-asset-activation-ctrl-cmd-click.md](/adr/0025-asset-activation-ctrl-cmd-click.md) (Ctrl/Cmd+click to open)
- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (body links — the text-detection path that stays)
- BDR: [/bdr/0022-assets-inline-scrollable-detail-content.md](/bdr/0022-assets-inline-scrollable-detail-content.md) (the visible-link + click behavior this restores)
- Issue: [/issues/0031-fix-inline-asset-link-style-click.md](/issues/0031-fix-inline-asset-link-style-click.md)
