use crate::i18n::t;
use crate::render::{asset_row_lines, link_segments, Asset, StyleRun, PANEL_HPAD, PANEL_VPAD};
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

/// Height ceiling that bounds the asset card; sized to clear a common spaced
/// multi-link card (4 rows + 3 separators + 2 vpad + 2 borders = 11).
const ASSET_PANEL_MAX_ROWS: u16 = 14;

/// Extra rows appended inside the Artifacts card after the asset list:
/// one blank separator row + one italic footnote hint line.
///
/// Placed AFTER the asset rows so `asset_index_at_panel_row`'s asset walk
/// requires no modification; clicks on these rows resolve to None.
pub const ASSET_HINT_ROWS: u16 = 2;

/// Width available for asset content rows inside the panel.
///
/// Removes `2 * PANEL_HPAD` from `panel_inner_width` so label text clears both
/// border insets.  `panel_inner_width` is the panel area width minus 2 border
/// columns (i.e. the inner ratatui content width for the asset panel block).
fn asset_content_width(panel_inner_width: usize) -> usize {
    panel_inner_width.saturating_sub(2 * PANEL_HPAD)
}

/// Wrapped panel height shared by the render and model paths.
///
/// Sums the wrapped row count for every asset (using `asset_content_width` so
/// the wrap matches the renderer exactly), adds one blank separator row between
/// consecutive assets, adds `PANEL_VPAD` blank rows at the interior top and
/// bottom, and adds 2 for the panel borders.  The total is capped at
/// `ASSET_PANEL_MAX_ROWS`.
///
/// `inner_width` is the asset panel's inner content width, equal to the chunk
/// width minus 2 border columns (i.e. the ratatui Paragraph area width for the
/// asset panel block).  Pass `area.width.saturating_sub(2)` from the renderer
/// chunk, or `viewport_cols.saturating_sub(2)` from the model.
///
/// This is the authoritative wrapped-height computation reused by both the
/// renderer (`draw_detail`) and the model hit-test helpers so that no second
/// divergent count can exist.
pub fn asset_panel_render_height(assets: &[Asset], inner_width: usize) -> u16 {
    if assets.is_empty() {
        return 0;
    }
    let content_w = asset_content_width(inner_width);
    let row_count: usize = assets
        .iter()
        .enumerate()
        .map(|(i, asset)| asset_row_lines(i + 1, asset, content_w).len())
        .sum();
    let separators = assets.len().saturating_sub(1);
    let capped = (row_count as u16 + separators as u16 + 2 * PANEL_VPAD as u16 + 2)
        .min(ASSET_PANEL_MAX_ROWS);
    capped + ASSET_HINT_ROWS
}

/// Draw the Detail screen as a single scrollable content block with an optional
/// fixed Artifacts panel below.
///
/// The content block renders `lines` directly (the Título meta row inside the
/// Details panel carries the task name).  The block border has no title.
/// When `assets` is non-empty the area is split vertically into a content chunk
/// (Min(0)) and a fixed panel chunk whose height is given by `asset_panel_render_height`
/// (capped at `ASSET_PANEL_MAX_ROWS`).
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
        0 => render_content(frame, area, params.lines, params.line_styles, params.offset),
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
        RichStyle::Strike => Style::default().add_modifier(Modifier::CROSSED_OUT),
        RichStyle::Underline => Style::default().add_modifier(Modifier::UNDERLINED),
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
) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
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

    let viewport_height = inner.height as usize;
    let max_offset = lines.len().saturating_sub(viewport_height);
    let eff = offset.min(max_offset);

    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((eff as u16, 0)),
        inner,
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

fn render_assets_panel(frame: &mut Frame, area: ratatui::layout::Rect, assets: &[Asset]) {
    let panel_title = format!(" {} ", t("Artifacts"));
    let panel_inner_width = area.width.saturating_sub(2) as usize;
    let content_w = asset_content_width(panel_inner_width);
    let hpad = " ".repeat(PANEL_HPAD);

    let mut rows: Vec<Line> = Vec::new();

    for _ in 0..PANEL_VPAD {
        rows.push(Line::raw(""));
    }

    for (i, asset) in assets.iter().enumerate() {
        if i > 0 {
            rows.push(Line::raw(""));
        }
        for row_text in asset_row_lines(i + 1, asset, content_w) {
            rows.push(Line::from(vec![
                Span::raw(hpad.clone()),
                Span::styled(row_text, theme::asset_style()),
            ]));
        }
    }

    rows.push(Line::raw(""));
    rows.push(Line::from(vec![
        Span::raw(hpad.clone()),
        Span::styled(t("Ctrl/Cmd+click to open"), theme::asset_hint_style()),
    ]));

    for _ in 0..PANEL_VPAD {
        rows.push(Line::raw(""));
    }

    let panel = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::ALL)
            .title(panel_title)
            .title_style(theme::header_style()),
    );

    frame.render_widget(panel, area);
}
