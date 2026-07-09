use crate::render::{link_segments, Asset, StyleRun};
use crate::richtext::RichStyle;
use crate::tui::theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Parameters for drawing the Detail screen.
pub struct DetailParams<'a> {
    pub lines: &'a [String],
    pub line_styles: &'a [Vec<StyleRun>],
    // populated by view.rs; retained for call-site stability (view.rs is out of scope)
    #[allow(dead_code)]
    pub assets: &'a [Asset],
    pub offset: usize,
    pub loading: bool,
    pub task_id: i64,
    pub task_name: &'a str,
    /// Index of the focused comment card; `None` when nothing is focused.
    pub focused_comment: Option<usize>,
    /// Per-card line ranges `(start_line, line_count)` in global `lines`.
    pub comment_spans: &'a [(usize, usize)],
}

/// Draw the Detail screen as a single globally-scrollable bordered content block.
///
/// The content block renders `lines` directly (the Título meta row inside the
/// Details panel carries the task name). The block border has no title.
/// Assets are already part of `lines` via `build_detail_content`, so this
/// function always calls `render_content` on the full area — there is no
/// separate Artifacts panel or vertical split.
pub fn draw_detail(frame: &mut Frame, area: Rect, params: DetailParams<'_>) {
    if params.loading {
        let fallback_title = if params.task_name.is_empty() {
            format!(" #{} ", params.task_id)
        } else {
            format!(" {} ", params.task_name)
        };
        let msg = Paragraph::new(crate::i18n::t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(fallback_title));
        frame.render_widget(msg, area);
        return;
    }

    let focused_span = params
        .focused_comment
        .and_then(|idx| params.comment_spans.get(idx).copied());
    render_content(
        frame,
        area,
        params.lines,
        params.line_styles,
        params.offset,
        focused_span,
    );
}

/// Build a ratatui `Style` for the given `RichStyle` emphasis kind.
fn emphasis_style(rs: RichStyle) -> Style {
    match rs {
        RichStyle::Bold => Style::default().add_modifier(Modifier::BOLD),
        RichStyle::Italic => Style::default().add_modifier(Modifier::ITALIC),
        RichStyle::Code => theme::code_style(),
        RichStyle::Strike => Style::default().add_modifier(Modifier::CROSSED_OUT),
        RichStyle::Underline => Style::default().add_modifier(Modifier::UNDERLINED),
        RichStyle::Link => theme::link_style(),
        RichStyle::EditAffordance => theme::edit_affordance_style(),
        RichStyle::DeleteAffordance => theme::delete_affordance_style(),
        RichStyle::PanelTitle => theme::panel_title_style(),
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
    focused_span: Option<(usize, usize)>,
) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let focus_style = theme::focused_comment_style();
    let text: Text = Text::from(
        lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let runs = line_styles.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
                let line = styled_line_with_runs(l, runs);
                if is_in_focused_span(i, focused_span) {
                    line.patch_style(focus_style)
                } else {
                    line
                }
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

/// Return true when global line index `i` falls within the focused card span.
fn is_in_focused_span(i: usize, span: Option<(usize, usize)>) -> bool {
    match span {
        Some((start, count)) => i >= start && i < start + count,
        None => false,
    }
}
