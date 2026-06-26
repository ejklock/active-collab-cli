use crate::i18n::t;
use crate::render::{link_segments, Asset};
use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span, Text},
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

/// Height of the Artifacts panel for a given asset count.
///
/// Returns 0 when there are no assets (no panel is drawn).
/// Otherwise: 1 row per asset plus 2 border rows, capped at 8.
pub fn asset_panel_height(assets_len: usize) -> u16 {
    if assets_len == 0 {
        return 0;
    }
    (assets_len as u16 + 2).min(8)
}

/// Compute the Rect occupied by the Artifacts panel within `area`.
///
/// Returns `None` when `assets_len` is 0 (no panel is drawn).
/// Delegates all height arithmetic to `asset_panel_height` so the
/// formula lives in exactly one place.
pub fn detail_asset_panel_rect(area: Rect, assets_len: usize) -> Option<Rect> {
    let panel_height = asset_panel_height(assets_len);
    if panel_height == 0 {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(panel_height)])
        .split(area);
    Some(chunks[1])
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
pub fn draw_detail(frame: &mut Frame, area: Rect, params: DetailParams<'_>) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let title = build_frame_title(params.task_name, params.task_id, inner_width);

    if params.loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    match detail_asset_panel_rect(area, params.assets.len()) {
        None => render_content(frame, area, params.lines, params.offset, title),
        Some(panel_rect) => {
            let content_rect = Rect::new(area.x, area.y, area.width, panel_rect.y - area.y);
            render_content(frame, content_rect, params.lines, params.offset, title);
            render_assets_panel(frame, panel_rect, params.assets);
        }
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

fn styled_line(line: &str) -> Line<'static> {
    let segs = link_segments(line);
    let spans: Vec<Span<'static>> = segs
        .into_iter()
        .map(|seg| {
            if seg.is_link {
                Span::styled(seg.text, theme::link_style())
            } else {
                Span::raw(seg.text)
            }
        })
        .collect();
    Line::from(spans)
}

fn render_content(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    lines: &[String],
    offset: usize,
    title: String,
) {
    let text: Text = Text::from(lines.iter().map(|l| styled_line(l)).collect::<Vec<_>>());

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
        // Scrollbar content_length = max_offset + 1 maps the scroll range [0, max_offset]
        // to ratatui's [0, content-1] so the thumb reaches the track bottom exactly
        // when eff == max_offset. viewport_content_length sizes the thumb proportionally.
        let sb_content = max_offset + 1;
        let mut scrollbar_state = ScrollbarState::new(sb_content)
            .viewport_content_length(viewport_height)
            .position(eff);
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
