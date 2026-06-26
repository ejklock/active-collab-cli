---
type: ADR
title: TUI visual redesign — vibrant dashboard (user header, unified lists, scrollbar)
description: Evolve the parity-faithful TUI into a vibrant dashboard look — a logged-in-user header bar, a unified browse/mine table design with column headers, responsive columns, a visible scrollbar, accent colors, and sequential "Attachment N" asset labels.
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

Named ANSI bright colors for broad terminal support; exact ratatui `Color`s:

| Role | Style | ratatui |
|---|---|---|
| App header bar (identity banner) | bold white on cyan | `fg White, bg Cyan, BOLD` |
| Block titles | bold light-cyan | `fg LightCyan, BOLD` |
| Column header row | bold light-cyan | `fg LightCyan, BOLD` |
| Selected row | bold black on light-cyan | `fg Black, bg LightCyan, BOLD` |
| Count / badge (e.g. task count, "N total") | bold magenta | `fg Magenta, BOLD` |
| Asset label | yellow | `fg Yellow` (unchanged) |
| Footer hint bar | white on blue | `fg White, bg Blue` (unchanged, Python pair3) |

### 2. Logged-in user header bar

A header block rendered **above** the content area in `view()` (new vertical
layout: header + content + footer). Content per the user's choice **name + email
+ instance**: `"{name} <{email}> · {instance}"`. The **email + instance** come
from the `Instance` struct and paint **immediately**; the **name** is resolved
from `user_map[instance.user_id]` (cached or fetched in the background) and fills
in on a second paint — reusing the existing progressive-paint pattern. Multi-
instance aggregation shows the active/first identity plus a `(+N more)` suffix.

### 3. Unified browse + column headers + responsive columns

`Screen::Projects` adopts the shared table treatment with a styled column header
(`TASKS` · `PROJECT` · `INSTANCE`). `ProjectGroup` gains an `instance` field
(the instance of its tasks) so the column has a value. The flexible `PROJECT`/
`NAME` column stays `Constraint::Min(0)` with the existing `truncate_cell`
ellipsis; fixed columns truncate gracefully. Column titles use `column_header_style`.

### 4. Scrollbar

Use ratatui's `Scrollbar` + `ScrollbarState` (0.29). The Detail content area gets
a vertical scrollbar on the right, driven by `offset` and `lines.len()`, shown
when content exceeds the viewport. The list screens render the same affordance
for long lists.

### 5. Asset labels "Attachment N"

`render_assets_panel` labels become `"{Attachment} {n}: {filename}"` where
`{Attachment}` is the static i18n key `t("Attachment")` (pt_BR → "Anexo") and
`{n}` is the 1-based index matching the `1-9` open shortcut — e.g. `Anexo 1: report.pdf`.
Interpolation stays caller-side because `t()` is a static catalog lookup.

## Alternatives considered

- **Refined (Python-faithful+).** Keep the boxed look, only add the user line and
  column headers. Rejected by the user in favor of more "vibes", but its restraint
  informs the legibility guardrails (keep the white-on-blue footer, avoid 24-bit
  RGB that degrades on basic terminals).
- **Minimal / high-contrast (borderless).** Drop boxes for whitespace + one accent.
  Rejected: the box framing carries useful structure on a dense task list.
- **24-bit RGB themed palette.** Rejected: named bright ANSI colors render
  predictably across the terminals this CLI targets; RGB risks washed-out or
  invisible text on some profiles.
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
