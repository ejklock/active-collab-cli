use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Rect},
    widgets::{
        Block, Borders, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Table, TableState,
    },
    Frame,
};

/// Render a responsive [`Table`] with a styled header and selection highlight into `frame`.
///
/// Each caller builds its own [`Row`] values (with per-row height for multi-line
/// wrapped names). `widths` should use `Constraint::Min(0)` for the name/description
/// column so it absorbs remaining width, degrading gracefully on narrow terminals.
pub fn render_table(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    header: &[&str],
    rows: Vec<Row<'static>>,
    widths: &[Constraint],
    selected: usize,
) {
    let header_row = Row::new(header.to_vec()).style(theme::column_header_style());

    let total_rows = rows.len();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_owned())
        .title_style(theme::column_header_style());

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(block)
        .row_highlight_style(theme::selection_style())
        .highlight_symbol(theme::SELECTION_SYMBOL);

    let mut state = TableState::default();
    state.select(Some(selected));

    StatefulWidget::render(table, area, frame.buffer_mut(), &mut state);

    let visible_capacity = area.height.saturating_sub(2).saturating_sub(1) as usize;
    if total_rows > visible_capacity {
        let mut scrollbar_state = ScrollbarState::new(total_rows).position(selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tui_render.rs"]
mod tests;
