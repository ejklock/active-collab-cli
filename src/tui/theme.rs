//! Central style layer for the TUI.
//!
//! # Design-system token model (ADR 0060)
//!
//! Every visible color in the TUI is named by **role**, not by hue. The role
//! set is small — thirteen tokens — and lives in [`Palette`]. Concrete palettes
//! (`ANGIE_DARK`, `SLATE_DARK`, `NORD_DARK`, and their `_LIGHT` twins) fill those
//! roles with truecolor `Rgb` values. Swapping the whole look is therefore a
//! *values* change (`set_active`), never a code change: the `*_style()` builders
//! below read the active palette and never mention a literal color.
//!
//! This is the single source of truth for consistency — a screen cannot drift,
//! because it reuses these builders instead of constructing `Style` inline.
//!
//! Default palette is **Angie (dark)** — the calm GitHub-dark look the README
//! demo already promises. `Slate & Amber` and `Nord Frost` ship as opt-in
//! choices selectable from Settings (see [`set_active`] / [`ThemeChoice`]).

use crate::tui::model::DueStyle;
use ratatui::style::{Color, Modifier, Style};
use std::sync::RwLock;

/// The thirteen semantic roles every screen draws from.
///
/// `Copy` so a reader can cheaply snapshot the active palette under the lock and
/// release it before building a `Style`.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Page background — the terminal surface everything sits on.
    pub surface_base: Color,
    /// Raised band (identity bar, status bar) — one step off the base.
    pub surface_raised: Color,
    /// Box-drawing rules and separators.
    // Unused until a later slice's builder reads panel border color from tokens (ADR 0060 follow-ups).
    #[allow(dead_code)]
    pub border: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary / muted text (labels, column headers, timestamps).
    pub text_secondary: Color,
    /// The one accent — app name, panel titles, focus.
    pub accent: Color,
    /// "open / a link / done" — also the underlined-link color.
    pub success: Color,
    /// Informational / structural highlight (quotes, original-title marker).
    // Unused until a later slice's builder reads the info token (ADR 0060 follow-ups).
    #[allow(dead_code)]
    pub info: Color,
    /// "due now / soon".
    pub warning: Color,
    /// "overdue / destructive".
    pub danger: Color,
    /// Selection highlight background (the cursor row / selected card).
    pub selection: Color,
    /// Foreground used *on top of* `selection` (kept high-contrast per palette).
    pub selection_fg: Color,
    /// Status-bar hint text — a touch brighter than `text_secondary`.
    pub footer: Color,
}

// Angie — GitHub-dark, refined. Default.
pub const ANGIE_DARK: Palette = Palette {
    surface_base: Color::Rgb(13, 17, 23),
    surface_raised: Color::Rgb(22, 27, 34),
    border: Color::Rgb(48, 54, 61),
    text: Color::Rgb(230, 237, 243),
    text_secondary: Color::Rgb(139, 148, 158),
    accent: Color::Rgb(86, 212, 221),
    success: Color::Rgb(63, 185, 80),
    info: Color::Rgb(88, 166, 255),
    warning: Color::Rgb(210, 153, 34),
    danger: Color::Rgb(248, 81, 73),
    selection: Color::Rgb(210, 153, 34),
    selection_fg: Color::Rgb(13, 17, 23),
    footer: Color::Rgb(173, 186, 199),
};
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub const ANGIE_LIGHT: Palette = Palette {
    surface_base: Color::Rgb(255, 255, 255),
    surface_raised: Color::Rgb(246, 248, 250),
    border: Color::Rgb(208, 215, 222),
    text: Color::Rgb(31, 35, 40),
    text_secondary: Color::Rgb(101, 109, 118),
    accent: Color::Rgb(15, 124, 140),
    success: Color::Rgb(26, 127, 55),
    info: Color::Rgb(9, 105, 218),
    warning: Color::Rgb(154, 103, 0),
    danger: Color::Rgb(207, 34, 46),
    selection: Color::Rgb(191, 135, 0),
    selection_fg: Color::Rgb(255, 255, 255),
    footer: Color::Rgb(87, 96, 106),
};

// Slate & Amber — a warmer evolution of the original retro palette.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub const SLATE_DARK: Palette = Palette {
    surface_base: Color::Rgb(23, 22, 28),
    surface_raised: Color::Rgb(33, 31, 40),
    border: Color::Rgb(52, 49, 61),
    text: Color::Rgb(236, 231, 225),
    text_secondary: Color::Rgb(164, 158, 151),
    accent: Color::Rgb(224, 164, 88),
    success: Color::Rgb(139, 196, 138),
    info: Color::Rgb(127, 180, 230),
    warning: Color::Rgb(224, 164, 88),
    danger: Color::Rgb(229, 112, 107),
    selection: Color::Rgb(224, 164, 88),
    selection_fg: Color::Rgb(23, 22, 28),
    footer: Color::Rgb(201, 194, 186),
};
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub const SLATE_LIGHT: Palette = Palette {
    surface_base: Color::Rgb(250, 248, 245),
    surface_raised: Color::Rgb(240, 236, 229),
    border: Color::Rgb(221, 214, 204),
    text: Color::Rgb(36, 31, 26),
    text_secondary: Color::Rgb(107, 97, 87),
    accent: Color::Rgb(179, 101, 26),
    success: Color::Rgb(63, 126, 63),
    info: Color::Rgb(43, 108, 176),
    warning: Color::Rgb(179, 101, 26),
    danger: Color::Rgb(193, 74, 68),
    selection: Color::Rgb(217, 138, 43),
    selection_fg: Color::Rgb(250, 248, 245),
    footer: Color::Rgb(107, 97, 87),
};

