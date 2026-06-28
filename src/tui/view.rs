use crate::i18n::t;
use crate::render::{display_width, wrap_text};
use crate::tui::model::{ClickTarget, Model, Screen, Selection};
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

pub(crate) fn hint_for_screen(screen: &Screen) -> String {
    match screen {
        Screen::Detail { .. } => t("↑/↓ scroll  r refresh  Esc/b back  q quit"),
        _ => t("↑/↓ navigate  Enter select  r refresh  Esc/b back  q quit"),
    }
}

/// Number of wrapped lines a text occupies at the given display-column width.
/// Returns at least 1 for non-empty text; returns 1 for empty text.
fn wrapped_height(text: &str, width: usize) -> u16 {
    if text.is_empty() || width == 0 {
        return 1;
    }
    wrap_text(text, width).len().max(1) as u16
}

/// Pre-computed plan for how the footer should be rendered.
struct FooterPlan {
    height: u16,
    /// The full hint string (may be multi-line when stacked).
    hint: String,
    /// Right-side text (timestamp and/or copied indicator), if any.
    right_text: Option<String>,
    /// When true, hint and right cannot share a row; render right below hint.
    stacked: bool,
    right_is_copied: bool,
}

impl FooterPlan {
    fn compute(hint: &str, last_loaded: Option<&str>, copied_feedback: bool, width: usize) -> Self {
        let timestamp_text = last_loaded
            .and_then(format_br_datetime)
            .map(|formatted| format!("{} {}", t("Updated at"), formatted));

        let copied_indicator = if copied_feedback {
            Some(t("footer.copied_indicator"))
        } else {
            None
        };

        let right_segments: Vec<String> = [copied_indicator, timestamp_text]
            .into_iter()
            .flatten()
            .collect();

        if right_segments.is_empty() {
            return Self {
                height: wrapped_height(hint, width),
                hint: hint.to_string(),
                right_text: None,
                stacked: false,
                right_is_copied: false,
            };
        }

        let right_text = right_segments.join("  ");
        let hint_dw = display_width(hint);
        let right_dw = display_width(&right_text);

        if hint_dw + 1 + right_dw <= width {
            Self {
                height: 1,
                hint: hint.to_string(),
                right_text: Some(right_text),
                stacked: false,
                right_is_copied: copied_feedback,
            }
        } else {
            let hint_height = wrapped_height(hint, width);
            let right_height = wrapped_height(&right_text, width);
            Self {
                height: hint_height + right_height,
                hint: hint.to_string(),
                right_text: Some(right_text),
                stacked: true,
                right_is_copied: copied_feedback,
            }
        }
    }
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

    let area_width = area.width as usize;
    let header_line = model.header.header_line();
    let header_wrapped = wrap_text(&header_line, area_width);
    let header_height = header_wrapped.len().max(1) as u16;

    let hint_text = hint_for_screen(screen);
    let footer_plan = FooterPlan::compute(
        &hint_text,
        model.last_loaded.as_deref(),
        model.copied_feedback,
        area_width,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(0),
            Constraint::Length(footer_plan.height),
        ])
        .split(area);

    let header_text = header_wrapped.join("\n");
    let header = Paragraph::new(header_text).style(theme::app_header_style());
    frame.render_widget(header, chunks[0]);

    match screen {
        Screen::Projects {
            groups,
            selected,
            loading,
            revalidating,
            ..
        } => {
            draw_projects(
                frame,
                chunks[1],
                groups,
                *selected,
                *loading,
                *revalidating,
                targets,
            );
        }
        Screen::Tasks {
            project_name,
            tasks,
            selected,
            loading,
            revalidating,
            card_heights,
            card_offsets,
            rendered_width,
            ..
        } => {
            let today = chrono::Local::now().date_naive();
            draw_tasks(
                frame,
                chunks[1],
                project_name,
                tasks,
                *selected,
                *loading,
                *revalidating,
                today,
                targets,
                card_heights,
                card_offsets,
                *rendered_width,
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
            if let Some(ref sel) = model.selection {
                draw_selection_highlight(frame, sel);
            }
        }
    }

    render_footer(frame, chunks[2], footer_plan, theme::footer_style());
}

