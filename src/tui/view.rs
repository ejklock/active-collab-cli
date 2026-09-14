use crate::i18n::t;
use crate::render::{display_width, wrap_text};
use crate::tui::detail_geometry::Selection;
use crate::tui::footer::{self, FooterPlan};
use crate::tui::model::{
    filter_assignee_candidates, ClickTarget, Compose, ComposeKind, ComposeStatus, EditStatus,
    EstimateForm, ImageAssetRef, ImageStatus, LogTimeField, LogTimeForm, LogTimeStatus,
    ModalButtonTarget, Model, Screen,
};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks, DetailParams};
use crate::tui::theme;
use crate::tui::widgets::modal::ModalContent;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Modifier,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Margin (cells) between the near-full image-viewer box and the detail
/// content area it sits inside — distinguishes it from `render_modal`'s
/// centered ~70% box (ADR 0065).
const IMAGE_VIEWER_MARGIN: u16 = 1;

const MIN_WIDTH: u16 = 24;
const MIN_HEIGHT: u16 = 6;

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

/// Status text rendered inside the log-time modal's in-box hint line.
///
/// Returns `Some(status)` when the form has a transient state to display,
/// `None` when editing normally (the hint text suffices).
pub(crate) fn log_time_modal_status(form: &LogTimeForm) -> Option<String> {
    match &form.status {
        LogTimeStatus::Submitting => Some(t("Sending…")),
        LogTimeStatus::Error(_) => Some(t("Failed to log time")),
        LogTimeStatus::Editing => None,
    }
}

/// Status text rendered inside the estimate-edit modal's in-box hint line.
///
/// Returns `Some(status)` when the form has a transient state to display,
/// `None` when editing normally (the hint text suffices).
pub(crate) fn estimate_modal_status(form: &EstimateForm) -> Option<String> {
    match &form.status {
        EditStatus::Submitting => Some(t("Sending…")),
        EditStatus::Error(_) => Some(t("Failed to update task")),
        EditStatus::Editing => None,
    }
}

/// Status text rendered inside the status-confirm modal's in-box hint line.
///
/// Returns `Some(status)` when the overlay has a transient state to display,
/// `None` when awaiting confirmation normally (the hint text suffices).
pub(crate) fn status_confirm_modal_status(status: &EditStatus) -> Option<String> {
    match status {
        EditStatus::Submitting => Some(t("Sending…")),
        EditStatus::Error(_) => Some(t("Failed to update task")),
        EditStatus::Editing => None,
    }
}

/// Status text rendered inside the assignee-picker modal's in-box hint line.
///
/// Returns `Some(status)` when the picker has a transient state to display,
/// `None` when awaiting selection normally (the hint text suffices).
pub(crate) fn assignee_picker_modal_status(status: &EditStatus) -> Option<String> {
    match status {
        EditStatus::Submitting => Some(t("Sending…")),
        EditStatus::Error(_) => Some(t("Failed to update task")),
        EditStatus::Editing => None,
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

    let footer_plan = footer::plan(
        screen,
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
            if let Some(form) = overlay.log_time() {
                render_log_time_modal(frame, area, form);
            }
            if let Some(form) = overlay.estimate_edit() {
                render_estimate_modal(frame, area, form);
            }
            if let Some((completed_target, status)) = overlay.status_confirm() {
                render_status_confirm_modal(frame, area, completed_target, status);
            }
            if let Some((candidates, filter, selected, status)) = overlay.assignee_picker() {
                render_assignee_picker_modal(frame, area, candidates, filter, selected, status);
            }
            if overlay.is_confirm() {
                render_confirm_modal(frame, area, modal_btn_targets);
            }
            if let Some((asset, status)) = overlay.image_viewer() {
                render_image_viewer(frame, area, chunks[1], asset, status);
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

/// Render the compose modal chrome (backdrop/border/title/hint) via `render_modal`
/// with an empty body, then paint `cp.editor` — the `tui_textarea::TextArea` — as a
/// widget into the returned inner Rect. Routing the caret/selection/scroll through
/// the widget itself (instead of a static `Paragraph` of `editor.lines()`) is what
/// makes them visible; the shared `render_modal` primitive still owns the box.
fn render_compose_modal(frame: &mut Frame, frame_area: ratatui::layout::Rect, cp: &Compose) {
    use crate::tui::widgets::modal::render_modal;
    let hint = compose_modal_hint(cp);
    let title = compose_modal_title(cp);
    let body_rect = render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &title,
            lines: &[],
            hint: Some(&hint),
        },
    );
    frame.render_widget(&cp.editor, body_rect);
}

fn log_time_modal_hint(form: &LogTimeForm) -> String {
    match log_time_modal_status(form) {
        Some(status) => status,
        None => t("Ctrl+S send · Tab switch field · Esc cancel"),
    }
}

/// Format one labeled field row, marking the currently focused field.
fn log_time_field_line(label: &str, value: &str, is_focused: bool) -> String {
    let marker = if is_focused { ">" } else { " " };
    format!("{marker} {label}: {value}")
}

/// Render the log-time modal chrome via `render_modal`: the Hours and Summary
/// fields as labeled rows, plus the hint/status line. Both fields are plain-text
/// buffers (ADR 0064 does not apply — single-line, no caret/undo needs), so they
/// render as static rows rather than a `TextArea` widget.
fn render_log_time_modal(frame: &mut Frame, frame_area: ratatui::layout::Rect, form: &LogTimeForm) {
    use crate::tui::widgets::modal::render_modal;
    let hint = log_time_modal_hint(form);
    let hours_line =
        log_time_field_line(&t("Hours"), &form.hours, form.field == LogTimeField::Hours);
    let summary_line = log_time_field_line(
        &t("Summary"),
        &form.summary,
        form.field == LogTimeField::Summary,
    );
    let lines = [hours_line, summary_line];
    render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &t("Log time"),
            lines: &lines,
            hint: Some(&hint),
        },
    );
}

