//! Pure layout math for the Tasks screen's stacked task cards.
//!
//! Mirrors the `detail_geometry` split: this module owns the card-content
//! string, card-height arithmetic, card inner-width derivation, the
//! first-visible-card scroll math, the due-date bucket starts, the slot heights
//! (card height plus the header row of a bucket's first card) and the bucket
//! header text, all as pure functions over primitives.
//! `screens::tasks` stays draw-only and calls into this module.

use crate::i18n::t;
use crate::render::buckets::{bucket_sums, due_bucket, DueBucket};
use crate::render::{fmt_hours_or_dash, wrap_text, PANEL_HPAD};
use crate::tui::model::{kanban_columns, TaskRow};
use chrono::NaiveDate;

/// Width consumed by one card's left+right chrome: 2 border cols + 2×PANEL_HPAD.
pub(crate) const CARD_CHROME: u16 = 2 + 2 * PANEL_HPAD as u16;

/// Build the first-line content string for a task card: `#<number>  <name>`.
///
/// Single source of truth: both the height cache (`card_height`) and the
/// renderer derive their content from this function, so they can never drift.
pub(crate) fn card_content(task: &TaskRow) -> String {
    format!("#{}  {}", task.task_number, task.name)
}

/// Build the hours content line of a task card: estimate, the user's hours
/// this week and the task's total hours, each `–` when unknown.
pub(crate) fn hours_line(task: &TaskRow) -> String {
    format!(
        "{} {} · {} {} · {} {}",
        t("est"),
        fmt_hours_or_dash(task.estimate),
        t("week"),
        fmt_hours_or_dash(task.logged_week),
        t("total"),
        fmt_hours_or_dash(task.logged_total),
    )
}

/// Card height for a single task: 2 border rows + wrapped title rows + 1 hours row
/// + 1 due-date row.
pub(crate) fn card_height(task: &TaskRow, card_inner_w: usize) -> u16 {
    let content = card_content(task);
    let lines = wrap_text(&content, card_inner_w.max(1));
    let body_rows = if lines.is_empty() { 1 } else { lines.len() };
    2 + body_rows as u16 + 2
}

/// The bucket each task opens: `Some(bucket)` at the first task of every
/// non-empty bucket, `None` elsewhere. A list without a grouping date has no
/// buckets, so every entry is `None`. The tasks are already in bucket order.
pub(crate) fn bucket_starts(tasks: &[TaskRow], today: Option<NaiveDate>) -> Vec<Option<DueBucket>> {
    let Some(today) = today else {
        return vec![None; tasks.len()];
    };
    let mut previous = None;
    tasks
        .iter()
        .map(|task| {
            let bucket = due_bucket(task.due_on.as_deref(), today);
            let opens = previous != Some(bucket);
            previous = Some(bucket);
            opens.then_some(bucket)
        })
        .collect()
}

/// Rows each task occupies in the list: its card, plus the bucket header row
/// above the first task of a bucket.
pub(crate) fn slot_heights(
    tasks: &[TaskRow],
    card_inner_w: usize,
    today: Option<NaiveDate>,
) -> Vec<u16> {
    tasks
        .iter()
        .zip(bucket_starts(tasks, today))
        .map(|(task, opens)| card_height(task, card_inner_w) + u16::from(opens.is_some()))
        .collect()
}

/// Columns a boxed card spends beside its text: the box's two side borders and
/// the due-bucket bar with its space.
const BOXED_CARD_CHROME: u16 = 2 + 2;

/// Width of a boxed card's text inside a box area `inner_width` columns wide.
pub(crate) fn boxed_text_w(inner_width: u16) -> usize {
    inner_width.saturating_sub(BOXED_CARD_CHROME) as usize
}

/// Rows of a card drawn without its own border: the wrapped title, the hours
/// line and the due-date line.
pub(crate) fn boxed_card_height(task: &TaskRow, text_w: usize) -> u16 {
    let title_rows = wrap_text(&card_content(task), text_w.max(1)).len().max(1);
    title_rows as u16 + 2
}

/// One row-group of the vertical kanban: a piece of a phase's bordered box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhaseSlot {
    /// The top border of the box of the phase at this `Phase::ALL` position,
    /// carrying the title over `count` cards.
    Top { phase: usize, count: usize },
    /// The card of the task at this index into the task list; `separated` when
    /// another card follows in the same box, so a dashed line closes this one.
    Card { index: usize, separated: bool },
    /// The blank row that keeps a box with no card visible.
    Empty,
    /// The bottom border of a box; `gap` adds the blank row before the next box.
    Bottom { gap: bool },
}

