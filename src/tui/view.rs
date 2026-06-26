use crate::i18n::t;
use crate::tui::model::{Model, Screen};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks};
use crate::tui::theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::Paragraph,
    Frame,
};

const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 6;

/// Render the top screen into the terminal frame.
///
/// Splits the frame into the main content area and a one-line footer, then
/// dispatches to the correct screen renderer.
pub fn view(model: &Model, frame: &mut Frame) {
    let Some(screen) = model.top() else { return };

    let area = frame.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = Paragraph::new(t("Terminal too small")).alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    match screen {
        Screen::Projects {
            groups,
            selected,
            loading,
            ..
        } => {
            draw_projects(frame, chunks[0], groups, *selected, *loading);
        }
        Screen::Tasks {
            project_name,
            tasks,
            selected,
            loading,
            ..
        } => {
            draw_tasks(frame, chunks[0], project_name, tasks, *selected, *loading);
        }
        Screen::Detail {
            lines,
            assets,
            offset,
            loading,
            task_id,
            ..
        } => {
            draw_detail(frame, chunks[0], lines, assets, *offset, *loading, *task_id);
        }
    }

    let footer_text = match screen {
        Screen::Detail { assets, .. } if !assets.is_empty() => {
            t("↑/↓ scroll  Esc/b back  q quit  1-9 open asset  d+1-9 download")
        }
        _ => t("↑/↓ navigate  Enter select  Esc/b back  q quit"),
    };
    let footer = Paragraph::new(footer_text).style(theme::footer_style());
    frame.render_widget(footer, chunks[1]);
}
