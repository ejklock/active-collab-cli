use super::*;

// --- display_width ---

#[test]
fn display_width_ascii_counts_one_col_per_char() {
    assert_eq!(display_width("hello"), 5);
}

#[test]
fn display_width_empty_string_is_zero() {
    assert_eq!(display_width(""), 0);
}

#[test]
fn display_width_cjk_chars_count_two_cols_each() {
    assert_eq!(display_width("你好"), 4);
}

#[test]
fn display_width_emoji_counts_two_cols() {
    assert_eq!(display_width("😀"), 2);
}

#[test]
fn display_width_mixed_ascii_and_cjk() {
    assert_eq!(display_width("ab你好"), 6);
}

// --- slice_by_display_cols ---

#[test]
fn slice_by_display_cols_ascii_window_returns_expected_substring() {
    assert_eq!(slice_by_display_cols("hello world", 0, 5), "hello");
    assert_eq!(slice_by_display_cols("hello world", 6, 11), "world");
}

#[test]
fn slice_by_display_cols_empty_window_returns_empty_string() {
    assert_eq!(slice_by_display_cols("hello", 3, 3), "");
    assert_eq!(slice_by_display_cols("hello", 5, 2), "");
}

#[test]
fn slice_by_display_cols_end_past_string_clamps_to_tail() {
    assert_eq!(slice_by_display_cols("hi", 0, 100), "hi");
}

#[test]
fn slice_by_display_cols_never_splits_a_double_width_char() {
    // "你" occupies display cols [0,2). A window ending mid-char (col 1) must not
    // include a half-char: the char's first col (0) is inside [0,1), so it is
    // included whole.
    assert_eq!(slice_by_display_cols("你a", 0, 1), "你");
    // A window starting mid-char (col 1) must skip the double-width char entirely
    // rather than including a fragment of it.
    assert_eq!(slice_by_display_cols("你a", 1, 3), "a");
}

#[test]
fn slice_by_display_cols_start_at_or_past_end_of_string_returns_empty() {
    assert_eq!(slice_by_display_cols("hi", 2, 5), "");
}

// --- box_inner_content ---

#[test]
fn box_inner_content_boxed_line_strips_border_and_hpad() {
    let line = "\u{2502} content \u{2502}";
    assert_eq!(box_inner_content(line), Some("content"));
}

#[test]
fn box_inner_content_preserves_trailing_padding_beyond_single_hpad_space() {
    let line = "\u{2502} content   \u{2502}";
    assert_eq!(box_inner_content(line), Some("content  "));
}

#[test]
fn box_inner_content_non_boxed_line_returns_none() {
    assert_eq!(box_inner_content("plain text"), None);
}

#[test]
fn box_inner_content_corner_border_row_returns_none() {
    let row = format!("{BOX_TL}{}{BOX_TR}", BOX_H.repeat(2));
    assert_eq!(box_inner_content(&row), None);
}

#[test]
fn box_inner_content_too_short_to_contain_chrome_returns_none() {
    assert_eq!(box_inner_content("\u{2502}"), None);
}

#[test]
fn box_inner_content_empty_string_returns_none() {
    assert_eq!(box_inner_content(""), None);
}

// --- fit_to_display_width ---

#[test]
fn fit_to_display_width_pads_short_strings() {
    assert_eq!(fit_to_display_width("hi", 5), "hi   ");
}

#[test]
fn fit_to_display_width_truncates_long_strings_and_does_not_pad() {
    assert_eq!(fit_to_display_width("hello world", 5), "hello");
}

#[test]
fn fit_to_display_width_exact_width_is_unchanged() {
    assert_eq!(fit_to_display_width("hello", 5), "hello");
}

// --- truncate_to_display_width ---

#[test]
fn truncate_to_display_width_never_pads_short_strings() {
    assert_eq!(truncate_to_display_width("hi", 5), "hi");
}

#[test]
fn truncate_to_display_width_clips_long_strings() {
    assert_eq!(truncate_to_display_width("hello world", 5), "hello");
}

#[test]
fn truncate_to_display_width_zero_width_returns_empty() {
    assert_eq!(truncate_to_display_width("hello", 0), "");
}

// --- panel_content_width ---

#[test]
fn panel_content_width_subtracts_border_and_hpad() {
    // 2 border cols + 2 * PANEL_HPAD (1 each side) = 4 cols of chrome.
    assert_eq!(panel_content_width(10), 6);
}

#[test]
fn panel_content_width_saturates_at_zero_for_narrow_widths() {
    assert_eq!(panel_content_width(1), 0);
}
