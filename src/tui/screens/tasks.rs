use crate::render::{display_width, wrap_text, PANEL_HPAD};
use crate::tui::model::{ClickTarget, TaskRow};
use crate::tui::theme;
use ratatui::{
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

const BOX_TL: &str = "\u{256D}";
const BOX_TR: &str = "\u{256E}";
const BOX_BL: &str = "\u{2570}";
const BOX_BR: &str = "\u{256F}";
const BOX_H: &str = "\u{2500}";
const BOX_V: &str = "\u{2502}";

/// Width consumed by one card's left+right chrome: 2 border cols + 2×PANEL_HPAD.
const CARD_CHROME: u16 = 2 + 2 * PANEL_HPAD as u16;

/// Draw the Tasks screen (project task list) as stacked bordered cards.
#[allow(clippy::too_many_arguments)]
pub fn draw_tasks(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    project_name: &str,
    tasks: &[TaskRow],
    selected: usize,
    loading: bool,
    revalidating: bool,
    targets: &mut Vec<ClickTarget>,
) {
    let title = if revalidating {
        format!(" {} ↻ ", project_name)
    } else {
        format!(" {} ", project_name)
    };

    if loading {
        let msg = Paragraph::new(crate::i18n::t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_style(theme::column_header_style());
    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    if inner.height == 0 || inner.width == 0 || tasks.is_empty() {
        return;
    }

    let card_inner_w = inner.width.saturating_sub(CARD_CHROME) as usize;
    let card_heights = build_card_heights(tasks, card_inner_w);
    let total_rows: u16 = card_heights.iter().sum();
    let visible_h = inner.height;

    let first_visible = first_visible_card(&card_heights, selected, visible_h);

    render_cards(
        frame,
        inner,
        tasks,
        &card_heights,
        first_visible,
        selected,
        card_inner_w,
        targets,
    );

    if total_rows > visible_h {
        let total_cards = tasks.len();
        let mut sb_state = ScrollbarState::new(total_cards).position(selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}

/// Return the height (in buffer rows) of each task card.
///
/// Each card = 2 border rows + max(1, wrapped content line count).
fn build_card_heights(tasks: &[TaskRow], card_inner_w: usize) -> Vec<u16> {
    tasks
        .iter()
        .map(|t| card_height_for(t, card_inner_w))
        .collect()
}

/// Height of a single task card: 2 border rows + number of wrapped content lines.
fn card_height_for(task: &TaskRow, card_inner_w: usize) -> u16 {
    let content = task_card_content(task);
    let lines = wrap_text(&content, card_inner_w.max(1));
    let body_rows = if lines.is_empty() { 1 } else { lines.len() };
    2 + body_rows as u16
}

/// Build the first-line content string for a task card: `#<number>  <name>`.
fn task_card_content(task: &TaskRow) -> String {
    format!("#{}  {}", task.task_number, task.name)
}

/// Compute the first-visible card index so the selected card is fully on screen.
///
/// Scans forward from card 0; returns the smallest first-visible index such that
/// the selected card's bottom row is within `visible_h`. Falls back to 0 when
/// `selected` is 0 or the card fits without scrolling.
fn first_visible_card(heights: &[u16], selected: usize, visible_h: u16) -> usize {
    if selected == 0 || visible_h == 0 {
        return 0;
    }

    // Cumulative y offsets (card i starts at cum[i])
    let mut cum: Vec<u16> = Vec::with_capacity(heights.len());
    let mut acc = 0u16;
    for &h in heights {
        cum.push(acc);
        acc = acc.saturating_add(h);
    }

    let sel_start = cum[selected];
    let sel_end = sel_start + heights[selected];

    // If selected card fits from offset 0, no scrolling needed
    if sel_end <= visible_h {
        return 0;
    }

    // Find the smallest first_visible such that sel_end fits in the window.
    // Walk first_visible forward until window [cum[first], cum[first]+visible_h) contains sel_end.
    for (first, &start) in cum.iter().enumerate().take(selected + 1) {
        let window_end = start.saturating_add(visible_h);
        if sel_end <= window_end {
            return first;
        }
    }

    selected
}

/// Render visible task cards into `area` and record click targets.
#[allow(clippy::too_many_arguments)]
fn render_cards(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    tasks: &[TaskRow],
    heights: &[u16],
    first_visible: usize,
    selected: usize,
    card_inner_w: usize,
    targets: &mut Vec<ClickTarget>,
) {
    targets.clear();

    let mut y = area.y;
    let bottom = area.y + area.height;

    for (i, (task, &h)) in tasks
        .iter()
        .zip(heights.iter())
        .enumerate()
        .skip(first_visible)
    {
        if y >= bottom {
            break;
        }
        let visible_rows = h.min(bottom.saturating_sub(y));

        let card_rect = ratatui::layout::Rect::new(area.x, y, area.width, visible_rows);
        let is_selected = i == selected;
        render_single_card(frame, card_rect, task, card_inner_w, is_selected);

        let absolute_y_start = y;
        let absolute_y_end = y + visible_rows;
        targets.push(ClickTarget {
            y_start: absolute_y_start,
            y_end: absolute_y_end,
            index: i,
        });

        y += h;
    }
}

/// Render a single task card into `card_rect`.
///
/// Builds top-border, content, and bottom-border lines as a `Text` block.
/// The selected card is styled with `selection_style()` so its entire area
/// (borders + content) carries the highlight background.
fn render_single_card(
    frame: &mut Frame,
    card_rect: ratatui::layout::Rect,
    task: &TaskRow,
    card_inner_w: usize,
    is_selected: bool,
) {
    let content = task_card_content(task);
    let wrapped = wrap_text(&content, card_inner_w.max(1));
    let body_lines = if wrapped.is_empty() {
        vec![String::new()]
    } else {
        wrapped
    };

    let inner_w_cols = card_inner_w + 2 * PANEL_HPAD;
    let top_border = format!("{}{}{}", BOX_TL, BOX_H.repeat(inner_w_cols), BOX_TR);
    let bot_border = format!("{}{}{}", BOX_BL, BOX_H.repeat(inner_w_cols), BOX_BR);

    let hpad = " ".repeat(PANEL_HPAD);
    let base_style = if is_selected {
        theme::selection_style()
    } else {
        ratatui::style::Style::default()
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::styled(top_border, base_style));
    for body in &body_lines {
        let fitted = fit_to_card_width(body, card_inner_w);
        let content_text = format!("{}{}{}{}{}", BOX_V, hpad, fitted, hpad, BOX_V);
        lines.push(Line::styled(content_text, base_style));
    }
    lines.push(Line::styled(bot_border, base_style));

    let text = Text::from(lines);
    frame.render_widget(Paragraph::new(text), card_rect);
}

/// Pad or truncate `s` to exactly `width` display columns.
fn fit_to_card_width(s: &str, width: usize) -> String {
    let w = display_width(s);
    if w >= width {
        truncate_to_display_width(s, width)
    } else {
        let padding = width - w;
        format!("{}{}", s, " ".repeat(padding))
    }
}

/// Truncate `s` to at most `width` display columns.
fn truncate_to_display_width(s: &str, width: usize) -> String {
    let mut acc = 0usize;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > width {
            break;
        }
        result.push(ch);
        acc += cw;
    }
    result
}
