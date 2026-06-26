use ratatui::style::{Color, Modifier, Style};

const NEAR_BLACK: Color = Color::Rgb(13, 13, 13);
const SOFT_CYAN: Color = Color::Rgb(102, 204, 204);
const STEEL: Color = Color::Rgb(140, 165, 196);
const STEEL_BG: Color = Color::Rgb(38, 52, 74);
const AMBER: Color = Color::Rgb(210, 160, 90);
const MUTED_GREEN: Color = Color::Rgb(120, 190, 130);
const LIGHT_GREY: Color = Color::Rgb(208, 216, 224);

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

/// Table header row — steel blue, bold.
pub fn header_style() -> Style {
    Style::default().fg(STEEL).add_modifier(Modifier::BOLD)
}

/// Status/hint bar — light grey on steel-blue band, bold.
pub fn footer_style() -> Style {
    Style::default()
        .fg(LIGHT_GREY)
        .bg(STEEL_BG)
        .add_modifier(Modifier::BOLD)
}

/// Style for asset rows in the dedicated Artifacts panel — muted green, underlined.
pub fn asset_style() -> Style {
    Style::default()
        .fg(MUTED_GREEN)
        .add_modifier(Modifier::UNDERLINED)
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

/// Badge style (amber, bold) — retained for theme-consistency tests.
#[allow(dead_code)]
pub fn badge_style() -> Style {
    Style::default().fg(AMBER).add_modifier(Modifier::BOLD)
}

/// Selection mode indicator in the footer — near-black on amber, bold.
///
/// Matches the row selection palette so the indicator is visually cohesive.
pub fn selection_indicator_style() -> Style {
    Style::default()
        .fg(NEAR_BLACK)
        .bg(AMBER)
        .add_modifier(Modifier::BOLD)
}

/// Inline code spans in rich text — steel, dim.
pub fn code_style() -> Style {
    Style::default().fg(STEEL).add_modifier(Modifier::DIM)
}
