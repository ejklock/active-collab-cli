use crate::render::{
    display_width, truncate_to_display_width, wrap_text, BOX_BL, BOX_BR, BOX_H, BOX_TL, BOX_TR,
    BOX_V, PANEL_HPAD,
};
use crate::tui::model::{relative_due, ClickTarget, TaskRow};
use crate::tui::task_layout;
use crate::tui::theme;
use ratatui::{
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

/// Draw the Tasks screen (project task list) as stacked bordered cards.
///
/// `card_heights` and `card_offsets` are the memoized layout cache from
/// `Screen::Tasks`; `rendered_width` is the cache's build width. When the
/// cache does not match the current `card_inner_w` (defensive floor), heights
/// are computed inline for that one frame so rendering is never incorrect.
#[allow(clippy::too_many_arguments)]
pub fn draw_tasks(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    project_name: &str,
    tasks: &[TaskRow],
    selected: usize,
    loading: bool,
    revalidating: bool,
    today: chrono::NaiveDate,
    targets: &mut Vec<ClickTarget>,
    card_heights_cache: &[u16],
    card_offsets_cache: &[u32],
    cache_rendered_width: usize,
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

    let card_inner_w = task_layout::inner_w(inner.width + 2);
    let visible_h = inner.height;

    let (card_heights, total_rows) = resolve_heights(
        tasks,
        card_inner_w,
        card_heights_cache,
        card_offsets_cache,
        cache_rendered_width,
    );

    let first_visible = task_layout::first_visible(
        card_offsets_cache,
        cache_rendered_width,
        card_inner_w,
        &card_heights,
        selected,
        visible_h,
    );

    render_cards(
        frame,
        inner,
        tasks,
        &card_heights,
        first_visible,
        selected,
        card_inner_w,
        today,
        targets,
    );

    if total_rows > visible_h as u32 {
        let total_cards = tasks.len();
        let mut sb_state = ScrollbarState::new(total_cards).position(selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut sb_state);
    }
}

/// Resolve card heights and total row count from the cache when valid, or compute inline.
///
/// Returns `(heights, total_rows_u32)`. The defensive floor activates when
/// `cache_rendered_width != card_inner_w` or the cache length does not match
/// the task count, ensuring a width-derivation mismatch never produces wrong output.
fn resolve_heights<'a>(
    tasks: &[TaskRow],
    card_inner_w: usize,
    cache_heights: &'a [u16],
    cache_offsets: &[u32],
    cache_rendered_width: usize,
) -> (std::borrow::Cow<'a, [u16]>, u32) {
    let cache_valid = cache_rendered_width == card_inner_w
        && cache_heights.len() == tasks.len()
        && cache_offsets.len() == tasks.len() + 1;

    if cache_valid {
        let total = *cache_offsets.last().unwrap_or(&0);
        return (std::borrow::Cow::Borrowed(cache_heights), total);
    }

    let heights: Vec<u16> = tasks
        .iter()
        .map(|t| task_layout::card_height(t, card_inner_w))
        .collect();
    let total: u32 = heights.iter().map(|&h| h as u32).sum();
    (std::borrow::Cow::Owned(heights), total)
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
    today: chrono::NaiveDate,
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
        render_single_card(frame, card_rect, task, card_inner_w, is_selected, today);

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
/// Builds top-border, content (line 1), due-date (line 2), and bottom-border
/// lines as a `Text` block. The selected card carries the selection highlight
/// base style on every row; the due-date fg color (red/yellow) is layered over
/// the base so urgency color remains visible even on the selection background.
fn render_single_card(
    frame: &mut Frame,
    card_rect: ratatui::layout::Rect,
    task: &TaskRow,
    card_inner_w: usize,
    is_selected: bool,
    today: chrono::NaiveDate,
) {
    let content = task_layout::card_content(task);
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
        Style::default()
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::styled(top_border, base_style));
    for body in &body_lines {
        let fitted = fit_to_card_width(body, card_inner_w);
        let content_text = format!("{}{}{}{}{}", BOX_V, hpad, fitted, hpad, BOX_V);
        lines.push(Line::styled(content_text, base_style));
    }
    lines.push(due_line(task, card_inner_w, &hpad, base_style, today));
    lines.push(Line::styled(bot_border, base_style));

    let text = Text::from(lines);
    frame.render_widget(Paragraph::new(text), card_rect);
}

/// Build the due-date content line (line 2) for a task card.
///
/// The due fg color (red/yellow/default) is applied over the base_style background
/// so the urgency color survives even when the card is selected (amber bg).
/// When project_name is Some(non-empty), appends ' · <project>' after the due text
/// in the default style (no urgency color on the project portion).
fn due_line(
    task: &TaskRow,
    card_inner_w: usize,
    hpad: &str,
    base_style: Style,
    today: chrono::NaiveDate,
) -> Line<'static> {
    let (due_text, due_kind) = relative_due(task.due_on.as_deref(), today);
    let due_fg = theme::due_style(due_kind);
    // Merge: keep the base bg (selection or default) but override fg with urgency color.
    let due_cell_style = base_style.patch(due_fg);

    let project = task.project_name.as_deref().filter(|n| !n.is_empty());

    match project {
        Some(proj) => {
            let separator = " \u{00B7} ";
            let composed = format!("{due_text}{separator}{proj}");
            let (due_part, proj_part) =
                split_due_project(&due_text, separator, proj, &composed, card_inner_w);
            let used = display_width(&due_part) + display_width(&proj_part);
            let padding = card_inner_w.saturating_sub(used);
            Line::from(vec![
                Span::styled(format!("{BOX_V}{hpad}"), base_style),
                Span::styled(due_part, due_cell_style),
                Span::styled(proj_part, base_style),
                Span::styled(format!("{}{hpad}{BOX_V}", " ".repeat(padding)), base_style),
            ])
        }
        None => {
            let fitted = fit_to_card_width(&due_text, card_inner_w);
            Line::from(vec![
                Span::styled(format!("{BOX_V}{hpad}"), base_style),
                Span::styled(fitted, due_cell_style),
                Span::styled(format!("{hpad}{BOX_V}"), base_style),
            ])
        }
    }
}

/// Split the composed `due · project` string into (due_part, proj_part) that together
/// fit within `card_inner_w` display columns.
///
/// Preserves the due text at full width; truncates only the project suffix when
/// the total exceeds the card width. When even the due text alone would exceed
/// the width, the project part is simply omitted.
fn split_due_project(
    due_text: &str,
    separator: &str,
    proj: &str,
    composed: &str,
    card_inner_w: usize,
) -> (String, String) {
    let total_w = display_width(composed);
    if total_w <= card_inner_w {
        return (due_text.to_string(), format!("{separator}{proj}"));
    }
    let due_w = display_width(due_text);
    let sep_w = display_width(separator);
    let available_for_proj = card_inner_w.saturating_sub(due_w + sep_w);
    if available_for_proj == 0 {
        // Not enough room for separator + any project chars; return only due text.
        return (
            truncate_to_display_width(due_text, card_inner_w),
            String::new(),
        );
    }
    let truncated_proj = truncate_to_display_width(proj, available_for_proj);
    (due_text.to_string(), format!("{separator}{truncated_proj}"))
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
