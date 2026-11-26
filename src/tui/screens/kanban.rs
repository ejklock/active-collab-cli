use crate::board::Phase;
use crate::i18n::t;
use crate::render::{display_width, fmt_hours_or_dash, truncate_to_display_width};
use crate::tui::model::{kanban_columns, relative_due, KanbanFocus, TaskRow};
use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Rows one card takes: name, `project/number`, due date and estimate.
const CARD_ROWS: u16 = 3;
/// Blank rows between two cards.
const CARD_GAP: u16 = 1;
/// The colored bar and the space that follow it, at the left of every card line.
const BAR_COLS: usize = 2;
pub(super) const FLAG: &str = "⚑";

/// The column title of a phase, in the display language.
pub(super) fn phase_title(phase: Phase) -> String {
    match phase {
        Phase::ToDo => t("TO DO"),
        Phase::InProgress => t("IN PROGRESS"),
        Phase::InReview => t("IN REVIEW"),
        Phase::ReadyForRelease => t("READY FOR RELEASE"),
    }
}

/// Width below which four columns no longer fit their titles and card facts.
const NARROW_BELOW: u16 = 100;

/// Draw the mine tasks as four equal columns, one per board phase, in
/// `Phase::ALL` order. The focused column's border and its selected card carry
/// the selection style; each card keeps the order of `tasks`. Below
/// `NARROW_BELOW` columns a row of phase tabs and only the focused column are
/// drawn instead.
pub fn draw_kanban(
    frame: &mut Frame,
    area: Rect,
    tasks: &[TaskRow],
    focus: &KanbanFocus,
    revalidating: bool,
    today: chrono::NaiveDate,
) {
    if area.width < NARROW_BELOW {
        draw_tabs_and_focused_column(frame, area, tasks, focus, revalidating, today);
        return;
    }
    let slots = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(area);
    for (column, (phase, cards)) in Phase::ALL.iter().zip(kanban_columns(tasks)).enumerate() {
        let focused = column == focus.column;
        let selected = focused.then_some(focus.cards[column]);
        let marker = if revalidating && column == 0 {
            " ↻"
        } else {
            ""
        };
        let title = format!(" {} {}{marker} ", phase_title(*phase), cards.len());
        let border = if focused {
            theme::selection_style()
        } else {
            Style::default()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title)
            .title_style(theme::column_header_style());
        let inner = block.inner(slots[column]);
        frame.render_widget(block, slots[column]);
        draw_cards(frame, inner, tasks, &cards, selected, today);
    }
}

/// One tab row with the four phase titles and counts (the focused one
/// highlighted) and, below it, the focused column at the full width.
fn draw_tabs_and_focused_column(
    frame: &mut Frame,
    area: Rect,
    tasks: &[TaskRow],
    focus: &KanbanFocus,
    revalidating: bool,
    today: chrono::NaiveDate,
) {
    let [tab_row, body] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .areas(area);
    let columns = kanban_columns(tasks);
    let mut tabs: Vec<Span> = Vec::new();
    for (column, (phase, cards)) in Phase::ALL.iter().zip(&columns).enumerate() {
        let marker = if revalidating && column == 0 {
            " ↻"
        } else {
            ""
        };
        let style = if column == focus.column {
            theme::selection_style()
        } else {
            theme::column_header_style()
        };
        let text = format!(" {} {}{marker} ", phase_title(*phase), cards.len());
        tabs.push(Span::styled(text, style));
        tabs.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(tabs)), tab_row);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::selection_style());
    let inner = block.inner(body);
    frame.render_widget(block, body);
    draw_cards(
        frame,
        inner,
        tasks,
        &columns[focus.column],
        Some(focus.cards[focus.column]),
        today,
    );
}

/// Draw the cards of one column top to bottom, scrolled so the selected card
/// is on screen.
fn draw_cards(
    frame: &mut Frame,
    area: Rect,
    tasks: &[TaskRow],
    cards: &[usize],
    selected: Option<usize>,
    today: chrono::NaiveDate,
) {
    if area.width as usize <= BAR_COLS || area.height < CARD_ROWS {
        return;
    }
    let stride = CARD_ROWS + CARD_GAP;
    let visible = ((area.height + CARD_GAP) / stride).max(1) as usize;
    let first = selected.map_or(0, |card| (card + 1).saturating_sub(visible));
    for (slot, (card, &index)) in cards.iter().enumerate().skip(first).enumerate() {
        let y = area.y + slot as u16 * stride;
        if y + CARD_ROWS > area.y + area.height {
            break;
        }
        let rect = Rect::new(area.x, y, area.width, CARD_ROWS);
        let lines = card_lines(
            &tasks[index],
            rect.width as usize,
            today,
            selected == Some(card),
        );
        frame.render_widget(Paragraph::new(lines), rect);
    }
}

/// The three lines of a card at `width` columns; the bar takes the due-date
/// color and the selected card is highlighted on every line.
fn card_lines(
    task: &TaskRow,
    width: usize,
    today: chrono::NaiveDate,
    selected: bool,
) -> Vec<Line<'static>> {
    let text_w = width.saturating_sub(BAR_COLS);
    let base = if selected {
        theme::selection_style()
    } else {
        Style::default()
    };
    let (_, due_kind) = relative_due(task.due_on.as_deref(), today);
    let bar = base.patch(theme::due_style(due_kind));
    let line = |text: String| {
        let padding = text_w.saturating_sub(display_width(&text));
        Line::from(vec![
            Span::styled("▌ ", bar),
            Span::styled(format!("{text}{}", " ".repeat(padding)), base),
        ])
    };
    let name = truncate_to_display_width(&task.name, text_w);
    let reference = match task.project_name.as_deref().filter(|p| !p.is_empty()) {
        Some(project) => format!("{project}/{}", task.task_number),
        None => format!("#{}", task.task_number),
    };
    let due = task.due_on.as_deref().unwrap_or("–");
    let mut facts = format!("{due} · {} {}", t("est"), fmt_hours_or_dash(task.estimate));
    if task.flagged {
        facts = format!("{FLAG} {facts}");
    }
    vec![
        line(name),
        line(truncate_to_display_width(&reference, text_w)),
        line(truncate_to_display_width(&facts, text_w)),
    ]
}
