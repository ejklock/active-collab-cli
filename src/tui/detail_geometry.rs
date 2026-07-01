/// The first viewport row occupied by scrollable body text in the Detail screen.
///
/// Why 2: row 0 is the outer block top border, row 1 is the header bar row.
/// Rows 0..DETAIL_TEXT_TOP are chrome that is never part of the scrollable body.
pub(crate) const DETAIL_TEXT_TOP: u16 = 2;

/// Rows consumed by the Detail content block's chrome that are not scrollable text.
/// Breakdown: 1 top border + 1 bottom border + 1 header bar row + 1 footer bar row.
pub(crate) const DETAIL_CHROME_ROWS: u16 = 4;

/// Left-border columns added by the ratatui `Block::borders(ALL)` that `render_content`
/// wraps the body behind. This border is NOT part of the boxed panel lines stored in
/// `Screen::Detail.lines`; those lines carry their own `│ … │` chrome counted in
/// `BODY_LEFT_CHROME_COLS`. The full absolute-frame → inner-content left offset is
/// `DETAIL_CONTENT_BLOCK_BORDER_COLS + BODY_LEFT_CHROME_COLS` (total 3), matching
/// the offset used by `body_link_cmd_at`.
const DETAIL_CONTENT_BLOCK_BORDER_COLS: u16 = 1;

/// Number of scrollable body rows available in the current viewport.
///
/// Subtracts the chrome rows (top border + header bar + footer bar + bottom border)
/// from the full terminal height. Returns 0 when the viewport is smaller than the chrome.
pub(crate) fn content_height(viewport_rows: u16) -> u16 {
    viewport_rows.saturating_sub(DETAIL_CHROME_ROWS)
}

/// Number of scrollable body rows available, clamped to a minimum of 1.
///
/// Why the floor is 1: a zero-height body viewport makes the scroll and offset arithmetic
/// degenerate — `viewport_end = offset + height` and `lines_len.saturating_sub(height)`
/// both produce misleading values when height is 0. The floor keeps model-only tests at
/// viewport=(0,0) consistent with render behaviour, where at least one row is assumed.
pub(crate) fn content_height_clamped(viewport_rows: u16) -> usize {
    (content_height(viewport_rows) as usize).max(1)
}

/// Return true when `row` falls within the scrollable body text area.
///
/// The body spans `[DETAIL_TEXT_TOP, DETAIL_TEXT_TOP + content_height(viewport_rows))`.
pub(crate) fn is_in_content(viewport_rows: u16, row: u16) -> bool {
    row >= DETAIL_TEXT_TOP && row < DETAIL_TEXT_TOP + content_height(viewport_rows)
}

/// Map a viewport `row` to the logical content line index, accounting for the scroll `offset`.
///
/// Returns `None` when `row` is outside the scrollable body area. The caller is responsible
/// for the `lines.len()` guard — this module is viewport-only and does not know the line list.
pub(crate) fn row_to_line_idx(offset: usize, viewport_rows: u16, row: u16) -> Option<usize> {
    if !is_in_content(viewport_rows, row) {
        return None;
    }
    Some(offset + (row - DETAIL_TEXT_TOP) as usize)
}

/// An active text selection anchored at a body cell and extended by drag.
///
/// Coordinates are (viewport_row, viewport_col) — terminal cell positions
/// within the full frame. Anchor is set on mouse-down; cursor tracks drag.
/// Text is extracted when the button is released.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub anchor: (u16, u16),
    pub cursor: (u16, u16),
}

impl Selection {
    /// Whether the selection spans more than a single cell (i.e. a real drag).
    pub fn is_drag(&self) -> bool {
        self.anchor != self.cursor
    }

    /// Return (top_left, bottom_right) in reading order (row-major).
    pub fn normalized(&self) -> ((u16, u16), (u16, u16)) {
        let (ar, ac) = self.anchor;
        let (cr, cc) = self.cursor;
        if (ar, ac) <= (cr, cc) {
            ((ar, ac), (cr, cc))
        } else {
            ((cr, cc), (ar, ac))
        }
    }
}

/// Extract the text covered by `sel` from a Detail body's boxed `lines`.
///
/// Strips the box chrome (│ border + HPAD) from each logical line before slicing,
/// so the copied text is chrome-free and char-correct (Sc.8, Sc.9). Row→line
/// mapping delegates to `row_to_line_idx`. Returns text in reading order (anchor
/// normalized to be before cursor). Reads only its arguments — no terminal, time,
/// or async sources — so it is safe to call from the pure TEA update loop.
pub(crate) fn selected_text(
    offset: usize,
    viewport_rows: u16,
    sel: Selection,
    lines: &[String],
) -> String {
    let ((top_row, top_col), (bot_row, bot_col)) = sel.normalized();

    let mut parts: Vec<String> = Vec::new();
    for vp_row in top_row..=bot_row {
        let Some(line_idx) = row_to_line_idx(offset, viewport_rows, vp_row) else {
            continue;
        };
        let Some(line) = lines.get(line_idx) else {
            continue;
        };
        let chunk = line_slice(line, vp_row, top_row, bot_row, top_col, bot_col);
        if !chunk.is_empty() {
            parts.push(chunk);
        }
    }

    parts.join("\n")
}

/// Extract the relevant text slice from a single boxed body line.
///
/// Maps an absolute frame column from a `Selection` to an inner-content display column
/// by subtracting the full left offset: `DETAIL_CONTENT_BLOCK_BORDER_COLS` (ratatui
/// `Block::borders(ALL)` left edge) plus `BODY_LEFT_CHROME_COLS` (panel `│` + HPAD),
/// totalling 3 — the same offset used by `body_link_cmd_at` so highlight and copy
/// resolve the same column to the same content position.
///
/// Delegates to `render::slice_by_display_cols` which walks chars accumulating display
/// width, correctly handling double-width chars (emoji, CJK). Trailing padding spaces
/// added by `fit_to_display_width` are stripped before slicing.
fn line_slice(
    line: &str,
    vp_row: u16,
    top_row: u16,
    bot_row: u16,
    top_col: u16,
    bot_col: u16,
) -> String {
    let inner = crate::render::box_inner_content(line).unwrap_or("");
    let trimmed = inner.trim_end_matches(' ');
    let inner_display_width = crate::render::display_width(trimmed);

    let left_offset =
        DETAIL_CONTENT_BLOCK_BORDER_COLS as usize + crate::render::BODY_LEFT_CHROME_COLS;
    let start_inner_col = if vp_row == top_row {
        (top_col as usize).saturating_sub(left_offset)
    } else {
        0
    };
    let end_inner_col = if vp_row == bot_row {
        (bot_col as usize)
            .saturating_sub(left_offset)
            .saturating_add(1)
            .min(inner_display_width)
    } else {
        inner_display_width
    };

    crate::render::slice_by_display_cols(trimmed, start_inner_col, end_inner_col)
}

#[cfg(test)]
#[path = "../../tests/unit/detail_geometry.rs"]
mod detail_geometry_tests;
