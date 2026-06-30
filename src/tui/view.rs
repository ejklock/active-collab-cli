use crate::i18n::t;
use crate::render::{display_width, wrap_text};
use crate::tui::model::{
    ClickTarget, Compose, ComposeKind, ComposeStatus, ModalButtonTarget, Model, Screen, Selection,
};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks, DetailParams};
use crate::tui::theme;
use crate::tui::widgets::modal::ModalContent;
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

/// Footer hint for the given screen.
///
/// When a compose modal is open the modal owns the compose hint (ADR 0039 §5).
/// When the confirm-delete modal is open the modal owns its hint; the footer
/// shows no confirm hint (one-home rule). Falls through to own-focused or browse.
pub(crate) fn hint_for_screen(screen: &Screen) -> String {
    match screen {
        Screen::Detail {
            overlay,
            focused_comment,
            comments,
            current_user_id,
            ..
        } => {
            // The compose modal owns its hint when active; pass None to footer.
            let compose_for_footer = if overlay.is_compose() {
                None
            } else {
                overlay.compose()
            };
            // The confirm modal owns its hint; pass None so the footer does not
            // duplicate it (ADR 0039 §5 one-home suppression).
            let confirm_for_footer = if overlay.is_confirm() {
                None
            } else {
                overlay.confirm_delete_id()
            };
            detail_hint(
                compose_for_footer,
                confirm_for_footer,
                *focused_comment,
                comments,
                *current_user_id,
            )
        }
        _ => t("↑/↓ navigate  Enter select  r refresh  Esc/b back  q quit"),
    }
}

/// Derive the context-aware instruction hint for the Detail screen.
///
/// Priority order matches ADR 0038 §1: composing beats confirming-delete beats
/// own-comment-focused beats the browsing default.
pub(crate) fn detail_hint(
    compose: Option<&Compose>,
    confirm_delete: Option<i64>,
    focused_comment: Option<usize>,
    comments: &[serde_json::Value],
    current_user_id: Option<i64>,
) -> String {
    if compose.is_some() {
        return t("Ctrl+S send · Esc cancel");
    }
    if confirm_delete.is_some() {
        return t("Enter/click confirm · Esc cancel");
    }
    if is_own_comment_focused(focused_comment, comments, current_user_id) {
        return t("j/k move · Ctrl+click edit/delete · c new");
    }
    t("j/k move · c comment · r refresh · Esc/b back · q quit")
}

fn is_own_comment_focused(
    focused_comment: Option<usize>,
    comments: &[serde_json::Value],
    current_user_id: Option<i64>,
) -> bool {
    let (Some(idx), Some(uid)) = (focused_comment, current_user_id) else {
        return false;
    };
    comments
        .get(idx)
        .and_then(|c| c.get("created_by_id"))
        .and_then(|v| v.as_i64())
        .map(|cid| cid == uid)
        .unwrap_or(false)
}

/// Derive the transient status string for the Detail footer status row.
///
/// Priority (highest first): auth_error > copied_feedback.
/// When `compose` is `Some`, the modal overlay owns the compose hint/status (ADR 0039 §5);
/// the footer still shows auth_error or copied_feedback if either is set.
pub(crate) fn detail_status_line(
    compose: Option<&Compose>,
    copied_feedback: bool,
    auth_error: bool,
) -> Option<String> {
    if auth_error {
        return Some(t(
            "Token invalid or revoked — run `ac setup add` to re-authenticate.",
        ));
    }
    if compose.is_some() {
        return if copied_feedback {
            Some(t("Copiado ✓"))
        } else {
            None
        };
    }
    if copied_feedback {
        return Some(t("Copiado ✓"));
    }
    None
}

