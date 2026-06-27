use crate::i18n::t;
use crate::tui::drawer;
use crate::tui::model::{ClickTarget, TaskRow};
use ratatui::{
    layout::Constraint,
    text::{Line, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row},
    Frame,
};

/// 2 borders + 2 selection-symbol chars = 4.
const OVERHEAD: u16 = 4;

/// Draw the Tasks screen (project task list) into `area`.
#[allow(clippy::too_many_arguments)]
pub fn draw_tasks(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    project_name: &str,
    tasks: &[TaskRow],
    selected: usize,
    loading: bool,
    revalidating: bool,
    targets: &mut Vec<ClickTarget>,
) {
    let title = if revalidating {
        format!(" {} ↻ ", project_name)
    } else {
        format!(" {} ", project_name)
    };

    if loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    let name_width = area.width.saturating_sub(OVERHEAD) as usize;

    let mut row_heights: Vec<u16> = Vec::with_capacity(tasks.len());
    let rows: Vec<Row<'static>> = tasks
        .iter()
        .map(|row| {
            let lines = crate::render::wrap_text(&row.name, name_width.max(1));
            let lines = if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            };
            let height = lines.len() as u16;
            row_heights.push(height);
            let cell = Cell::from(Text::from(
                lines.into_iter().map(Line::from).collect::<Vec<_>>(),
            ));
            Row::new(vec![cell]).height(height)
        })
        .collect();

    let widths = [Constraint::Min(0)];
    let header = [t("NAME")];
    let header_refs: Vec<&str> = header.iter().map(|s| s.as_str()).collect();

    drawer::render_table(
        frame,
        area,
        &title,
        &header_refs,
        rows,
        &widths,
        selected,
        &row_heights,
        targets,
    );
}
