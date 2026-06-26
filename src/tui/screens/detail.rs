use crate::i18n::t;
use crate::render::Asset;
use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Parameters for drawing the Detail screen.
pub struct DetailParams<'a> {
    pub lines: &'a [String],
    pub assets: &'a [Asset],
    pub offset: usize,
    pub loading: bool,
    pub task_id: i64,
    pub task_name: &'a str,
}

/// Draw the Detail screen as a single scrollable content block with an optional
/// fixed Artifacts panel below.
///
/// The frame border title shows `task_name` (truncated with an ellipsis when
/// it does not fit), or falls back to `#<task_id>` when the name is empty.
///
/// When `assets` is non-empty the area is split vertically into a content chunk
/// (Min(0)) and a fixed panel chunk (Length capped at 8). Otherwise the full
/// area goes to content.
pub fn draw_detail(frame: &mut Frame, area: ratatui::layout::Rect, params: DetailParams<'_>) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let title = build_frame_title(params.task_name, params.task_id, inner_width);

    if params.loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    if params.assets.is_empty() {
        render_content(frame, area, params.lines, params.offset, title);
    } else {
        let panel_height = (params.assets.len() as u16 + 2).min(8);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(panel_height)])
            .split(area);
        render_content(frame, chunks[0], params.lines, params.offset, title);
        render_assets_panel(frame, chunks[1], params.assets);
    }
}

/// Build the frame border title from the task name, truncating with an ellipsis
/// when the name exceeds the available inner width. Falls back to `" #<id> "`
/// when the name is empty (e.g. still loading).
fn build_frame_title(task_name: &str, task_id: i64, inner_width: usize) -> String {
    if task_name.is_empty() {
        return format!(" #{} ", task_id);
    }
    let label = format!(" {} ", task_name);
    truncate_title_to_fit(&label, inner_width)
}

/// Truncate `label` to fit within `max_display_cols` display columns.
///
/// When the label fits, it is returned unchanged. When it is too wide, the
/// label is clipped at a character boundary and an ELLIPSIS + trailing space
/// is appended so the result stays within `max_display_cols`.
fn truncate_title_to_fit(label: &str, max_display_cols: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    use unicode_width::UnicodeWidthStr;

    let label_dw = UnicodeWidthStr::width(label);
    if label_dw <= max_display_cols {
        return label.to_string();
    }
    let ellipsis = '\u{2026}';
    let ellipsis_w = UnicodeWidthChar::width(ellipsis).unwrap_or(1);
    // Reserve room for ellipsis + trailing space (already part of format " name ")
    let budget = max_display_cols.saturating_sub(ellipsis_w + 1);
    let mut acc = 1usize; // leading space already contributes 1 col
    let mut result = String::from(" ");
    for ch in label.chars().skip(1) {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > budget {
            break;
        }
        result.push(ch);
        acc += cw;
    }
    result.push(ellipsis);
    result.push(' ');
    result
}

fn render_content(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    lines: &[String],
    offset: usize,
    title: String,
) {
    let text: Text = Text::from(
        lines
            .iter()
            .map(|l| Line::from(l.clone()))
            .collect::<Vec<_>>(),
    );

    let viewport_height = area.height.saturating_sub(2) as usize;
    let max_offset = lines.len().saturating_sub(viewport_height);
    let eff = offset.min(max_offset);

    let block = Block::default().borders(Borders::ALL).title(title);
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((eff as u16, 0));

    frame.render_widget(paragraph, area);

    let total_content = lines.len();
    if total_content > viewport_height {
        let mut scrollbar_state = ScrollbarState::new(total_content).position(eff);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_assets_panel(frame: &mut Frame, area: ratatui::layout::Rect, assets: &[Asset]) {
    let panel_title = format!(" {} ", t("Artifacts"));
    let rows: Vec<Line> = assets
        .iter()
        .enumerate()
        .map(|(i, asset)| {
            Line::styled(
                crate::render::asset_link_line(i + 1, asset),
                theme::asset_style(),
            )
        })
        .collect();

    let panel = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::ALL)
            .title(panel_title)
            .title_style(theme::header_style()),
    );

    frame.render_widget(panel, area);
}