// Nord Frost — cool, low-saturation, easy on the eyes for long sessions.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub const NORD_DARK: Palette = Palette {
    surface_base: Color::Rgb(46, 52, 64),
    surface_raised: Color::Rgb(59, 66, 82),
    border: Color::Rgb(67, 76, 94),
    text: Color::Rgb(236, 239, 244),
    text_secondary: Color::Rgb(169, 177, 196),
    accent: Color::Rgb(136, 192, 208),
    success: Color::Rgb(163, 190, 140),
    info: Color::Rgb(129, 161, 193),
    warning: Color::Rgb(235, 203, 139),
    danger: Color::Rgb(191, 97, 106),
    selection: Color::Rgb(94, 129, 172),
    selection_fg: Color::Rgb(236, 239, 244),
    footer: Color::Rgb(216, 222, 233),
};
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub const NORD_LIGHT: Palette = Palette {
    surface_base: Color::Rgb(236, 239, 244),
    surface_raised: Color::Rgb(229, 233, 240),
    border: Color::Rgb(216, 222, 233),
    text: Color::Rgb(46, 52, 64),
    text_secondary: Color::Rgb(76, 86, 106),
    accent: Color::Rgb(46, 125, 149),
    success: Color::Rgb(79, 122, 63),
    info: Color::Rgb(94, 129, 172),
    warning: Color::Rgb(180, 138, 46),
    danger: Color::Rgb(191, 97, 106),
    selection: Color::Rgb(129, 161, 193),
    selection_fg: Color::Rgb(46, 52, 64),
    footer: Color::Rgb(76, 86, 106),
};

/// A named palette family — the user-facing theme choice.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    Angie,
    Slate,
    Nord,
}

/// Light or dark mode within a [`ThemeChoice`].
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// Resolve a `(theme, mode)` pair to its concrete palette.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub fn palette_for(theme: ThemeChoice, mode: Mode) -> Palette {
    match (theme, mode) {
        (ThemeChoice::Angie, Mode::Dark) => ANGIE_DARK,
        (ThemeChoice::Angie, Mode::Light) => ANGIE_LIGHT,
        (ThemeChoice::Slate, Mode::Dark) => SLATE_DARK,
        (ThemeChoice::Slate, Mode::Light) => SLATE_LIGHT,
        (ThemeChoice::Nord, Mode::Dark) => NORD_DARK,
        (ThemeChoice::Nord, Mode::Light) => NORD_LIGHT,
    }
}

/// Parse a persisted `"theme"` config value (e.g. `"angie"`, `"nord"`).
/// Unknown values fall back to the default (`Angie`).
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub fn theme_from_str(s: &str) -> ThemeChoice {
    match s.trim().to_ascii_lowercase().as_str() {
        "slate" | "slate-amber" | "amber" => ThemeChoice::Slate,
        "nord" | "nord-frost" | "frost" => ThemeChoice::Nord,
        _ => ThemeChoice::Angie,
    }
}

/// Stable string form for persisting a [`ThemeChoice`] to config.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub fn theme_to_str(theme: ThemeChoice) -> &'static str {
    match theme {
        ThemeChoice::Angie => "angie",
        ThemeChoice::Slate => "slate",
        ThemeChoice::Nord => "nord",
    }
}

/// The active palette. Defaults to Angie (dark); the shell overrides it at
/// startup from config, and Settings can change it at runtime (see `set_active`).
static ACTIVE_PALETTE: RwLock<Palette> = RwLock::new(ANGIE_DARK);

/// Snapshot the active palette. Cheap (`Copy`); the read lock is uncontended in
/// practice because draws are single-threaded and theme changes are rare.
fn palette() -> Palette {
    *ACTIVE_PALETTE.read().expect("theme palette lock poisoned")
}

/// Replace the active palette wholesale. Call once at startup and again whenever
/// the user picks a theme in Settings; the next frame paints in the new palette.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub fn set_active_palette(p: Palette) {
    *ACTIVE_PALETTE.write().expect("theme palette lock poisoned") = p;
}

/// Convenience over [`set_active_palette`] using the named theme + mode.
// Unused until slice S3/S4 wires runtime theme selection (ADR 0062).
#[allow(dead_code)]
pub fn set_active(theme: ThemeChoice, mode: Mode) {
    set_active_palette(palette_for(theme, mode));
}

/// Prefix rendered before the selected row in a table.
pub const SELECTION_SYMBOL: &str = "▸ ";

