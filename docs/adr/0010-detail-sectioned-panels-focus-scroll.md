---
type: ADR
title: Detail screen as fixed, independently-scrollable sections (focus + Tab + numeric jump)
description: Restructure the Detail screen from one combined scroll region into fixed bordered sections — Header, Description (body), Comments, Artifacts — each with its own scrollbar, navigated with the industry-standard pane-focus model (Tab/Shift+Tab cycle, 1/2/3 jump, highlighted active border, scroll keys routed to the focused pane).
status: Reverted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, detail, scroll, focus]
timestamp: 2026-06-25T16:30:00Z
---

# 0010. Detail screen as fixed, independently-scrollable sections

> **Reverted (2026-06-26).** This decision was implemented (slice U6b) and
> user-tested, then reverted (slice U6c) at the user's request: the per-section
> focus model (`Tab` cycling + independent per-section scroll) felt heavier than
> wanted for a read view. The Detail screen returned to a **single global scroll**
> over the unified content (header-meta + body + comments via `build_detail_lines`)
> with the fixed Artifacts panel and the single [ADR 0009](/adr/0009-tui-visual-redesign-vibrant-dashboard.md)
> §4 scrollbar. The U6a line builders (`build_header_lines`/`build_body_lines`/
> `build_comment_lines`) were kept — they compose into `build_detail_lines`. This
> record is retained for provenance; the "always one global scroll for a read
> view" lesson stands.

## Context

[ADR 0009](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) slice U3 gave the
Detail screen a fixed **Artifacts** panel (the `Anexo N` list) below a single
scrollable content region. The user liked the fixed-panel affordance and asked to
extend it to the whole screen: **fixed sections for header, body, and comments,
each scrollable on its own when needed**.

Today `build_detail_lines` (`src/render.rs`) composes meta rows + a blank +
Description + a blank + comment boxes into **one** `Vec<String>`, rendered as a
single `Paragraph` driven by **one** `offset` (model `Screen::Detail.offset`,
keys `↑↓` = ±1, `PgUp`/`PgDn` = ±page). One offset cannot independently reach
two long regions (a long body pushes the comments off-screen, or vice-versa).

**How the industry solves "several sections, each scrollable":** the dominant
terminal-UI pattern is **pane focus** — `lazygit` (Status/Files/Branches/Commits/
Stash panes; `Tab` + `←/→` cycle, `1`–`5` jump, active pane gets a highlighted
border, `↑↓`/mouse scroll the focused pane), `k9s`, `gitui`, `tig`, and `tmux`
panes all do this. The single-offset model we have today is the **pager** pattern
(`less`, `man`), which is correct only for a *single* scroll region. The idiomatic
ratatui realization is an `enum Focus`/`active_pane` in the model, key events
routed to the focused pane, and the active border painted in a distinct style.

Force: **navigability of a multi-region read view** — the operator must scan the
fixed header, then independently scroll a long description and a long comment
thread, perceiving which region the keys act on. Presentation-layer only: the pure
TEA core (`model.rs` `update`) stays pure; the change is more model *state*
(focus + per-section offsets) and more *view* regions, not I/O or async.

## Decision

Adopt the **pane-focus** model, delivered as slice **U6**.

### 1. Three line builders (`src/render.rs`)

Split `build_detail_lines` into three public builders that return separate vecs:
`build_header_lines` (meta rows), `build_body_lines` (Description label + wrapped
body), `build_comment_lines` (comment boxes). The existing private
`build_meta_rows` / `build_description_rows` / `build_comments_rows` are reused;
`build_detail_lines` may remain as a thin composition for any non-sectioned caller
or be retired if unused. Each builder stays pure and width-bounded.

### 2. Four stacked bordered sections (`src/tui/screens/detail.rs`)

Vertical layout of `area`:

| Section | Height | Scroll |
|---|---|---|
| **Header** | fixed `= min(header_lines.len() + 2, cap)` | only scrolls in the extreme short-terminal case |
| **Description** | `Min(0)`, shares remaining space ~50/50 | own scrollbar on overflow |
| **Comments** | `Min(0)`, shares remaining space ~50/50 | own scrollbar on overflow |
| **Artifacts** | fixed (existing `min(len+2, 8)`), only when assets present | — |

Each section is its own `Block` + `Paragraph` with the U3 scrollbar mechanism
(`ScrollbarState::new(len).position(section_offset)`, shown only on overflow). The
**focused** section's border uses a highlighted style (`theme::header_style`,
LightCyan); unfocused borders stay default.

### 3. Focus + per-section offsets (`src/tui/model.rs`)

`Screen::Detail` gains a `focus: DetailFocus` field and replaces the single
`offset` with per-section offsets (`body_offset`, `comment_offset`; header offset
only if it can overflow). `DetailFocus` ∈ `{ Body, Comments }` (Header joins only
when it overflows). Key handling in `update`:

- `Tab` / `Shift+Tab` → cycle focus across the scrollable sections present.
- `↑↓` / `PgUp` `PgDn` / `g` `G` → scroll the **focused** section's offset, clamped
  to that section's own length (reusing the existing clamp-on-rebuild logic per
  section).

> **No numeric jump.** The lazygit-style `1`–`3` jump was dropped: the Detail
> screen already binds `1`–`9` to *open asset N* and `d`+`1`–`9` to *download asset
> N* (`view.rs` footer). Reusing those digits for focus jump would collide, so
> `Tab`/`Shift+Tab` is the sole focus-switch affordance.

`reflow_detail` rebuilds the three caches and clamps each offset independently.

### 4. Footer hint

The Detail footer hint gains the focus-switch key (`Tab` switch section),
prepended to the existing `↑/↓ scroll … 1-9 open asset … d+1-9 download` hint,
consistent with the white-on-blue hint bar. The `1-9` digits keep their
open-asset meaning.

## Alternatives considered

- **Keep the single combined offset (status quo).** Rejected: one offset cannot
  independently scroll two long regions; the user explicitly asked for separate
  sections.
- **"Always scroll Comments", body fixed-height.** The pager (single-region)
  pattern. Rejected: a long Description becomes unreachable by keyboard.
- **Accordion / collapse (one section expanded at a time).** Rejected: adds an
  expand/collapse interaction and hides content the user wants visible at a glance.
- **Mouse-over scroll.** Deferred to a future slice: requires enabling crossterm
  mouse capture (a terminal-mode change) and is additive to — not a replacement
  for — keyboard focus.
- **`hjkl` / vim windows instead of Tab.** Rejected as the primary affordance:
  `Tab` matches lazygit/gitui, the closest peers; `g`/`G` is kept.
- **Numeric `1`–`3` focus jump (lazygit-style).** Rejected on a keybinding
  collision: Detail already binds `1`–`9` to open-asset and `d`+`1`–`9` to
  download. `Tab`/`Shift+Tab` is the sole focus switch.

## Consequences

**Positive:** consistent fixed-section look across the whole Detail screen;
independent scroll of body and comments; clear active-pane affordance matching
industry TUIs; reuses the U3 scrollbar and existing clamp logic.

**Accepted trade-offs:** `Screen::Detail` carries more state (focus + per-section
offsets) and `update` grows focus-routing branches (kept within the complexity
budget by extracting a `scroll_focused` helper); `render.rs` exposes three builders
instead of one. The pure TEA core stays presentation-agnostic — focus is model
state mutated by key Msgs, not I/O.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md)
- Architecture: [/architecture.md](/architecture.md)
