/// The first viewport row occupied by scrollable body text in the Detail screen.
///
/// Why 2: row 0 is the outer block top border, row 1 is the header bar row.
/// Rows 0..DETAIL_TEXT_TOP are chrome that is never part of the scrollable body.
pub(crate) const DETAIL_TEXT_TOP: u16 = 2;

/// Number of scrollable body rows available in the current viewport.
///
/// Subtracts the chrome rows (top border + header bar + footer bar + bottom border)
/// from the full terminal height. Returns 0 when the viewport is smaller than the chrome.
pub(crate) fn content_height(viewport_rows: u16) -> u16 {
    viewport_rows.saturating_sub(crate::tui::model::DETAIL_CHROME_ROWS)
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

#[cfg(test)]
#[path = "../../tests/unit/detail_geometry.rs"]
mod detail_geometry_tests;
