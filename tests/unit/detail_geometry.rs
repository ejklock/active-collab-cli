use super::{content_height, is_in_content, row_to_line_idx, DETAIL_TEXT_TOP};

// AC4: is_in_content — row just below DETAIL_TEXT_TOP is out of the body area.
#[test]
fn is_in_content_row_below_text_top_is_false() {
    let viewport_rows = 24;
    assert!(
        !is_in_content(viewport_rows, DETAIL_TEXT_TOP - 1),
        "row {} (one below DETAIL_TEXT_TOP={}) must be outside the body area",
        DETAIL_TEXT_TOP - 1,
        DETAIL_TEXT_TOP,
    );
}

// AC4: is_in_content — row exactly at DETAIL_TEXT_TOP is the first valid body row.
#[test]
fn is_in_content_at_text_top_is_true() {
    let viewport_rows = 24;
    assert!(
        is_in_content(viewport_rows, DETAIL_TEXT_TOP),
        "row {} (DETAIL_TEXT_TOP) must be inside the body area for viewport_rows={}",
        DETAIL_TEXT_TOP,
        viewport_rows,
    );
}

// AC4: is_in_content — last in-content row is DETAIL_TEXT_TOP + content_height - 1.
#[test]
fn is_in_content_last_row_is_true() {
    let viewport_rows = 24u16;
    let last_row = DETAIL_TEXT_TOP + content_height(viewport_rows) - 1;
    assert!(
        is_in_content(viewport_rows, last_row),
        "row {} (last content row) must be inside the body area for viewport_rows={}",
        last_row,
        viewport_rows,
    );
}

// AC4: is_in_content — first row past content (DETAIL_TEXT_TOP + content_height) is out.
#[test]
fn is_in_content_first_row_past_content_is_false() {
    let viewport_rows = 24u16;
    let past_row = DETAIL_TEXT_TOP + content_height(viewport_rows);
    assert!(
        !is_in_content(viewport_rows, past_row),
        "row {} (first past content) must be outside the body area for viewport_rows={}",
        past_row,
        viewport_rows,
    );
}

// AC4: row_to_line_idx — in-range row with a non-zero offset produces the correct line index.
// offset=5, viewport_rows=24, row=4 (DETAIL_TEXT_TOP=2) → line_idx = 5 + (4 - 2) = 7.
#[test]
fn row_to_line_idx_in_range_applies_offset_shift() {
    let offset = 5usize;
    let viewport_rows = 24u16;
    let row = 4u16;
    let expected = offset + (row - DETAIL_TEXT_TOP) as usize;
    assert_eq!(
        row_to_line_idx(offset, viewport_rows, row),
        Some(expected),
        "offset={offset}, row={row} must map to line_idx={expected}",
    );
}

// AC4: row_to_line_idx — out-of-range row returns None.
#[test]
fn row_to_line_idx_out_of_range_row_returns_none() {
    let offset = 0usize;
    let viewport_rows = 24u16;
    assert_eq!(
        row_to_line_idx(offset, viewport_rows, 0),
        None,
        "row 0 is above DETAIL_TEXT_TOP and must return None",
    );
}

// AC4: row_to_line_idx — row at DETAIL_TEXT_TOP with zero offset maps to line 0.
#[test]
fn row_to_line_idx_at_text_top_zero_offset_returns_zero() {
    assert_eq!(
        row_to_line_idx(0, 24, DETAIL_TEXT_TOP),
        Some(0),
        "row=DETAIL_TEXT_TOP, offset=0 must map to line_idx=0",
    );
}

// TE mutation floor: changing DETAIL_TEXT_TOP from 2 to any other value must break
// at least one of the boundary tests above. This test locks the constant's value.
#[test]
fn detail_text_top_is_two() {
    assert_eq!(
        DETAIL_TEXT_TOP, 2,
        "DETAIL_TEXT_TOP must be 2 (top border + header bar rows above the scrollable body)",
    );
}
