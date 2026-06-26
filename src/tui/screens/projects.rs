use crate::i18n::t;
use crate::render::truncate_cell;
use crate::tui::drawer;
use crate::tui::model::ProjectGroup;
use crate::tui::theme;
use ratatui::{
    layout::Constraint,
    widgets::{Block, Borders, Cell, Paragraph},
    Frame,
};

/// Fixed column widths: TASKS (7) + INSTANCE (18) + 2 borders + selection symbol (2).
const TASKS_WIDTH: u16 = 7;
const INSTANCE_WIDTH: u16 = 18;
/// Borders (2) + selection symbol width (2) + column separators (2 pipes between 3 cols).
const FIXED_OVERHEAD: u16 = 6;

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

    let project_width =
        area.width
            .saturating_sub(TASKS_WIDTH + INSTANCE_WIDTH + FIXED_OVERHEAD) as usize;

    let rows: Vec<Vec<Cell<'static>>> = groups
        .iter()
        .map(|g| {
            vec![
                Cell::from(format!("{}", g.tasks.len())).style(theme::badge_style()),
                Cell::from(truncate_cell(&g.project_name, project_width)),
                Cell::from(truncate_cell(&g.instance, INSTANCE_WIDTH as usize)),
            ]
        })
        .collect();

    let widths = [
        Constraint::Length(TASKS_WIDTH),
        Constraint::Min(0),
        Constraint::Length(INSTANCE_WIDTH),
    ];
    let header = [t("Tasks"), t("Project"), t("Instance")];
    let header_refs: Vec<&str> = header.iter().map(|s| s.as_str()).collect();

    drawer::render_table(frame, area, &title, &header_refs, rows, &widths, selected);
}
