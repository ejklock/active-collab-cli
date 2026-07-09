---
type: ADR
title: Semantic design-system tokens with swappable palettes (Angie / Slate / Nord, dark + light)
description: Replace the ad-hoc `const` colors in theme.rs with a role-named token layer (`Palette`) and three concrete palettes in dark and light. Every `*_style()` builder reads the active palette instead of a literal color, so a theme is a values change, not a code change. Ships Angie (GitHub-dark) as the default — the look the README demo already promises — with Slate & Amber and Nord Frost as opt-in choices selectable from Settings.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, design-system, theme, tokens, color]
timestamp: 2026-07-06T00:00:00Z
---

# 0060. Semantic design-system tokens with swappable palettes

## Context

The demo SVG shipped in the README reads as calm, coherent and modern. The
running TUI — built on the ADR 0009 "vibrant dashboard" palette — does not: same
data, same layout, but busier and muddier. The gap is **not** structural; it is
entirely color, contrast and naming.

Two problems in `theme.rs` as it stands:

- **Colors are named by hue, not role.** `SOFT_CYAN`, `STEEL`, `AMBER`,
  `MUTED_GREEN` are module-level `const`s referenced directly by each
  `*_style()` builder. Re-theming means editing every builder; there is no way
  to offer the user an alternative look, and no single place that answers "what
  is the accent color?"
- **The retro palette (cyan / steel / amber on near-black) reads muddy** against
  the terminal's own background and does not match what the README leads users
  to expect.

The operator asked to (a) formalize a design system for the TUI and (b) close
the gap between the README's promise and the running app — grounded in what
`ratatui` can actually draw (truecolor cells, box-drawing, modifiers; no
gradients, shadows or sub-cell geometry).

Force: **consistency and re-themeability** — one source of truth for color, and
the ability to swap the whole look as a values change.

## Decision

Introduce a **role-named token layer** in `theme.rs` and drive every style
builder from the currently-active palette.

### 1. Thirteen semantic tokens (`Palette`)

Colors are named by **role**, never by hue:

`surface_base`, `surface_raised`, `border`, `text`, `text_secondary`, `accent`,
`success`, `info`, `warning`, `danger`, `selection`, `selection_fg`, `footer`.

`Palette` is a `Copy` struct of `ratatui::style::Color` values. Each `*_style()`
builder reads the active palette and references a token — it never mentions a
literal `Rgb`. This makes consistency structural: a screen cannot drift, because
it reuses the same builder.

### 2. Three palettes, dark + light

Six concrete `const Palette`s fill the roles:

- **Angie** (`ANGIE_DARK` / `ANGIE_LIGHT`) — GitHub-dark, refined. Calm neutrals,
  one cyan accent, semantic green / amber / red. **The default.**
- **Slate & Amber** (`SLATE_DARK` / `SLATE_LIGHT`) — a warmer evolution of
  today's retro palette: amber primary, warm greys, softened semantics.
- **Nord Frost** (`NORD_DARK` / `NORD_LIGHT`) — cool, low-saturation, frost-blue
  accent, for long sessions.

All three share the exact token structure, so the picker is a one-liner and no
screen code knows which palette is live.

### 3. Active palette + runtime swap

A process-wide `RwLock<Palette>` holds the active palette, defaulting to
`ANGIE_DARK`. The shell sets it once at startup from config
(`set_active(theme, mode)`); Settings can change it at runtime and the next
frame repaints. `palette_for`, `theme_from_str` / `theme_to_str` handle
resolution and persistence.

### 4. Semantic color has one meaning each

`warning` = "due now / soon". `danger` = "overdue / destructive". `success` =
"open / a link / done". `info` = structural highlight (e.g. quoted original
title). A semantic color is **never** reused for decoration — that is the rule
that keeps the palette readable. `due_style` maps `DueStyle::Overdue → danger`,
`Near → warning`, else default.

### 5. Drop-in: every existing builder signature is preserved

`selection_style`, `column_header_style`, `footer_style`, `app_header_style`,
`link_style`, `edit_affordance_style`, `delete_affordance_style`, `badge_style`,
`body_selection_style`, `copied_indicator_style`, `code_style`,
`footer_status_style`, `focused_comment_style`, the five `modal_*` builders,
`due_style`, and `SELECTION_SYMBOL` keep their exact signatures. No call site in
`view.rs`, `detail_render.rs`, `footer.rs`, or the screens changes. Only the
*bodies* change — from a literal `const` to a token read. This is the whole of
the required change and the lowest-risk, highest-value slice.

## Alternatives considered

- **Keep hue-named consts, just recolor them.** Rejected: fixes the muddiness
  but not the re-themeability; the user still cannot choose a look and there is
  still no single accent definition.
- **`OnceLock` (set-once) instead of `RwLock`.** Rejected: Settings offers a
  live Theme row (§3), which needs a runtime swap; a set-once cell forbids it.
- **A `Theme` value threaded through every draw call as an argument.** Rejected
  as a large, invasive change to every function signature for no user-visible
  benefit over a process-wide active palette; draws are single-threaded so the
  uncontended read lock is effectively free.
- **Per-widget inline `Style`.** Rejected — that is the drift the token layer
  exists to prevent.

## Consequences

**Positive:** one source of truth for color; the app matches the README's
promise; three ready palettes and a light mode; re-theming is a values change;
no call site churn.

**Accepted trade-offs:** a process-wide `RwLock` is shared mutable state (bounded
to `theme.rs`, written only at startup and on the Settings toggle). A poisoned
lock panics — acceptable for a style read that has no meaningful fallback.

## Follow-ups (separate, explicit decisions — not in this ADR)

The token swap is color only. Two structural refinements the design system
recommends are deliberately **out of scope** here and each warrants its own ADR:

- **Legend panels over border-in-border.** Squared panels whose title sits in
  the top rule, stacked directly on the surface, replacing the outer content
  frame + nested rounded panels. A `render/` change, not a `theme.rs` change.
- **Settings "Theme" row** wiring §3 to a persisted config key + `ac setup theme`.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) (the palette this amends)
- ADR: [/adr/0010-detail-sectioned-panels-focus-scroll.md](/adr/0010-detail-sectioned-panels-focus-scroll.md) (panels the legend follow-up would restyle)
- ADR: [/adr/0026-task-list-as-cards.md](/adr/0026-task-list-as-cards.md) (card selection styling)
- ADR: [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md) (footer this recolors)
- Architecture: [/architecture.md](/architecture.md)
