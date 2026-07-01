//! Pure layout math for the Tasks screen's stacked task cards.
//!
//! Mirrors the `detail_geometry` split: this module owns the card-content
//! string, card-height arithmetic, card inner-width derivation, and the
//! first-visible-card scroll math, all as pure functions over primitives.
//! `screens::tasks` stays draw-only and calls into this module.

use crate::render::{wrap_text, PANEL_HPAD};
use crate::tui::model::TaskRow;

/// Width consumed by one card's left+right chrome: 2 border cols + 2×PANEL_HPAD.
pub(crate) const CARD_CHROME: u16 = 2 + 2 * PANEL_HPAD as u16;

/// Build the first-line content string for a task card: `#<number>  <name>`.
///
/// Single source of truth: both the height cache (`card_height`) and the
/// renderer derive their content from this function, so they can never drift.
pub(crate) fn card_content(task: &TaskRow) -> String {
    format!("#{}  {}", task.task_number, task.name)
}

/// Card height for a single task: 2 border rows + wrapped title rows + 1 due-date row.
pub(crate) fn card_height(task: &TaskRow, card_inner_w: usize) -> u16 {
    let content = card_content(task);
    let lines = wrap_text(&content, card_inner_w.max(1));
    let body_rows = if lines.is_empty() { 1 } else { lines.len() };
    2 + body_rows as u16 + 1
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
