---
type: ADR
title: TUI visual redesign — vibrant dashboard (user header, unified lists, scrollbar)
description: Evolve the parity-faithful TUI into a vibrant dashboard look — a logged-in-user header bar, a unified browse/mine table design with column headers, responsive columns, a visible scrollbar, accent colors, asset labels rendered as evident links, and Detail content composed of stacked rounded panels in a single scroll.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, design, theme]
timestamp: 2026-06-25T15:00:00Z
---

# 0009. TUI visual redesign — vibrant dashboard

## Context

With Python parity complete ([ADR 0007](/adr/0007-tui-module-structure.md),
[ADR 0008](/adr/0008-async-event-loop-with-eventstream-and-select.md), and the
P1–P4 rendering slices), user testing of the release binary surfaced UX gaps that
go **beyond** the original Python TUI:

- **Browse is visually inconsistent with mine/detail.** The `Projects` screen uses
  an ad-hoc two-column layout (`#` + `Project`) while the `Tasks`/mine list (shared
  `Screen::Tasks`) already renders a styled table. There is no unifying header.
- **No identity context.** Nothing shows *who* you are logged in as (name / email /
  instance) — relevant because browse and mine aggregate across instances.
- **Lists are not obviously responsive** and long content has **no scrollbar**, so
  the user cannot tell there is more to scroll.
- **Flat palette.** The colors are parity-faithful but muted ("could have more vibes").
- **Opaque asset labels.** The detail assets panel shows raw filenames; the user
  asked for sequential `Attachment 1..N` (pt-BR "Anexo 1..N") labels aligned with
  the `1-9 open asset` shortcut.

Force: **UX quality of a multi-instance terminal client** — the operator must see
their identity, scan lists consistently, perceive scroll affordance, and enjoy a
livelier but still legible palette. This is a presentation-layer evolution; the
pure TEA core (`model.rs` `update`) and the network/cache layers are unchanged
except for surfacing already-available data (the logged-in user's display name,
resolved via the existing `fetch_user_map` / `user_map[user_id]` machinery).

## Decision

Adopt a **vibrant dashboard** visual system (chosen by the user over a *refined
Python-faithful* and a *minimal high-contrast* alternative), delivered as five
vertical, independently demoable slices.

### 1. Palette (`src/tui/theme.rs`)

Truecolor RGB — a **sober cool-retro** palette (soft cyan + steel-blue + amber
on near-black). The palette first shipped as a brighter 80s-synthwave set and was
toned down to this sober variant after user testing (U8); named `Color::Rgb`
constants, exact ratatui `Color`s:

| Role | Style | ratatui |
|---|---|---|
| App header bar (identity banner) | bold soft-cyan on steel-blue | `fg Rgb(102,204,204), bg Rgb(38,52,74), BOLD` |
| Block titles / column header row | bold soft-cyan | `fg Rgb(102,204,204), BOLD` |
| Table header row | bold steel | `fg Rgb(140,165,196), BOLD` |
| Selected row | bold near-black on amber | `fg Rgb(13,13,13), bg Rgb(210,160,90), BOLD` |
| Count / badge (e.g. task count) | bold amber | `fg Rgb(210,160,90), BOLD` |
| Asset label (link affordance) | muted green, underlined | `fg Rgb(120,190,130), UNDERLINED` |
| Footer hint bar | light-grey on steel-blue | `fg Rgb(208,216,224), bg Rgb(38,52,74), BOLD` |

### 2. Logged-in user header bar

A header block rendered **above** the content area in `view()` (new vertical
layout: header + content + footer). Content per the user's choice **name + email
+ instance**: `"{name} <{email}> · {instance}"`. The **email + instance** come
from the `Instance` struct and paint **immediately**; the **name** is resolved
from `user_map[instance.user_id]` (cached or fetched in the background) and fills
in on a second paint — reusing the existing progressive-paint pattern. Multi-
instance aggregation shows the active/first identity plus a `(+N more)` suffix.

### 3. Unified browse + column headers + responsive columns

`Screen::Projects` adopts the shared table treatment with a two-column styled
header (`TASKS` · `PROJECT`). `ProjectGroup` retains its `instance` field for
internal grouping (multiple instances can coexist in the same browse list), but
the Instance column is **not rendered** — it was removed in U9 to free horizontal
space for the project/task-name column. The flexible `PROJECT`/`NAME` column
stays `Constraint::Min(0)` with `truncate_cell` ellipsis; the count badge column
is a fixed `Constraint::Length`. Column titles use `column_header_style`.

The Tasks-in-project screen likewise drops its `INSTANCE` column and renders
two columns: `TASK#` · `NAME`. Width math: `name_width = area.width - TASK_NUM_WIDTH - OVERHEAD`
where `OVERHEAD = 5` (2 borders + 2 selection-symbol chars + 1 inter-column separator).
For the Projects screen: `project_width = area.width - TASKS_WIDTH - FIXED_OVERHEAD`
where `FIXED_OVERHEAD = 5` (same accounting). Both screens truncate with an ellipsis
on narrow terminals and show more of the name on wider terminals (responsive).

