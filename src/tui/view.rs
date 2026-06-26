use crate::i18n::t;
use crate::tui::model::{ClickTarget, Model, Screen};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks, DetailParams};
use crate::tui::theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::Paragraph,
    Frame,
};

const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 6;

/// Reformat a BRT timestamp `YYYY-MM-DDTHH:MM:SS` into `DD/MM/YYYY HH:MM`.
/// Returns None when the input is too short or cannot be sliced at the expected offsets.
pub(crate) fn format_br_datetime(iso: &str) -> Option<String> {
    // Minimum: "YYYY-MM-DDTHH:MM" = 16 chars.
    if iso.len() < 16 {
        return None;
    }
    let year = iso.get(0..4)?;
    let month = iso.get(5..7)?;
    let day = iso.get(8..10)?;
    let hour = iso.get(11..13)?;
    let minute = iso.get(14..16)?;
    Some(format!("{}/{}/{} {}:{}", day, month, year, hour, minute))
}

/// Render the top screen into the terminal frame.
///
/// Splits the frame into the main content area and a one-line footer, then
/// dispatches to the correct screen renderer. `targets` is populated by the
/// Projects/Tasks renderers with the visible rows' y-ranges; the shell stores
/// it on the model after draw so `handle_click_list` can resolve clicks.
pub fn view(model: &Model, frame: &mut Frame, targets: &mut Vec<ClickTarget>) {
    let Some(screen) = model.top() else { return };

    let area = frame.area();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = Paragraph::new(t("Terminal too small")).alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(model.header.header_line()).style(theme::app_header_style());
    frame.render_widget(header, chunks[0]);

    match screen {
        Screen::Projects {
            groups,
            selected,
            loading,
            ..
        } => {
            draw_projects(frame, chunks[1], groups, *selected, *loading, targets);
        }
        Screen::Tasks {
            project_name,
            tasks,
            selected,
            loading,
            ..
        } => {
            draw_tasks(
                frame,
                chunks[1],
                project_name,
                tasks,
                *selected,
                *loading,
                targets,
            );
        }
        Screen::Detail {
            task,
            lines,
            line_styles,
            assets,
            offset,
            loading,
            task_id,
            ..
        } => {
            let task_name = task.get("name").and_then(|v| v.as_str()).unwrap_or("");
            draw_detail(
                frame,
                chunks[1],
                DetailParams {
                    lines,
                    line_styles,
                    assets,
                    offset: *offset,
                    loading: *loading,
                    task_id: *task_id,
                    task_name,
                },
            );
        }
    }

    let hint_text = match screen {
        Screen::Detail { assets, .. } if !assets.is_empty() => {
            t("↑/↓ scroll  r refresh  Esc/b back  q quit  1-9 open asset  d+1-9 download  s selection")
        }
        Screen::Detail { .. } => t("↑/↓ scroll  r refresh  Esc/b back  q quit  s selection"),
        _ => t("↑/↓ navigate  Enter select  r refresh  Esc/b back  q quit  s selection"),
    };
    let footer_style = theme::footer_style();

    render_footer(
        frame,
        chunks[2],
        hint_text,
        model.last_loaded.as_deref(),
        model.selection_mode,
        footer_style,
    );
}

fn render_footer(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    hint: String,
    last_loaded: Option<&str>,
    selection_mode: bool,
    style: ratatui::style::Style,
) {
    let timestamp_text = last_loaded
        .and_then(format_br_datetime)
        .map(|formatted| format!("{} {}", t("Updated at"), formatted));

    let indicator = if selection_mode {
        Some(t("footer.selection_indicator"))
    } else {
        None
    };

    let right_segments: Vec<String> = [indicator, timestamp_text].into_iter().flatten().collect();

    if right_segments.is_empty() {
        let footer = Paragraph::new(hint).style(style);
        frame.render_widget(footer, area);
        return;
    }

    let right_text = right_segments.join("  ");
    let right_width = right_text.chars().count() as u16;
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(area);

    let hint_widget = Paragraph::new(hint).style(style);
    frame.render_widget(hint_widget, footer_chunks[0]);

    let indicator_style = if selection_mode {
        theme::selection_indicator_style()
    } else {
        style
    };
    let right_widget = Paragraph::new(right_text)
        .style(indicator_style)
        .alignment(Alignment::Right);
    frame.render_widget(right_widget, footer_chunks[1]);
}