/// Status text rendered inside the compose modal's in-box hint line.
///
/// Returns `Some(status)` when the compose has a transient state to display,
/// `None` when editing normally (the hint text suffices).
pub(crate) fn compose_modal_status(compose: &Compose) -> Option<String> {
    match &compose.status {
        ComposeStatus::Submitting => Some(t("Sending…")),
        ComposeStatus::Error(_) => Some(t("Failed to post comment")),
        ComposeStatus::Editing => None,
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
    /// Thin transient status row rendered below the hint region; collapses when None.
    status_line: Option<String>,
}

impl FooterPlan {
    fn compute(
        hint: &str,
        last_loaded: Option<&str>,
        copied_feedback: bool,
        status_line: Option<String>,
        width: usize,
    ) -> Self {
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

        let status_height: u16 = if status_line.is_some() { 1 } else { 0 };

        if right_segments.is_empty() {
            return Self {
                height: wrapped_height(hint, width) + status_height,
                hint: hint.to_string(),
                right_text: None,
                stacked: false,
                right_is_copied: false,
                status_line,
            };
        }

        let right_text = right_segments.join("  ");
        let hint_dw = display_width(hint);
        let right_dw = display_width(&right_text);

        if hint_dw + 1 + right_dw <= width {
            Self {
                height: 1 + status_height,
                hint: hint.to_string(),
                right_text: Some(right_text),
                stacked: false,
                right_is_copied: copied_feedback,
                status_line,
            }
        } else {
            let hint_height = wrapped_height(hint, width);
            let right_height = wrapped_height(&right_text, width);
            Self {
                height: hint_height + right_height + status_height,
                hint: hint.to_string(),
                right_text: Some(right_text),
                stacked: true,
                right_is_copied: copied_feedback,
                status_line,
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
/// `modal_btn_targets` is populated when the confirm modal renders; the shell
/// stores it on the model so `handle_click_detail` can resolve plain clicks.
pub fn view(
    model: &Model,
    frame: &mut Frame,
    targets: &mut Vec<ClickTarget>,
    modal_btn_targets: &mut Vec<ModalButtonTarget>,
) {
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
    let status_line = if let Screen::Detail {
        overlay,
        auth_error,
        ..
    } = screen
    {
        detail_status_line(overlay.compose(), model.copied_feedback, *auth_error)
    } else {
        None
    };
    let footer_plan = FooterPlan::compute(
        &hint_text,
        model.last_loaded.as_deref(),
        model.copied_feedback,
        status_line,
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
            focused_comment,
            comment_spans,
            overlay,
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
                    focused_comment: *focused_comment,
                    comment_spans,
                },
            );
            if let Some(ref sel) = model.selection {
                draw_selection_highlight(frame, sel);
            }
            if let Some(cp) = overlay.compose() {
                render_compose_modal(frame, area, cp);
            }
            if overlay.is_confirm() {
                render_confirm_modal(frame, area, modal_btn_targets);
            }
        }
    }

    render_footer(frame, chunks[2], footer_plan, theme::footer_style());
}

fn compose_modal_title(cp: &Compose) -> String {
    match &cp.kind {
        ComposeKind::New => t("New comment"),
        ComposeKind::Edit { .. } => t("Edit comment"),
    }
}

fn compose_modal_hint(cp: &Compose) -> String {
    match compose_modal_status(cp) {
        Some(status) => status,
        None => t("Ctrl+S send · Esc cancel"),
    }
}

fn render_compose_modal(frame: &mut Frame, frame_area: ratatui::layout::Rect, cp: &Compose) {
    use crate::tui::widgets::modal::render_modal;
    let (body_lines, _body_styles) = crate::render::compose_block_lines(cp);
    let hint = compose_modal_hint(cp);
    let title = compose_modal_title(cp);
    render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &title,
            lines: &body_lines,
            hint: Some(&hint),
        },
    );
}

/// Render the delete-confirm modal overlay and register the two button click targets.
///
/// Uses the shared `render_modal` primitive (ADR 0039) to dim the backdrop and draw a
/// centered bordered box. The button row in the hint line shows `[Sim]  [Não]`;
/// their absolute cell Rects are derived from the Rect `render_modal` returns so the
/// hit-test geometry is single-sourced (never recomputed independently).
fn render_confirm_modal(
    frame: &mut Frame,
    frame_area: ratatui::layout::Rect,
    btn_targets: &mut Vec<ModalButtonTarget>,
) {
    use crate::tui::widgets::modal::render_modal;
    let title = t("Delete comment?");
    let body = vec![t("This action cannot be undone.")];
    let confirm_label = format!("[{}]", t("Yes"));
    let cancel_label = format!("[{}]", t("No"));
    let hint = format!("{}  {}", confirm_label, cancel_label);
    let modal_rect = render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &title,
            lines: &body,
            hint: Some(&hint),
        },
    );
    register_confirm_button_targets(modal_rect, &confirm_label, &cancel_label, btn_targets);
}

/// Derive the absolute button cell Rects from the modal Rect and push them as targets.
///
/// The hint row occupies the last inner row: `modal_rect.y + modal_rect.height - 2`
/// (penultimate row before the bottom border). Buttons are left-aligned with a two-space
/// gap between them. Column positions are computed from the label display widths.
fn register_confirm_button_targets(
    modal_rect: ratatui::layout::Rect,
    confirm_label: &str,
    cancel_label: &str,
    btn_targets: &mut Vec<ModalButtonTarget>,
) {
    let inner_x = modal_rect.x + 1;
    let hint_row = modal_rect.y + modal_rect.height.saturating_sub(2);
    let confirm_w = display_width(confirm_label) as u16;
    let cancel_start = inner_x + confirm_w + 2;
    let cancel_w = display_width(cancel_label) as u16;
    btn_targets.push(ModalButtonTarget {
        x_start: inner_x,
        x_end: inner_x + confirm_w,
        row: hint_row,
        is_confirm: true,
    });
    btn_targets.push(ModalButtonTarget {
        x_start: cancel_start,
        x_end: cancel_start + cancel_w,
        row: hint_row,
        is_confirm: false,
    });
}

fn render_footer(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    plan: FooterPlan,
    style: ratatui::style::Style,
) {
    let (hint_area, status_area) = split_footer_status_row(area, &plan.status_line);
    render_footer_hint_region(frame, hint_area, &plan, style);
    if let Some(ref status_text) = plan.status_line {
        if let Some(sa) = status_area {
            let status_widget =
                Paragraph::new(status_text.clone()).style(theme::footer_status_style());
            frame.render_widget(status_widget, sa);
        }
    }
}

fn split_footer_status_row(
    area: ratatui::layout::Rect,
    status_line: &Option<String>,
) -> (ratatui::layout::Rect, Option<ratatui::layout::Rect>) {
    if status_line.is_none() || area.height < 2 {
        return (area, None);
    }
    let hint_height = area.height.saturating_sub(1);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(hint_height), Constraint::Length(1)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn render_footer_hint_region(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    plan: &FooterPlan,
    style: ratatui::style::Style,
) {
    let right_text = match &plan.right_text {
        None => {
            let footer = Paragraph::new(plan.hint.clone()).style(style);
            frame.render_widget(footer, area);
            return;
        }
        Some(rt) => rt.clone(),
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
