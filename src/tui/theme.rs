use ratatui::style::{Color, Modifier, Style};

/// Prefix rendered before the selected row in a table.
pub const SELECTION_SYMBOL: &str = "▸ ";

/// Style for the currently selected list/table row.
pub fn selection_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::LightCyan)
        .add_modifier(Modifier::BOLD)
}

/// Style for column header rows and list block titles.
pub fn column_header_style() -> Style {
    Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD)
}

/// Style for the table header row.
pub fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Status/hint bar style — white on blue, matching Python pair3 'status'.
pub fn footer_style() -> Style {
    Style::default().fg(Color::White).bg(Color::Blue)
}

/// Style for asset rows in the dedicated Artifacts panel.
pub fn asset_style() -> Style {
    Style::default().fg(Color::Yellow)
}

/// Identity bar at the top of every screen — white on cyan, bold.
pub fn app_header_style() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Badge style for the task-count cell on the Projects screen — magenta, bold.
pub fn badge_style() -> Style {
    Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD)
}
