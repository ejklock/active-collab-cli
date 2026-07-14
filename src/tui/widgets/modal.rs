use crate::tui::theme;
use ratatui::{
    layout::Rect,
    style::Modifier,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

const MODAL_MARGIN: u16 = 2;

/// Body content for a modal box.
///
/// `hint` renders on the bottom line inside the box (controls or status text);
/// `None` omits it.
pub struct ModalContent<'a> {
    pub title: &'a str,
    pub lines: &'a [String],
    pub hint: Option<&'a str>,
}

/// Compute a centered, clamped `Rect` for a modal box.
///
/// Centers `desired_w × desired_h` within `frame_area`, then clamps so the
/// box stays inside `frame_area` with `MODAL_MARGIN` clearance on each side.
/// Never returns a `Rect` whose right or bottom edge exceeds `frame_area`.
pub fn modal_area(frame_area: Rect, desired_w: u16, desired_h: u16) -> Rect {
    let max_w = frame_area.width.saturating_sub(MODAL_MARGIN * 2);
    let max_h = frame_area.height.saturating_sub(MODAL_MARGIN * 2);
    let w = desired_w.min(max_w).max(1);
    let h = desired_h.min(max_h).max(1);
    let x = frame_area.x + (frame_area.width.saturating_sub(w)) / 2;
    let y = frame_area.y + (frame_area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Compute the ≈ 70 % target size for a modal box, respecting a content minimum.
///
/// Returns `(target_w, target_h)` where each dimension is `frame * 7 / 10`
/// raised to the content minimum so the body + hint + borders always fit.
pub fn modal_target_size(frame_area: Rect, content: &ModalContent<'_>) -> (u16, u16) {
    let body_rows = content.lines.len() as u16;
    let hint_rows: u16 = if content.hint.is_some() { 1 } else { 0 };
    let min_h = body_rows + hint_rows + 2;
    let min_w = content
        .lines
        .iter()
        .map(|l| l.chars().count() as u16)
        .max()
        .unwrap_or(0)
        .saturating_add(4)
        .max(10);
    let target_w = (frame_area.width * 7 / 10).max(min_w);
    let target_h = (frame_area.height * 7 / 10).max(min_h);
    (target_w, target_h)
}

/// Render a modal overlay:
/// 1. Strongly dims the backdrop cells in `frame_area`: applies `Modifier::DIM`
///    and the dark backdrop background from `theme::modal_backdrop_style()`.
/// 2. Sizes the modal box to ≈ 70 % of the frame (content-minimum floor), then
///    computes and `Clear`s the modal `Rect` so the box is opaque.
/// 3. Draws a bordered titled box with body lines and an optional hint line.
///
/// Returns the inner content (body) `Rect` — beneath the title border, above
/// the hint row — so a caller can either register click targets relative to
/// it (the confirm modal) or render a widget of its own into it (the compose
/// modal's `TextArea`, on top of the body `Paragraph` this function already
/// drew there).
pub fn render_modal(frame: &mut Frame, frame_area: Rect, content: ModalContent<'_>) -> Rect {
    dim_backdrop(frame, frame_area);
    let (desired_w, desired_h) = modal_target_size(frame_area, &content);
    let area = modal_area(frame_area, desired_w, desired_h);
    frame.render_widget(Clear, area);
    draw_modal_box(frame, area, content)
}

fn dim_backdrop(frame: &mut Frame, area: Rect) {
    let backdrop_bg = theme::modal_backdrop_style();
    let buf = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let patched = cell.style().add_modifier(Modifier::DIM).patch(backdrop_bg);
                cell.set_style(patched);
            }
        }
    }
}

fn draw_modal_box(frame: &mut Frame, area: Rect, content: ModalContent<'_>) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::modal_border_style())
        .title(format!(" {} ", content.title))
        .title_style(theme::modal_title_style());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    draw_modal_body(frame, inner, content)
}

fn draw_modal_body(frame: &mut Frame, inner: Rect, content: ModalContent<'_>) -> Rect {
    let hint_rows: u16 = if content.hint.is_some() { 1 } else { 0 };
    let body_h = inner.height.saturating_sub(hint_rows);
    let body_area = Rect::new(inner.x, inner.y, inner.width, body_h);
    let body_text: Vec<&str> = content.lines.iter().map(String::as_str).collect();
    let body_paragraph = Paragraph::new(body_text.join("\n")).style(theme::modal_body_style());
    frame.render_widget(body_paragraph, body_area);
    draw_modal_hint(frame, inner, body_h, content.hint);
    body_area
}

fn draw_modal_hint(frame: &mut Frame, inner: Rect, body_h: u16, hint: Option<&str>) {
    let Some(hint_text) = hint else { return };
    if inner.height < body_h + 1 {
        return;
    }
    let hint_area = Rect::new(inner.x, inner.y + body_h, inner.width, 1);
    let hint_widget = Paragraph::new(hint_text).style(theme::modal_hint_style());
    frame.render_widget(hint_widget, hint_area);
}
