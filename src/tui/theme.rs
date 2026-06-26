use ratatui::style::{Color, Modifier, Style};

/// Prefix rendered before the selected row in a table.
pub const SELECTION_SYMBOL: &str = "▸ ";

/// Style for the currently selected list/table row.
pub fn selection_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
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