/// The vertical kanban top to bottom: for each phase in `Phase::ALL` order its
/// box, made of the top border, the cards in the order of `tasks` (or the blank
/// row of an empty phase) and the bottom border.
pub(crate) fn phase_slots(tasks: &[TaskRow]) -> Vec<PhaseSlot> {
    let columns = kanban_columns(tasks);
    let last = columns.len().saturating_sub(1);
    let mut slots = Vec::new();
    for (phase, cards) in columns.iter().enumerate() {
        slots.push(PhaseSlot::Top {
            phase,
            count: cards.len(),
        });
        if cards.is_empty() {
            slots.push(PhaseSlot::Empty);
        }
        for (position, &index) in cards.iter().enumerate() {
            slots.push(PhaseSlot::Card {
                index,
                separated: position + 1 < cards.len(),
            });
        }
        slots.push(PhaseSlot::Bottom { gap: phase < last });
    }
    slots
}

/// Rows a slot occupies, for card text `text_w` columns wide.
pub(crate) fn phase_slot_height(slot: PhaseSlot, tasks: &[TaskRow], text_w: usize) -> u16 {
    match slot {
        PhaseSlot::Top { .. } | PhaseSlot::Empty => 1,
        PhaseSlot::Card { index, separated } => {
            boxed_card_height(&tasks[index], text_w) + u16::from(separated)
        }
        PhaseSlot::Bottom { gap } => 1 + u16::from(gap),
    }
}

/// The header text of the bucket that starts at `start`, with the sums of the
/// bucket's current tasks.
pub(crate) fn bucket_header(
    tasks: &[TaskRow],
    start: usize,
    bucket: DueBucket,
    today: NaiveDate,
) -> String {
    let members: Vec<&TaskRow> = tasks[start..]
        .iter()
        .take_while(|task| due_bucket(task.due_on.as_deref(), today) == bucket)
        .collect();
    let sums = bucket_sums(&members);
    format!(
        "{} ({}) · {} {} · {} {} · {} {}",
        bucket.label(),
        members.len(),
        t("est"),
        fmt_hours_or_dash(sums.estimate),
        t("week"),
        fmt_hours_or_dash(sums.logged_week),
        t("total"),
        fmt_hours_or_dash(sums.logged_total),
    )
}

/// Compute the card inner width from the full terminal width.
///
/// The outer Tasks block has 1-col borders each side (2 total), and each card
/// adds `CARD_CHROME` more columns. This is the single source of truth used by
/// both `reflow_tasks` (pre-draw in the shell) and `draw_tasks` (render pass).
pub(crate) fn inner_w(terminal_width: u16) -> usize {
    terminal_width.saturating_sub(2).saturating_sub(CARD_CHROME) as usize
}

/// Compute the first-visible card index so the selected card is fully on screen.
///
/// Uses a binary search over the prefix-sum offsets (O(log T)) when the cache
/// is valid. Falls back to a linear walk on the inline-computed heights when
/// the cache does not match the current width (defensive floor).
///
/// Preserves the exact semantics of the previous linear implementation:
/// - Returns 0 when `selected == 0` or `visible_h == 0`.
/// - Returns 0 when the selected card fits without scrolling.
/// - Otherwise returns the smallest first-visible index such that `sel_end`
///   fits within `first_start + visible_h`.
pub(crate) fn first_visible(
    cache_offsets: &[u32],
    cache_rendered_width: usize,
    card_inner_w: usize,
    inline_heights: &[u16],
    selected: usize,
    visible_h: u16,
) -> usize {
    if selected == 0 || visible_h == 0 {
        return 0;
    }

    let cache_valid =
        cache_rendered_width == card_inner_w && cache_offsets.len() == inline_heights.len() + 1;

    if cache_valid {
        first_visible_binary(cache_offsets, selected, visible_h)
    } else {
        first_visible_linear(inline_heights, selected, visible_h)
    }
}

/// Binary search over the prefix-sum offsets to find the first-visible card index.
pub(crate) fn first_visible_binary(offsets: &[u32], selected: usize, visible_h: u16) -> usize {
    let sel_end = offsets[selected + 1];
    let visible_h32 = visible_h as u32;

    if sel_end <= visible_h32 {
        return 0;
    }

    // Find the smallest first in 0..=selected such that offsets[first] + visible_h >= sel_end.
    let first = offsets[..=selected].partition_point(|&start| start + visible_h32 < sel_end);
    first.min(selected)
}

/// Linear walk fallback used when the prefix-sum cache is not valid for this width.
pub(crate) fn first_visible_linear(heights: &[u16], selected: usize, visible_h: u16) -> usize {
    let mut cum: Vec<u16> = Vec::with_capacity(heights.len());
    let mut acc = 0u16;
    for &h in heights {
        cum.push(acc);
        acc = acc.saturating_add(h);
    }

    let sel_start = cum[selected];
    let sel_end = sel_start.saturating_add(heights[selected]);

    if sel_end <= visible_h {
        return 0;
    }

    for (first, &start) in cum.iter().enumerate().take(selected + 1) {
        let window_end = start.saturating_add(visible_h);
        if sel_end <= window_end {
            return first;
        }
    }

    selected
}
