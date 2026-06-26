use crate::i18n::t;
use crate::render::truncate_cell;
use crate::tui::drawer;
use crate::tui::model::ProjectGroup;
use ratatui::{
    layout::Constraint,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Draw the Projects screen into `area`.
pub fn draw_projects(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    groups: &[ProjectGroup],
    selected: usize,
    loading: bool,
) {
    let title = t(" Projects ");

    if loading {
        let msg = Paragraph::new(t("Loading tasks…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    let name_width = area.width.saturating_sub(12) as usize;

    let rows: Vec<Vec<String>> = groups
        .iter()
        .map(|g| {
            vec![
                format!("{}", g.tasks.len()),
                truncate_cell(&g.project_name, name_width),
            ]
        })
        .collect();

    let widths = [Constraint::Length(7), Constraint::Min(0)];
    let header = [t("#"), t("Project")];
    let header_refs: Vec<&str> = header.iter().map(|s| s.as_str()).collect();

    drawer::render_table(frame, area, &title, &header_refs, rows, &widths, selected);
}
