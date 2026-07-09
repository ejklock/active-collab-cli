---
type: ADR
title: Theme selection — persisted config, env override, and `ac setup theme`
description: Wire the three palettes from ADR 0060 to the user. Persist the choice in the existing settings key/value table under `theme`, allow an `ACTIVE_COLLAB_THEME` env override, apply it at startup via `theme::set_active` (mirroring the language init path), and add an `ac setup theme [angie|slate|nord]` command that mirrors `setup language`. The interactive Settings-screen row is deferred until that screen lands on main.
status: Accepted
supersedes:
superseded_by:
tags: [tui, ux, ratatui, design-system, theme, config, cli, i18n]
timestamp: 2026-07-06T00:00:00Z
---

# 0062. Theme selection — config, env, and `ac setup theme`

## Context

ADR 0060 shipped three palettes (`Angie` default, `Slate`, `Nord`), each in dark
and light, plus `theme::set_active`, `theme::theme_from_str` and
`theme::theme_to_str`. What is missing is the seam that lets a user *pick* one and
have it persist — the palette is currently always the compiled-in default.

The codebase already has the exact pattern to copy: **language selection.**

- `settings` is a flat key/value table (`src/store/settings.rs`,
  `SettingsRepository::get/set`).
- `init_language()` in `src/main.rs` reads `ACTIVE_COLLAB_LANG` (env) then the DB
  `language` setting, resolves, and applies it — called once at startup.
- `setup_language()` in `src/commands/setup.rs` shows or sets the value with
  validation, wired as `ac setup language [code]`.

Note: an interactive Settings *screen* is mentioned in the CHANGELOG/README, but
there is **no `Settings` code in `src/tui/` on `main`** today. So this ADR wires
theme selection through the parts that exist (config + env + command); the
in-TUI Theme row is a deferred step (below).

## Decision

Mirror the language path exactly, one layer down (palette instead of catalog).

### 1. Persistence + env override

Store the choice under settings key **`theme`** (values `angie` / `slate` /
`nord`). Honor an `ACTIVE_COLLAB_THEME` env override, env winning over DB — the
same precedence `init_language` uses.

### 2. Apply at startup — `init_theme()` in `main.rs`

Add `init_theme()` mirroring `init_language()`: read env then DB, resolve via
`theme::theme_from_str`, and call `theme::set_active(choice, Mode::Dark)`. Call it
immediately after each `init_language()` call site. Dark is the default mode; a
future `theme_mode` key can extend this to light without touching call sites.

### 3. `ac setup theme [angie|slate|nord]`

Add `setup_theme()` in `commands/setup.rs`, a line-for-line analogue of
`setup_language()`: no argument prints the current theme; an argument validates
against the three names and persists it (`exit 2` on unsupported, mirroring the
language error contract, BDR 0003). Wire `SetupCmd::Theme(ThemeArgs)` in the CLI
and a `dispatch_setup_theme` in `main.rs`, both mirroring the language variants.

### 4. i18n keys

Add to every locale catalog: `"Current theme: {code}"`, `"Theme set to '{code}'."`,
`"Error: unsupported theme '{code}'. Supported: {list}."`.

## Deferred — interactive Settings "Theme" row

When the interactive Settings screen exists on `main`, add a **Theme** picker row
next to Language/Active-instance. On change it calls
`theme::set_active(choice, Mode::Dark)` (repaints next frame) and persists via
`SettingsRepository::set("theme", theme::theme_to_str(choice))` — the same handler
shape as the Language row. Left out here because there is no such screen to patch
yet.

## Alternatives considered

- **A `~/.config` TOML file for the theme.** Rejected: the settings table is the
  established home for flat prefs (language lives there); a second config surface
  would fragment configuration.
- **Auto-detect terminal dark/light (`COLORFGBG`/OSC 11).** Deferred: useful for
  choosing Mode, but orthogonal to picking a palette family and unreliable across
  terminals. A separate enhancement.

## Consequences

**Positive:** users can pick a palette and it sticks; env override eases
demos/screenshots; zero new dependencies; the command mirrors an existing,
tested one.

**Accepted trade-offs:** the in-TUI Theme row waits on the Settings screen. Until
then the theme is set via `ac setup theme` or the env var, which is sufficient to
ship all three palettes.

## Related

- ADR: [/adr/0060-design-system-semantic-tokens.md](/adr/0060-design-system-semantic-tokens.md) (the palettes this selects)
- ADR: [/adr/0005-i18n-catalog-as-embedded-json.md](/adr/0005-i18n-catalog-as-embedded-json.md) (catalog the new keys join)
- BDR: [/bdr/0003-cli-command-output-parity.md](/bdr/0003-cli-command-output-parity.md) (exit-code contract the command follows)