/// Selected row — high-contrast foreground on the selection highlight, bold.
pub fn selection_style() -> Style {
    let p = palette();
    Style::default()
        .fg(p.selection_fg)
        .bg(p.selection)
        .add_modifier(Modifier::BOLD)
}

/// Column header rows and list block titles — accent, bold.
pub fn column_header_style() -> Style {
    Style::default()
        .fg(palette().accent)
        .add_modifier(Modifier::BOLD)
}

/// Status/hint bar — bright hint text on the raised band, bold.
pub fn footer_style() -> Style {
    let p = palette();
    Style::default()
        .fg(p.footer)
        .bg(p.surface_raised)
        .add_modifier(Modifier::BOLD)
}

/// Identity bar at the top of every screen — accent on the raised band, bold.
pub fn app_header_style() -> Style {
    let p = palette();
    Style::default()
        .fg(p.accent)
        .bg(p.surface_raised)
        .add_modifier(Modifier::BOLD)
}

/// URL links embedded in Description and Comment body text — success, underlined.
pub fn link_style() -> Style {
    Style::default()
        .fg(palette().success)
        .add_modifier(Modifier::UNDERLINED)
}

/// Edit affordance on own-comment card headers — accent + underlined.
pub fn edit_affordance_style() -> Style {
    Style::default()
        .fg(palette().accent)
        .add_modifier(Modifier::UNDERLINED)
}

/// Delete affordance on own-comment card headers — destructive + underlined.
pub fn delete_affordance_style() -> Style {
    Style::default()
        .fg(palette().danger)
        .add_modifier(Modifier::UNDERLINED)
}

/// Badge style (selection hue, bold) — retained for theme-consistency tests.
#[allow(dead_code)]
pub fn badge_style() -> Style {
    Style::default()
        .fg(palette().selection)
        .add_modifier(Modifier::BOLD)
}

/// Body text selection highlight — reversed foreground/background (REVERSED).
///
/// Reverse video is terminal-portable and provides visible contrast without
/// requiring foreground/background color knowledge of the underlying cell.
pub fn body_selection_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// Copied feedback indicator in the footer — base on success, bold.
pub fn copied_indicator_style() -> Style {
    let p = palette();
    Style::default()
        .fg(p.surface_base)
        .bg(p.success)
        .add_modifier(Modifier::BOLD)
}

/// Inline code spans in rich text — secondary text, dim.
pub fn code_style() -> Style {
    Style::default()
        .fg(palette().text_secondary)
        .add_modifier(Modifier::DIM)
}

/// Detail footer status row — same band as footer but dim, for transient messages.
pub fn footer_status_style() -> Style {
    let p = palette();
    Style::default()
        .fg(p.footer)
        .bg(p.surface_raised)
        .add_modifier(Modifier::DIM)
}

/// Focused comment card border/line highlight — raised background, bold.
///
/// Applied to every line of the currently-focused comment card in the Detail
/// thread so the cursor is visible without obscuring the card text.
pub fn focused_comment_style() -> Style {
    Style::default()
        .bg(palette().surface_raised)
        .add_modifier(Modifier::BOLD)
}

/// Panel title seated in a panel's top rule (Details / Description / Comments) —
/// accent + bold. Carried structurally as a `RichStyle::PanelTitle` run over the
/// label span of the top-border row (ADR 0063), mirroring how the comment
/// edit/delete affordances style tokens on that same border row.
// Unused until slice S4 wires panel titles into the layout (ADR 0063).
#[allow(dead_code)]
pub fn panel_title_style() -> Style {
    Style::default()
        .fg(palette().accent)
        .add_modifier(Modifier::BOLD)
}

/// Modal overlay border — accent, to match panel titles.
pub fn modal_border_style() -> Style {
    Style::default().fg(palette().accent)
}

/// Modal title text — bold, accent.
pub fn modal_title_style() -> Style {
    Style::default()
        .fg(palette().accent)
        .add_modifier(Modifier::BOLD)
}

/// Modal body text — primary text, default background.
pub fn modal_body_style() -> Style {
    Style::default().fg(palette().text)
}

/// In-box hint/status line — secondary text, dim.
pub fn modal_hint_style() -> Style {
    Style::default()
        .fg(palette().text_secondary)
        .add_modifier(Modifier::DIM)
}

/// Backdrop background for the strongly-dimmed modal overlay.
///
/// Applied to every backdrop cell in addition to `Modifier::DIM` so the thread
/// reads as clearly behind the modal (not merely fg-dimmed / transparent).
pub fn modal_backdrop_style() -> Style {
    Style::default().bg(palette().surface_base)
}

/// Due-date color for a task card's line 2, keyed on the DueStyle variant.
///
/// Overdue -> danger; Near -> warning; Normal or None -> default fg (no override).
/// Callers merge this with the base (selection) background when a card is selected
/// so urgency color stays visible even on the selection highlight.
pub fn due_style(kind: DueStyle) -> Style {
    let p = palette();
    match kind {
        DueStyle::Overdue => Style::default().fg(p.danger),
        DueStyle::Near => Style::default().fg(p.warning),
        DueStyle::Normal | DueStyle::None => Style::default(),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/theme.rs"]
mod theme_tests;
