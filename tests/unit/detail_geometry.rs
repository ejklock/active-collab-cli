use super::{
    content_height, content_height_clamped, is_in_content, row_to_line_idx, selected_text,
    Selection, DETAIL_CHROME_ROWS, DETAIL_TEXT_TOP,
};

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

// AC3: content_height_clamped — viewport smaller than the chrome yields 1 (the floor).
// Dropping the .max(1) floor causes this to return 0.
#[test]
fn content_height_clamped_below_chrome_returns_one() {
    let viewport_rows = DETAIL_CHROME_ROWS - 1;
    assert_eq!(
        content_height_clamped(viewport_rows),
        1,
        "viewport_rows={viewport_rows} (< DETAIL_CHROME_ROWS={DETAIL_CHROME_ROWS}) must clamp to 1",
    );
}

// AC3: content_height_clamped — viewport exactly equal to the chrome yields 1 (the floor).
// Dropping the .max(1) floor causes this to return 0 (content_height returns 0 at the boundary).
#[test]
fn content_height_clamped_at_chrome_boundary_returns_one() {
    let viewport_rows = DETAIL_CHROME_ROWS;
    assert_eq!(
        content_height_clamped(viewport_rows),
        1,
        "viewport_rows={viewport_rows} (== DETAIL_CHROME_ROWS={DETAIL_CHROME_ROWS}) must clamp to 1",
    );
}

// AC3: content_height_clamped — viewport with N body rows above the chrome yields N.
// Dropping the chrome subtraction causes this to return viewport_rows (too large).
#[test]
fn content_height_clamped_above_chrome_returns_body_rows() {
    let body_rows: u16 = 10;
    let viewport_rows = DETAIL_CHROME_ROWS + body_rows;
    assert_eq!(
        content_height_clamped(viewport_rows),
        body_rows as usize,
        "viewport_rows={viewport_rows} with {body_rows} body rows must return {body_rows}",
    );
}

// TE mutation floor: viewport=0 (degenerate) also hits the floor — ensures zero-height
// does not slip through even without saturating_sub.
#[test]
fn content_height_clamped_zero_viewport_returns_one() {
    assert_eq!(
        content_height_clamped(0),
        1,
        "viewport_rows=0 must clamp to 1 to prevent degenerate scroll arithmetic",
    );
}

// --- ADR 0050: selected_text — the column half absorbed from model.rs ---

// A boxed Detail body line: `│ {content} │`, matching the chrome
// `box_inner_content` strips (border + HPAD on each side).
fn boxed(content: &str) -> String {
    format!("\u{2502} {content} \u{2502}")
}

// Absolute-frame column offset for inner-content column 0: the ratatui outer
// block border (1 col) plus the panel's own `│ ` chrome (2 cols) — total 3,
// matching DETAIL_CONTENT_BLOCK_BORDER_COLS + BODY_LEFT_CHROME_COLS.
const LEFT_OFFSET: u16 = 3;

// AC5: single-row selection extracts the exact inner-content slice.
#[test]
fn selected_text_single_row_extracts_slice() {
    let lines = vec![boxed("hello world")];
    let sel = Selection {
        anchor: (2, LEFT_OFFSET),
        cursor: (2, LEFT_OFFSET + 4),
    };
    assert_eq!(
        selected_text(0, 24, sel, &lines),
        "hello",
        "selecting inner cols [0,5) of 'hello world' must yield 'hello'",
    );
}

// AC5: multi-row selection joins each row's slice with '\n', trimming to the
// selection start column on the first row and the end column on the last row.
#[test]
fn selected_text_multi_row_joins_with_newline() {
    let lines = vec![boxed("hello world"), boxed("second line")];
    let sel = Selection {
        anchor: (2, LEFT_OFFSET + 6),
        cursor: (3, LEFT_OFFSET + 5),
    };
    assert_eq!(
        selected_text(0, 24, sel, &lines),
        "world\nsecond",
        "row0 tail from col6 is 'world'; row1 head through col5 is 'second'",
    );
}

// AC5: partial-column selection starting and ending mid-line (not at a line edge).
#[test]
fn selected_text_partial_column_mid_line() {
    let lines = vec![boxed("hello world")];
    let sel = Selection {
        anchor: (2, LEFT_OFFSET + 3),
        cursor: (2, LEFT_OFFSET + 7),
    };
    assert_eq!(
        selected_text(0, 24, sel, &lines),
        "lo wo",
        "selecting inner cols [3,8) of 'hello world' must yield 'lo wo'",
    );
}

// AC5: double-width (emoji) selection never splits a glyph — the window boundary
// falls exactly on the glyph edge so only the first emoji is included.
#[test]
fn selected_text_double_width_glyph_not_split() {
    let lines = vec![boxed("\u{1F600}\u{1F600}")];
    let sel = Selection {
        anchor: (2, LEFT_OFFSET),
        cursor: (2, LEFT_OFFSET + 1),
    };
    assert_eq!(
        selected_text(0, 24, sel, &lines),
        "\u{1F600}",
        "inner cols [0,2) must select exactly the first (2-col-wide) emoji",
    );
}

// AC5: a scroll offset shifts which logical line a viewport row maps to, but the
// extracted text is unaffected by the shift itself — proves selected_text composes
// row_to_line_idx rather than assuming offset=0.
#[test]
fn selected_text_applies_scroll_offset_to_row_mapping() {
    let lines = vec![boxed("first"), boxed("second"), boxed("third")];
    let sel = Selection {
        anchor: (2, LEFT_OFFSET),
        cursor: (2, LEFT_OFFSET + 5),
    };
    assert_eq!(
        selected_text(1, 24, sel, &lines),
        "second",
        "offset=1 shifts viewport row 2 to line_idx 1 ('second'), not line_idx 0",
    );
}

// AC5: a selection row outside the scrollable body area contributes nothing —
// row_to_line_idx returns None and the row is skipped rather than panicking.
#[test]
fn selected_text_row_outside_content_is_skipped() {
    let lines = vec![boxed("only line")];
    let sel = Selection {
        anchor: (0, LEFT_OFFSET),
        cursor: (2, LEFT_OFFSET + 3),
    };
    assert_eq!(
        selected_text(0, 24, sel, &lines),
        "only",
        "row 0 and 1 are above DETAIL_TEXT_TOP and contribute nothing; only row 2 does",
    );
}
