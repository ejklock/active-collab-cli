//! Pure display-width / box-geometry primitives shared across the crate.
//!
//! This module owns the terminal display-column math (`display_width`,
//! `slice_by_display_cols`, `fit_to_display_width`, `truncate_to_display_width`),
//! the boxed-line accessor (`box_inner_content`), the panel chrome constants
//! (`PANEL_HPAD`, `BODY_LEFT_CHROME_COLS`, `panel_content_width`), and the
//! box-drawing glyphs. It intentionally has no styled-text-model dependency
//! (ADR 0049) so `wrap`/`cli_render`/`detail_render` — and later
//! `detail_geometry` and `task_layout` — can borrow width math without
//! dragging that heavier dependency in.

use unicode_width::UnicodeWidthStr;

/// Display columns consumed by the panel-box left chrome: 1 space of horizontal
/// padding on each side of a panel's content.
pub(crate) const PANEL_HPAD: usize = 1;

/// Box-drawing glyphs used by squared legend panels and cards (ADR 0049 single
/// home, ADR 0061 squared corners).
pub(crate) const BOX_TL: &str = "\u{250C}";
pub(crate) const BOX_TR: &str = "\u{2510}";
pub(crate) const BOX_BL: &str = "\u{2514}";
pub(crate) const BOX_BR: &str = "\u{2518}";
pub(crate) const BOX_H: &str = "\u{2500}";
pub(crate) const BOX_V: &str = "\u{2502}";

/// Strip the panel-box left border and HPAD from a boxed content line.
///
/// A box content line has the form `│ {content} │` where `│` is U+2502 (3 bytes)
/// and the surrounding space is PANEL_HPAD (1 byte each side). Returns the inner
/// content slice `{content}` (including any trailing-space padding added by
/// `fit_to_display_width`), or `None` when the string is not a box content line
/// (e.g. a border row starting with `┌` or `└`).
///
/// This is the single public interface for this primitive — the `_pub`
/// pass-through wrapper that used to live in `render.rs` is retired (ADR 0049).
pub fn box_inner_content(s: &str) -> Option<&str> {
    const PREFIX_BYTES: usize = 4; // U+2502 = 3 UTF-8 bytes, then one HPAD space
    const SUFFIX_BYTES: usize = 4; // one HPAD space, then U+2502 = 3 UTF-8 bytes
    if !s.starts_with('\u{2502}') {
        return None;
    }
    let len = s.len();
    if len < PREFIX_BYTES + SUFFIX_BYTES {
        return None;
    }
    Some(&s[PREFIX_BYTES..len - SUFFIX_BYTES])
}

/// Display columns consumed by the panel-box left chrome: the `│` border (1 col) plus
/// `PANEL_HPAD` (1 space), giving 2 total.
///
/// This is the panel-chrome term only. The full absolute-frame → inner-content left
/// offset also includes the ratatui `Block::borders(ALL)` left border (1 col) drawn by
/// `render_content`; that additional column is `DETAIL_CONTENT_BLOCK_BORDER_COLS` in
/// `detail_geometry.rs`. Add both terms when converting an absolute frame column to an
/// inner-content column.
pub const BODY_LEFT_CHROME_COLS: usize = 1 + PANEL_HPAD;

pub(crate) fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Return the substring of `s` that occupies the half-open DISPLAY-column window
/// `[start_col, end_col)`.
///
/// Double-width chars (emoji, CJK) are never split: a char whose first display
/// column falls within the window is included in full. `end_col` is clamped to
/// `display_width(s)` so a window past the end simply returns the tail of the
/// string. An empty window (`start_col >= end_col`) returns an empty string.
///
/// This is the single source of truth for mapping display columns to char
/// boundaries. It must be used wherever a display-column range is converted to a
/// text slice — never treat a display column as a char index.
pub fn slice_by_display_cols(s: &str, start_col: usize, end_col: usize) -> String {
    if start_col >= end_col {
        return String::new();
    }
    let end_col = end_col.min(display_width(s));
    if start_col >= end_col {
        return String::new();
    }
    let mut result = String::new();
    let mut acc = 0usize;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc >= end_col {
            break;
        }
        if acc >= start_col {
            result.push(ch);
        }
        acc += cw;
    }
    result
}

pub(crate) fn fit_to_display_width(s: &str, cols: usize) -> String {
    let w = display_width(s);
    if w <= cols {
        let padding = cols - w;
        let mut out = s.to_string();
        for _ in 0..padding {
            out.push(' ');
        }
        return out;
    }
    let mut acc = 0usize;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > cols {
            break;
        }
        result.push(ch);
        acc += cw;
    }
    let padding = cols - acc;
    for _ in 0..padding {
        result.push(' ');
    }
    result
}

/// Truncate `s` to at most `width` display columns.
///
/// Unlike `fit_to_display_width`, this never pads the result — callers that
/// need a fixed-width cell compose this with their own padding.
pub(crate) fn truncate_to_display_width(s: &str, width: usize) -> String {
    let mut acc = 0usize;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > width {
            break;
        }
        result.push(ch);
        acc += cw;
    }
    result
}

pub(crate) fn panel_content_width(width: usize) -> usize {
    width.saturating_sub(2 + 2 * PANEL_HPAD)
}

#[cfg(test)]
#[path = "../../tests/unit/text_measure.rs"]
mod tests;
