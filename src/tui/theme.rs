use crate::tui::model::DueStyle;
use ratatui::style::{Color, Modifier, Style};

const NEAR_BLACK: Color = Color::Rgb(13, 13, 13);
const SOFT_CYAN: Color = Color::Rgb(102, 204, 204);
const STEEL: Color = Color::Rgb(140, 165, 196);
const STEEL_BG: Color = Color::Rgb(38, 52, 74);
const AMBER: Color = Color::Rgb(210, 160, 90);
const MUTED_GREEN: Color = Color::Rgb(120, 190, 130);
const LIGHT_GREY: Color = Color::Rgb(208, 216, 224);
const DUE_RED: Color = Color::Rgb(220, 80, 80);
const DUE_YELLOW: Color = Color::Rgb(210, 180, 60);

/// Prefix rendered before the selected row in a table.
pub const SELECTION_SYMBOL: &str = "▸ ";

/// Selected row — near-black on discreet amber, bold.
pub fn selection_style() -> Style {
    Style::default()
        .fg(NEAR_BLACK)
        .bg(AMBER)
        .add_modifier(Modifier::BOLD)
}

/// Column header rows and list block titles — soft cyan, bold.
pub fn column_header_style() -> Style {
    Style::default().fg(SOFT_CYAN).add_modifier(Modifier::BOLD)
}

/// Status/hint bar — light grey on steel-blue band, bold.
pub fn footer_style() -> Style {
    Style::default()
        .fg(LIGHT_GREY)
        .bg(STEEL_BG)
        .add_modifier(Modifier::BOLD)
}

/// Identity bar at the top of every screen — soft cyan on steel-blue band, bold.
pub fn app_header_style() -> Style {
    Style::default()
        .fg(SOFT_CYAN)
        .bg(STEEL_BG)
        .add_modifier(Modifier::BOLD)
}

/// URL links embedded in Description and Comment body text — muted green, underlined.
pub fn link_style() -> Style {
    Style::default()
        .fg(MUTED_GREEN)
        .add_modifier(Modifier::UNDERLINED)
}

/// Edit affordance on own-comment card headers — soft cyan + underlined.
pub fn edit_affordance_style() -> Style {
    Style::default()
        .fg(SOFT_CYAN)
        .add_modifier(Modifier::UNDERLINED)
}

/// Delete affordance on own-comment card headers — destructive red + underlined.
pub fn delete_affordance_style() -> Style {
    Style::default()
        .fg(DUE_RED)
        .add_modifier(Modifier::UNDERLINED)
}

/// Badge style (amber, bold) — retained for theme-consistency tests.
#[allow(dead_code)]
pub fn badge_style() -> Style {
    Style::default().fg(AMBER).add_modifier(Modifier::BOLD)
}

/// Body text selection highlight — reversed foreground/background (REVERSED modifier).
///
/// Reverse video is terminal-portable and provides visible contrast without
/// requiring foreground/background color knowledge of the underlying cell.
pub fn body_selection_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// Copied feedback indicator in the footer — near-black on muted green, bold.
pub fn copied_indicator_style() -> Style {
    Style::default()
        .fg(NEAR_BLACK)
        .bg(MUTED_GREEN)
        .add_modifier(Modifier::BOLD)
}

/// Inline code spans in rich text — steel, dim.
pub fn code_style() -> Style {
    Style::default().fg(STEEL).add_modifier(Modifier::DIM)
}

/// Detail footer status row — same band as footer but dim, for transient messages.
pub fn footer_status_style() -> Style {
    Style::default()
        .fg(LIGHT_GREY)
        .bg(STEEL_BG)
        .add_modifier(Modifier::DIM)
}

/// Focused comment card border/line highlight — steel-blue background, bold.
///
/// Applied to every line of the currently-focused comment card in the Detail
/// thread so the cursor is visible without obscuring the card text.
pub fn focused_comment_style() -> Style {
    Style::default().bg(STEEL_BG).add_modifier(Modifier::BOLD)
}

/// Modal overlay border — rounded-corner box, soft cyan, to match comment cards.
pub fn modal_border_style() -> Style {
    Style::default().fg(SOFT_CYAN)
}

/// Modal title text — bold, soft cyan.
pub fn modal_title_style() -> Style {
    Style::default().fg(SOFT_CYAN).add_modifier(Modifier::BOLD)
}

/// Modal body text — light grey, default background.
pub fn modal_body_style() -> Style {
    Style::default().fg(LIGHT_GREY)
}

/// In-box hint/status line — steel, dim.
pub fn modal_hint_style() -> Style {
    Style::default().fg(STEEL).add_modifier(Modifier::DIM)
}

/// Backdrop background for the strongly-dimmed modal overlay.
///
/// Applied to every backdrop cell in addition to `Modifier::DIM` so the thread
/// reads as clearly behind the modal (not merely fg-dimmed / transparent).
pub fn modal_backdrop_style() -> Style {
    Style::default().bg(NEAR_BLACK)
}

/// Due-date color for a task card's line 2, keyed on the DueStyle variant.
///
/// Overdue -> red; Near -> yellow; Normal or None -> default fg (no color override).
/// Callers merge this with the base (selection) background when a card is selected
/// so urgency color stays visible even on the amber highlight.
pub fn due_style(kind: DueStyle) -> Style {
    match kind {
        DueStyle::Overdue => Style::default().fg(DUE_RED),
        DueStyle::Near => Style::default().fg(DUE_YELLOW),
        DueStyle::Normal | DueStyle::None => Style::default(),
    }
}
