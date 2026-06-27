use crate::i18n::t;
use crate::render::{asset_row_lines, link_segments, Asset, StyleRun};
use crate::richtext::RichStyle;
use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Parameters for drawing the Detail screen.
pub struct DetailParams<'a> {
    pub lines: &'a [String],
    pub line_styles: &'a [Vec<StyleRun>],
    pub assets: &'a [Asset],
    pub offset: usize,
    pub loading: bool,
    pub task_id: i64,
    pub task_name: &'a str,
}

/// Wrapped panel height shared by the render and model paths.
///
/// Counts the number of terminal rows each asset label occupies (including
/// continuation lines when a label wraps at `inner_width`) and adds 2 for
/// the panel borders, capped at 8.  `inner_width` is the outer content-block
/// inner width (i.e. `viewport_cols - 2`); the function internally subtracts
/// another 2 for the asset-panel border before calling `asset_row_lines`.
///
/// This is the authoritative wrapped-height computation reused by both the
/// renderer (`draw_detail`) and the model hit-test helpers so that no second
/// divergent count can exist.
pub fn asset_panel_render_height(assets: &[Asset], inner_width: usize) -> u16 {
    if assets.is_empty() {
        return 0;
    }
    let panel_inner = inner_width.saturating_sub(2);
    let row_count: usize = assets
        .iter()
        .enumerate()
        .map(|(i, asset)| asset_row_lines(i + 1, asset, panel_inner).len())
        .sum();
    (row_count as u16 + 2).min(8)
}

/// Draw the Detail screen as a single scrollable content block with an optional
/// fixed Artifacts panel below.
///
/// The task name is rendered as a wrapped bold header inside the content block,
/// above the body.  The Block border has no title.
/// When `assets` is non-empty the area is split vertically into a content chunk
/// (Min(0)) and a fixed panel chunk whose height counts wrapped asset rows (capped 8).
pub fn draw_detail(frame: &mut Frame, area: Rect, params: DetailParams<'_>) {
    let inner_width = area.width.saturating_sub(2) as usize;

    if params.loading {
        let fallback_title = if params.task_name.is_empty() {
            format!(" #{} ", params.task_id)
        } else {
            format!(" {} ", params.task_name)
        };
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(fallback_title));
        frame.render_widget(msg, area);
        return;
    }

    let panel_height = asset_panel_render_height(params.assets, inner_width);

    match panel_height {
        0 => render_content(
            frame,
            area,
            params.lines,
            params.line_styles,
            params.offset,
            params.task_name,
        ),
        ph => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(ph)])
                .split(area);
            render_content(
                frame,
                chunks[0],
                params.lines,
                params.line_styles,
                params.offset,
                params.task_name,
            );
            render_assets_panel(frame, chunks[1], params.assets);
        }
    }
}

/// Build a ratatui `Style` for the given `RichStyle` emphasis kind.
fn emphasis_style(rs: RichStyle) -> Style {
    match rs {
        RichStyle::Bold => Style::default().add_modifier(Modifier::BOLD),
        RichStyle::Italic => Style::default().add_modifier(Modifier::ITALIC),
        RichStyle::Code => theme::code_style(),
        RichStyle::Plain => Style::default(),
    }
}

/// Merge emphasis and link styles for a display column.
///
/// Link style wins for `is_link` segments; emphasis is applied on top
/// via add_modifier so both bold+link can coexist when they overlap.
fn merged_cell_style(base_emphasis: Style, is_link: bool) -> Style {
    if is_link {
        let link = theme::link_style();
        Style {
            add_modifier: link.add_modifier | base_emphasis.add_modifier,
            ..link
        }
    } else {
        base_emphasis
    }
}

/// Convert a display-column position to the `RichStyle` that covers it.
///
/// Scans `runs` (sorted by start) and returns the style of the first run
/// whose [start, start+len) interval contains `col`. Returns Plain when no
/// run covers the column.
fn emphasis_at_col(runs: &[StyleRun], col: usize) -> RichStyle {
    for run in runs {
        if col >= run.start && col < run.start + run.len {
            return run.style;
        }
    }
    RichStyle::Plain
}

/// Produce a ratatui `Line` from a plain string and its parallel style runs.
///
/// Applies the existing link coloring from `link_segments` (so `↗ Link N`
/// labels keep their link_style) AND the emphasis runs (Bold/Italic/Code).
/// When a line has no style runs the result is identical to the prior
/// link-only styled_line behavior.
fn styled_line_with_runs(line: &str, runs: &[StyleRun]) -> Line<'static> {
    let segs = link_segments(line);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut col = 0usize;

    for seg in segs {
        let is_link = seg.is_link;
        if runs.is_empty() {
            let span = if is_link {
                Span::styled(seg.text, theme::link_style())
            } else {
                Span::raw(seg.text)
            };
            spans.push(span);
            continue;
        }
        split_segment_by_runs(&seg.text, is_link, col, runs, &mut spans, &mut col);
        continue;
    }

    Line::from(spans)
}

