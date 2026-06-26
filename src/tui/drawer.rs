use crate::tui::model::ClickTarget;
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
///
/// `row_heights` must have the same length as `rows` and carry each row's line count
/// (the same value passed to `Row::height`). After rendering, the function reads the
/// post-render scroll offset from `TableState::offset()` and uses it together with
/// `row_heights` to populate `targets` with the visible rows' terminal y-ranges.
#[allow(clippy::too_many_arguments)]
pub fn render_table(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    header: &[&str],
    rows: Vec<Row<'static>>,
    widths: &[Constraint],
    selected: usize,
    row_heights: &[u16],
    targets: &mut Vec<ClickTarget>,
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

    record_click_targets(area, &state, row_heights, targets);

    let visible_capacity = area.height.saturating_sub(2).saturating_sub(1) as usize;
    if total_rows > visible_capacity {
        let mut scrollbar_state = ScrollbarState::new(total_rows).position(selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Walk the visible rows after rendering and record each row's terminal y-range.
///
/// `area.y + 2` is the first data row (1 border + 1 header). Rows are walked
/// from the scroll offset reported by `TableState::offset()` (the first visible
/// row index) until the area's bottom border is reached.
fn record_click_targets(
    area: Rect,
    state: &TableState,
    row_heights: &[u16],
    targets: &mut Vec<ClickTarget>,
) {
    targets.clear();

    let first = state.offset();
    if first >= row_heights.len() {
        return;
    }

    let data_top = area.y + 2;
    let bottom_border = area.y + area.height.saturating_sub(1);

    let mut y = data_top;
    for (i, &h) in row_heights.iter().enumerate().skip(first) {
        if y >= bottom_border {
            break;
        }
        let y_end = (y + h).min(bottom_border);
        targets.push(ClickTarget {
            y_start: y,
            y_end,
            index: i,
        });
        y += h;
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tui_render.rs"]
mod tests;