fn estimate_modal_hint(form: &EstimateForm) -> String {
    match estimate_modal_status(form) {
        Some(status) => status,
        None => t("Ctrl+S send · Esc cancel"),
    }
}

/// Render the estimate-edit modal chrome via `render_modal`: the estimate field as
/// a single labeled row, plus the hint/status line. Single line, plain-text buffer
/// (ADR 0064 does not apply — no caret/undo needs), so it renders as a static row
/// rather than a `TextArea` widget.
fn render_estimate_modal(
    frame: &mut Frame,
    frame_area: ratatui::layout::Rect,
    form: &EstimateForm,
) {
    use crate::tui::widgets::modal::render_modal;
    let hint = estimate_modal_hint(form);
    let value_line = format!("{}: {}", t("Estimate (hours)"), form.value);
    let lines = [value_line];
    render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &t("Edit estimate"),
            lines: &lines,
            hint: Some(&hint),
        },
    );
}

fn status_confirm_modal_hint(status: &EditStatus) -> String {
    match status_confirm_modal_status(status) {
        Some(status) => status,
        None => t("Enter/Ctrl+S confirm · Esc cancel"),
    }
}

/// The complete-vs-reopen prompt line, chosen from the pending `completed_target`.
fn status_confirm_prompt(completed_target: bool) -> String {
    if completed_target {
        t("Mark task as completed?")
    } else {
        t("Reopen task?")
    }
}

/// Render the status-confirm modal chrome via `render_modal`: the complete-vs-reopen
/// prompt as a single line, plus the hint/status line. No text entry — the target is
/// fixed by `completed_target`, not typed.
fn render_status_confirm_modal(
    frame: &mut Frame,
    frame_area: ratatui::layout::Rect,
    completed_target: bool,
    status: &EditStatus,
) {
    use crate::tui::widgets::modal::render_modal;
    let hint = status_confirm_modal_hint(status);
    let lines = [status_confirm_prompt(completed_target)];
    render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &t("Change status"),
            lines: &lines,
            hint: Some(&hint),
        },
    );
}

fn assignee_picker_modal_hint(status: &EditStatus) -> String {
    match assignee_picker_modal_status(status) {
        Some(status) => status,
        None => t("↑/↓ move · type to filter · Enter/Ctrl+S confirm · Esc cancel"),
    }
}

/// Mark the highlighted candidate row with a leading indicator, mirroring the
/// focused-field marker convention used by the log-time modal.
fn assignee_candidate_line(name: &str, is_selected: bool) -> String {
    let marker = if is_selected { ">" } else { " " };
    format!("{marker} {name}")
}

/// Build the assignee-picker modal body: a search line reflecting the current
/// filter, then either the filtered candidate rows (highlighted row marked), an
/// empty-directory line when the user directory has no candidates at all, or a
/// distinct empty-match line when the filter excludes every candidate.
fn assignee_picker_modal_lines(
    candidates: &[(i64, String)],
    filter: &str,
    selected: usize,
) -> Vec<String> {
    let search_line = format!("{}: {}", t("Search"), filter);
    let mut lines = vec![search_line];
    if candidates.is_empty() {
        lines.push(t("No assignable users"));
        return lines;
    }
    let filtered = filter_assignee_candidates(candidates, filter);
    if filtered.is_empty() {
        lines.push(t("No matching users"));
        return lines;
    }
    lines.extend(
        filtered
            .iter()
            .enumerate()
            .map(|(i, (_, name))| assignee_candidate_line(name, i == selected)),
    );
    lines
}

/// Render the assignee-picker modal chrome via `render_modal`: the search line,
/// the filtered candidate list with the highlighted row marked, an empty-directory
/// or empty-match line as appropriate, plus the hint/status line.
fn render_assignee_picker_modal(
    frame: &mut Frame,
    frame_area: ratatui::layout::Rect,
    candidates: &[(i64, String)],
    filter: &str,
    selected: usize,
    status: &EditStatus,
) {
    use crate::tui::widgets::modal::render_modal;
    let hint = assignee_picker_modal_hint(status);
    let lines = assignee_picker_modal_lines(candidates, filter, selected);
    render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &t("Assign task"),
            lines: &lines,
            hint: Some(&hint),
        },
    );
}

