use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Rect},
    widgets::{Block, Borders, Row, StatefulWidget, Table, TableState},
    Frame,
};

/// Render a responsive [`Table`] with a styled header and selection highlight into `frame`.
///
/// `widths` should use `Constraint::Min(0)` for the name/description column so it
/// absorbs remaining width, degrading gracefully on narrow terminals without panicking.
pub fn render_table(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    header: &[&str],
    rows: Vec<Vec<String>>,
    widths: &[Constraint],
    selected: usize,
) {
    let header_row = Row::new(header.to_vec()).style(theme::header_style());

    let data_rows: Vec<Row> = rows.into_iter().map(Row::new).collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_owned());

    let table = Table::new(data_rows, widths)
        .header(header_row)
        .block(block)
        .row_highlight_style(theme::selection_style())
        .highlight_symbol(theme::SELECTION_SYMBOL);

    let mut state = TableState::default();
    state.select(Some(selected));

    StatefulWidget::render(table, area, frame.buffer_mut(), &mut state);
}

#[cfg(test)]
#[path = "../../tests/unit/tui_render.rs"]
mod tests;
