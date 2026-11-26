use super::kanban::phase_title;
use super::tasks::card_rows;
use crate::board::Phase;
use crate::render::{display_width, BOX_BL, BOX_BR, BOX_H, BOX_TL, BOX_TR, BOX_V};
use crate::tui::model::{relative_due, KanbanFocus, TaskRow};
use crate::tui::task_layout::{self, PhaseSlot};
use crate::tui::theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

/// The thin line between two cards of a box.
const SEPARATOR: &str = "┄";
/// The colored bar and the space that follow it, at the left of every card line.
const BAR: &str = "▌ ";

/// Draw the mine tasks as one vertical list grouped by board phase: a bordered
/// box per phase in `Phase::ALL` order, its title and count in the top border,
/// with its cards stacked inside at full width and told apart by a dashed line.
/// The list scrolls so the selected card stays on screen; a box cut by the top
/// or bottom of the area shows only its visible rows.
pub fn draw_kanban_vertical(
    frame: &mut Frame,
    area: Rect,
    tasks: &[TaskRow],
    focus: &KanbanFocus,
    revalidating: bool,
    today: chrono::NaiveDate,
) {
    let outer = Block::default().borders(Borders::ALL);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let text_w = task_layout::boxed_text_w(inner.width);
    let slots = task_layout::phase_slots(tasks);
    let heights: Vec<u16> = slots
        .iter()
        .map(|&slot| task_layout::phase_slot_height(slot, tasks, text_w))
        .collect();
    let selected_task = focus.selected_task(tasks);
    let selected_slot = slots
        .iter()
        .position(
            |slot| matches!(slot, PhaseSlot::Card { index, .. } if Some(*index) == selected_task),
        )
        .unwrap_or(0);
    let first = task_layout::first_visible_linear(&heights, selected_slot, inner.height);

    let bottom = inner.y + inner.height;
    let mut y = inner.y;
    for (slot, &height) in slots.iter().zip(&heights).skip(first) {
        if y >= bottom {
            break;
        }
        let rect = Rect::new(inner.x, y, inner.width, height.min(bottom - y));
        match *slot {
            PhaseSlot::Top { phase, count } => {
                let marker = if revalidating && phase == 0 {
                    " ↻"
                } else {
                    ""
                };
                let title = format!("{} {count}{marker}", phase_title(Phase::ALL[phase]));
                frame.render_widget(Paragraph::new(top_border(&title, rect.width)), rect);
            }
            PhaseSlot::Card { index, separated } => {
                let selected = Some(index) == selected_task;
                let sides = Block::default().borders(Borders::LEFT | Borders::RIGHT);
                let content = sides.inner(rect);
                frame.render_widget(sides, rect);
                let mut lines = card_lines(&tasks[index], text_w, today, selected);
                if separated {
                    lines.push(Line::raw(SEPARATOR.repeat(content.width as usize)));
                }
                frame.render_widget(Paragraph::new(lines), content);
            }
            PhaseSlot::Empty => {
                let inside = " ".repeat(rect.width.saturating_sub(2) as usize);
                let line = Line::raw(format!("{BOX_V}{inside}{BOX_V}"));
                frame.render_widget(Paragraph::new(line), rect);
            }
            PhaseSlot::Bottom { .. } => {
                let fill = BOX_H.repeat(rect.width.saturating_sub(2) as usize);
                let line = Line::raw(format!("{BOX_BL}{fill}{BOX_BR}"));
                frame.render_widget(Paragraph::new(line), rect);
            }
        }
        y += height;
    }

    let total_rows: u32 = heights.iter().map(|&h| u32::from(h)).sum();
    if total_rows > u32::from(inner.height) {
        let mut state = ScrollbarState::new(slots.len()).position(selected_slot);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut state);
    }
}

/// The top border of a box `width` columns wide with `title` set into it.
fn top_border(title: &str, width: u16) -> Line<'static> {
    let label = format!(" {title} ");
    let fill = (width as usize).saturating_sub(3 + display_width(&label));
    Line::from(vec![
        Span::raw(format!("{BOX_TL}{BOX_H}")),
        Span::styled(label, theme::column_header_style()),
        Span::raw(format!("{}{BOX_TR}", BOX_H.repeat(fill))),
    ])
}

/// The lines of a card without a border: title rows, hours and due date, each
/// behind the due-date colored bar; the selected card is highlighted on every
/// line.
fn card_lines(
    task: &TaskRow,
    text_w: usize,
    today: chrono::NaiveDate,
    selected: bool,
) -> Vec<Line<'static>> {
    let base = if selected {
        theme::selection_style()
    } else {
        Style::default()
    };
    let (_, due_kind) = relative_due(task.due_on.as_deref(), today);
    let bar_style = base.patch(theme::due_style(due_kind));
    card_rows(task, text_w, base, today, true)
        .into_iter()
        .map(|row| {
            let mut spans = vec![Span::styled(BAR, bar_style)];
            spans.extend(row);
            Line::from(spans)
        })
        .collect()
}
