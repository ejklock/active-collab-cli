use crate::i18n::t;
use crate::render::truncate_cell;
use crate::tui::drawer;
use crate::tui::model::TaskRow;
use ratatui::{
    layout::Constraint,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Draw the Tasks screen (project task list) into `area`.
pub fn draw_tasks(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    project_name: &str,
    tasks: &[TaskRow],
    selected: usize,
    loading: bool,
) {
    let title = format!(" {} ", project_name);

    if loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    let name_width = area.width.saturating_sub(30) as usize;

    let rows: Vec<Vec<String>> = tasks
        .iter()
        .map(|row| {
            vec![
                format!("{}", row.task_number),
                row.instance.clone(),
                truncate_cell(&row.name, name_width),
            ]
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Length(16),
        Constraint::Min(0),
    ];
    let header = [t("TASK#"), t("INSTANCE"), t("NAME")];
    let header_refs: Vec<&str> = header.iter().map(|s| s.as_str()).collect();

    drawer::render_table(frame, area, &title, &header_refs, rows, &widths, selected);
}