/// Split a single link segment into sub-spans by the emphasis style runs that
/// intersect it, then append those sub-spans to `out`.
///
/// `seg_text` is the plain text of the segment; `is_link` marks it as a URL
/// label; `start_col` is the current display-column cursor before this segment;
/// `col_out` is updated to the cursor position after the segment.
fn split_segment_by_runs(
    seg_text: &str,
    is_link: bool,
    start_col: usize,
    runs: &[StyleRun],
    out: &mut Vec<Span<'static>>,
    col_out: &mut usize,
) {
    use unicode_width::UnicodeWidthChar;

    let mut current_chunk = String::new();
    let mut current_style: Option<Style> = None;
    let mut col = start_col;

    for ch in seg_text.chars() {
        let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0);
        let rich = emphasis_at_col(runs, col);
        let cell_style = merged_cell_style(emphasis_style(rich), is_link);

        match &current_style {
            None => {
                current_style = Some(cell_style);
                current_chunk.push(ch);
            }
            Some(cs) if *cs == cell_style => {
                current_chunk.push(ch);
            }
            Some(_) => {
                flush_span(&current_chunk, current_style.take().unwrap(), out);
                current_chunk.clear();
                current_chunk.push(ch);
                current_style = Some(cell_style);
            }
        }
        col += ch_w;
    }

    if !current_chunk.is_empty() {
        flush_span(&current_chunk, current_style.unwrap_or_default(), out);
    }
    *col_out = col;
}

/// Emit a `Span` to `out`.
///
/// Uses `Span::raw` when the style equals `Style::default()` to avoid
/// unnecessary styling overhead.
fn flush_span(text: &str, style: Style, out: &mut Vec<Span<'static>>) {
    if text.is_empty() {
        return;
    }
    let span = if style == Style::default() {
        Span::raw(text.to_string())
    } else {
        Span::styled(text.to_string(), style)
    };
    out.push(span);
}

fn render_content(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    lines: &[String],
    line_styles: &[Vec<StyleRun>],
    offset: usize,
    task_name: &str,
) {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let inner_width = area.width.saturating_sub(2) as usize;
    let name_wrapped = crate::render::wrap_text(task_name, inner_width);
    let name_line_count = if task_name.is_empty() {
        0usize
    } else {
        name_wrapped.len().max(1)
    };

    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let name_rows = (name_line_count as u16).min(inner.height);
    let body_rows = inner.height.saturating_sub(name_rows);

    let (name_rect, body_rect) = split_inner_for_name(inner, name_rows, body_rows);

    if let Some(nr) = name_rect {
        let name_lines: Vec<Line<'static>> = name_wrapped
            .iter()
            .map(|l| Line::styled(l.clone(), bold))
            .collect();
        frame.render_widget(Paragraph::new(name_lines), nr);
    }

    if body_rect.height == 0 {
        return;
    }

    let text: Text = Text::from(
        lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let runs = line_styles.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
                styled_line_with_runs(l, runs)
            })
            .collect::<Vec<_>>(),
    );

    let viewport_height = body_rect.height as usize;
    let max_offset = lines.len().saturating_sub(viewport_height);
    let eff = offset.min(max_offset);

    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((eff as u16, 0)),
        body_rect,
    );

    let total_content = lines.len();
    if total_content > viewport_height {
        let sb_content = max_offset + 1;
        let mut scrollbar_state = ScrollbarState::new(sb_content)
            .viewport_content_length(viewport_height)
            .position(eff);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn split_inner_for_name(inner: Rect, name_rows: u16, body_rows: u16) -> (Option<Rect>, Rect) {
    if name_rows > 0 && body_rows > 0 {
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(name_rows), Constraint::Min(0)])
            .split(inner);
        (Some(splits[0]), splits[1])
    } else if name_rows > 0 {
        (
            Some(inner),
            Rect::new(inner.x, inner.y + inner.height, inner.width, 0),
        )
    } else {
        (None, inner)
    }
}

fn render_assets_panel(frame: &mut Frame, area: ratatui::layout::Rect, assets: &[Asset]) {
    let panel_title = format!(" {} ", t("Artifacts"));
    let panel_inner_width = area.width.saturating_sub(2) as usize;
    let rows: Vec<Line> = assets
        .iter()
        .enumerate()
        .flat_map(|(i, asset)| {
            asset_row_lines(i + 1, asset, panel_inner_width)
                .into_iter()
                .map(|row_text| Line::styled(row_text, theme::asset_style()))
                .collect::<Vec<_>>()
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