/// Render the delete-confirm modal overlay and register the two button click targets.
///
/// Uses the shared `render_modal` primitive (ADR 0039) to dim the backdrop and draw a
/// centered bordered box. The button row in the hint line shows `[Sim]  [Não]`;
/// their absolute cell Rects are derived from the body Rect `render_modal` returns so
/// the hit-test geometry is single-sourced (never recomputed independently).
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
    let body_rect = render_modal(
        frame,
        frame_area,
        ModalContent {
            title: &title,
            lines: &body,
            hint: Some(&hint),
        },
    );
    register_confirm_button_targets(body_rect, &confirm_label, &cancel_label, btn_targets);
}

/// Derive the absolute button cell Rects from the modal body Rect and push them as targets.
///
/// `body_rect` (from `render_modal`) already excludes the border and the hint row, so
/// the hint row itself sits immediately below it: `body_rect.y + body_rect.height`.
/// Buttons are left-aligned with a two-space gap between them; column positions are
/// computed from the label display widths.
fn register_confirm_button_targets(
    body_rect: ratatui::layout::Rect,
    confirm_label: &str,
    cancel_label: &str,
    btn_targets: &mut Vec<ModalButtonTarget>,
) {
    let inner_x = body_rect.x;
    let hint_row = body_rect.y + body_rect.height;
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

/// Render the image-viewer overlay.
///
/// Unlike `render_modal`'s centered ~70% box, this sizes the box to the
/// near-full detail content area (`content_area` minus `IMAGE_VIEWER_MARGIN`)
/// so a real image fills as much of the screen as possible once ratatui-image
/// lands (slice 0059). `frame_area` (the whole frame) is what gets dimmed,
/// matching the other Detail overlays.
fn render_image_viewer(
    frame: &mut Frame,
    frame_area: Rect,
    content_area: Rect,
    asset: &ImageAssetRef,
    status: &ImageStatus,
) {
    dim_frame(frame, frame_area);
    let viewer_area = image_viewer_area(content_area);
    frame.render_widget(Clear, viewer_area);
    draw_image_viewer_box(frame, viewer_area, asset, status);
}

/// Apply `Modifier::DIM` plus the modal backdrop style to every cell in `area`.
///
/// Duplicates `widgets::modal::dim_backdrop` (private to that module) rather
/// than exposing it — the same buffer-patch technique `draw_selection_highlight`
/// already uses below.
fn dim_frame(frame: &mut Frame, area: Rect) {
    let backdrop_style = theme::modal_backdrop_style();
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let patched = cell
                    .style()
                    .add_modifier(Modifier::DIM)
                    .patch(backdrop_style);
                cell.set_style(patched);
            }
        }
    }
}

/// Shrink `content_area` by `IMAGE_VIEWER_MARGIN` on every side, clamped to
/// stay non-degenerate on a tiny terminal.
fn image_viewer_area(content_area: Rect) -> Rect {
    let margin = IMAGE_VIEWER_MARGIN;
    let w = content_area.width.saturating_sub(margin * 2).max(1);
    let h = content_area.height.saturating_sub(margin * 2).max(1);
    Rect::new(content_area.x + margin, content_area.y + margin, w, h)
}

fn draw_image_viewer_box(
    frame: &mut Frame,
    area: Rect,
    asset: &ImageAssetRef,
    status: &ImageStatus,
) {
    let title = format!(" {} ", asset.label);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::modal_border_style())
        .title(title)
        .title_style(theme::modal_title_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    draw_image_viewer_body(frame, inner, asset, status);
}

/// Draw the placeholder body text plus the "Esc/q fechar" hint on the bottom row.
///
/// Loading shows a status line + the filename; Error shows the failure message;
/// Ready is a placeholder too in this slice — the real `ratatui-image` render
/// replaces it in slice 0059.
fn draw_image_viewer_body(
    frame: &mut Frame,
    inner: Rect,
    asset: &ImageAssetRef,
    status: &ImageStatus,
) {
    let hint_rows: u16 = 1;
    let body_h = inner.height.saturating_sub(hint_rows);
    let body_area = Rect::new(inner.x, inner.y, inner.width, body_h);
    let body_text = image_viewer_body_text(status, &asset.label);
    let paragraph = Paragraph::new(body_text)
        .alignment(Alignment::Center)
        .style(theme::modal_body_style());
    frame.render_widget(paragraph, body_area);

    if inner.height <= body_h {
        return;
    }
    let hint_area = Rect::new(inner.x, inner.y + body_h, inner.width, 1);
    let hint = Paragraph::new(t("Esc/q close"))
        .alignment(Alignment::Center)
        .style(theme::modal_hint_style());
    frame.render_widget(hint, hint_area);
}

fn image_viewer_body_text(status: &ImageStatus, label: &str) -> String {
    match status {
        ImageStatus::Loading => format!("{}\n{}", t("Loading…"), label),
        ImageStatus::Error(reason) => reason.clone(),
        ImageStatus::Ready => label.to_owned(),
    }
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