fn render_footer(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    plan: FooterPlan,
    style: ratatui::style::Style,
) {
    let right_text = match plan.right_text {
        None => {
            let footer = Paragraph::new(plan.hint).style(style);
            frame.render_widget(footer, area);
            return;
        }
        Some(rt) => rt,
    };

    if !plan.stacked {
        render_footer_side_by_side(
            frame,
            area,
            &plan.hint,
            &right_text,
            plan.right_is_copied,
            style,
        );
    } else {
        render_footer_stacked(
            frame,
            area,
            &plan.hint,
            &right_text,
            plan.right_is_copied,
            style,
        );
    }
}

/// Render a right-aligned footer segment, applying the copied indicator
/// style when `right_is_copied` is true.
fn render_footer_right_segment(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    right_text: &str,
    right_is_copied: bool,
    base_style: ratatui::style::Style,
) {
    let indicator_style = if right_is_copied {
        theme::copied_indicator_style()
    } else {
        base_style
    };
    let right_widget = Paragraph::new(right_text.to_string())
        .style(indicator_style)
        .alignment(Alignment::Right);
    frame.render_widget(right_widget, area);
}

fn render_footer_side_by_side(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    hint: &str,
    right_text: &str,
    right_is_copied: bool,
    style: ratatui::style::Style,
) {
    let right_width = display_width(right_text) as u16;
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(area);

    let hint_widget = Paragraph::new(hint.to_string()).style(style);
    frame.render_widget(hint_widget, footer_chunks[0]);

    render_footer_right_segment(frame, footer_chunks[1], right_text, right_is_copied, style);
}

fn render_footer_stacked(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    hint: &str,
    right_text: &str,
    right_is_copied: bool,
    style: ratatui::style::Style,
) {
    let width = area.width as usize;
    let hint_lines = wrap_text(hint, width);
    let hint_height = hint_lines.len().max(1) as u16;

    let stack_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(hint_height), Constraint::Min(0)])
        .split(area);

    let hint_widget = Paragraph::new(hint_lines.join("\n")).style(style);
    frame.render_widget(hint_widget, stack_chunks[0]);

    render_footer_right_segment(frame, stack_chunks[1], right_text, right_is_copied, style);
}

/// Draw a reverse-video highlight over the cells covered by `sel`.
///
/// Overwrites only the background+foreground modifier of cells in the
/// selection range; text content is preserved (ratatui buffer merge).
fn draw_selection_highlight(frame: &mut Frame, sel: &Selection) {
    let ((top_row, top_col), (bot_row, bot_col)) = sel.normalized();
    let buf = frame.buffer_mut();
    let area = *buf.area();
    let style = theme::body_selection_style();

    for r in top_row..=bot_row {
        if r >= area.height {
            break;
        }
        let (col_start, col_end) =
            highlighted_col_span(r, top_row, bot_row, top_col, bot_col, &area);
        apply_highlight_to_row(buf, r, col_start, col_end, style);
    }
}

/// Compute the (start, end) column range to highlight on a single row.
fn highlighted_col_span(
    row: u16,
    top_row: u16,
    bot_row: u16,
    top_col: u16,
    bot_col: u16,
    area: &ratatui::layout::Rect,
) -> (u16, u16) {
    let col_start = if row == top_row { top_col } else { 0 };
    let col_end = if row == bot_row {
        bot_col
    } else {
        area.width.saturating_sub(1)
    };
    (col_start, col_end)
}

/// Apply the highlight style to cells in [col_start..=col_end] on the given row.
fn apply_highlight_to_row(
    buf: &mut ratatui::buffer::Buffer,
    row: u16,
    col_start: u16,
    col_end: u16,
    style: ratatui::style::Style,
) {
    for c in col_start..=col_end {
        if c >= buf.area().width {
            break;
        }
        if let Some(cell) = buf.cell_mut((c, row)) {
            cell.set_style(style);
        }
    }
}