### 4. Scrollbar

Use ratatui's `Scrollbar` + `ScrollbarState` (0.29). The Detail content area gets
a vertical scrollbar on the right, driven by `offset` and `lines.len()`, shown
when content exceeds the viewport. The list screens render the same affordance
for long lists. The Detail scroll **clamps the effective offset** at render time
to `lines.len() - viewport_height` (D2) so the last line of content stays anchored
at the bottom — scrolling can never reveal blank space past the content end. The
clamp is computed once in `render_content` and feeds both `Paragraph::scroll` and
the scrollbar `position`.

### 5. Asset labels as evident links (U11)

`render_assets_panel` renders each asset through the pure `render::asset_link_line(index, asset)`
helper as `"[{n}] ↗ {label}"`, styled with `asset_style` (muted green, underlined)
so it reads as a link. `{n}` is the 1-based index matching the `1-9` open shortcut.
`{label}` is the asset name only when it looks like a real filename
(`looks_like_filename`: non-empty, ≤48 chars, ends in a `.<ext>` of 1–6
alphanumerics); otherwise — empty or an ugly URL fragment — it falls back to the
i18n `t("Open link")` (pt_BR → "Acessar link"). The long raw URL is never shown.
This replaced the earlier `"{Attachment} {n}: {filename}"` form, which surfaced
unreadable URL basenames for hyperlink-derived assets.

### 6. Detail content as stacked rounded panels (U10)

The Detail screen's scrollable content is composed of stacked rounded-border
panels instead of flat labelled lines, all inside the **single global scroll**
(one `offset`, `lines: Vec<String>`, every line ≤ `inner_width` so the scroll
offset never desyncs). A single primitive `render::panel_box(label, inner_lines, width)`
draws every box (rounded `╭ ╮ ╰ ╯ ─ │`, the `label` embedded in the top border,
body laid out by **display columns**). Every box line is padded/truncated to
exactly `width` **terminal display columns** via `render::fit_to_display_width`
(unicode-width — the same crate ratatui renders with, D1), so the right border
**always closes** even when the body holds wide CJK, ambiguous glyphs (e.g. ◆),
or decomposed accents; `wrap_text` likewise wraps by display width. Each box also
carries internal padding (`PANEL_HPAD`/`PANEL_VPAD`, D1) so text never sits flush
against the borders. `comment_box` is a thin delegate over `panel_box`.
`build_detail_lines` composes, in order:

1. a **Details** panel (`t("Details")` → "Detalhes") holding a 2-column aligned
   meta table (Task `id-id`, Project, Status, Assignee, optional Start/Due,
   Estimate, Logged);
2. a **Description** panel (`t("Description")`) with the wrapped body or
   `(no description)`;
3. a **Comments (N)** panel (`t("Comments")`) whose body is the nested per-comment
   `comment_box` cards (indented one space, blank-separated) — omitted when there
   are no comments.

The task **name is promoted to the Detail frame border title** (D2) — the old
` Task #<id> ` title and the centered title band are gone (the id still shows in
the Details meta table). The view threads the name from the stored `task` value
into `draw_detail` via `DetailParams`; a long name is truncated with an ellipsis
(display-width-aware), falling back to ` #<id> ` while the name is not yet loaded.

The Artifacts/assets section stays the **fixed** panel below the scroll (it is not
folded into the global scroll). `draw_detail` and the outer content frame (which
hosts the scrollbar) are unchanged — the panels live entirely in the pre-built
`lines`, keeping the renderer dumb and the layout logic pure and unit-tested.

## Alternatives considered

- **Refined (Python-faithful+).** Keep the boxed look, only add the user line and
  column headers. Rejected by the user in favor of more "vibes", but its restraint
  informs the legibility guardrails (keep the white-on-blue footer, avoid 24-bit
  RGB that degrades on basic terminals).
- **Minimal / high-contrast (borderless).** Drop boxes for whitespace + one accent.
  Rejected: the box framing carries useful structure on a dense task list.
- **24-bit RGB themed palette.** Initially rejected in favor of named ANSI, then
  **adopted in U8**: modern target terminals handle truecolor well, and the user
  wanted more "vibe". The first synthwave (neon magenta/purple) cut was toned down
  to a sober cool-retro RGB palette after user feedback, keeping legibility.
- **Fetch a dedicated `users/me` endpoint for the header name.** Rejected: the
  `user_map` (id→name) is already fetched/cached for the assignee line; reusing it
  avoids a second round-trip and a new client method.

## Consequences

**Positive:** consistent browse/mine/detail look; visible identity and scroll
affordance; livelier yet legible palette; clearer asset labels. No new network
calls (header name reuses `user_map`).

**Accepted trade-offs:** `view()` grows a third layout region (header); `ProjectGroup`
gains an `instance` field (and `build_groups` sets it); a new pt_BR catalog key.
The pure TEA core stays presentation-agnostic — the header identity is data the
shell threads into the model, not logic in `update`.

## Related

- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md)
- ADR: [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md)
- Architecture: [/architecture.md](/architecture.md)
