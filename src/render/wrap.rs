//! The greedy word-wrap engine shared by plain and rich text (ADR 0048).
//!
//! Owns `greedy_wrap` and the `WrapCell`/`WrapLine` traits that let one core
//! algorithm operate over both a plain `char` stream and a styled
//! `(char, RichStyle)` stream, plus the two public adapters `wrap_text` and
//! `wrap_rich`. Depends on `text_measure`-equivalent width math (via
//! `unicode_width` directly, unchanged from before the split) and on
//! `richtext` for the rich adapter's cell/line types (ADR 0049).

/// A single wrap-stream cell — a `char` for plain text, `(char, RichStyle)` for rich
/// text. `greedy_wrap` only needs to measure and classify a cell, never its concrete type.
trait WrapCell {
    fn display_width(&self) -> usize;
    /// Ascii whitespace that is NOT a newline — a word boundary within a segment.
    fn is_word_separator(&self) -> bool;
    fn is_newline(&self) -> bool;
}

impl WrapCell for char {
    fn display_width(&self) -> usize {
        unicode_width::UnicodeWidthChar::width(*self).unwrap_or(0)
    }

    fn is_word_separator(&self) -> bool {
        self.is_ascii_whitespace() && *self != '\n'
    }

    fn is_newline(&self) -> bool {
        *self == '\n'
    }
}

impl WrapCell for (char, crate::richtext::RichStyle) {
    fn display_width(&self) -> usize {
        self.0.display_width()
    }

    fn is_word_separator(&self) -> bool {
        self.0.is_word_separator()
    }

    fn is_newline(&self) -> bool {
        self.0.is_newline()
    }
}

/// A line accumulated by `greedy_wrap` — `String` for plain text, `RichLine` for rich
/// text. The core owns the running display-width count; the line only knows how to append.
trait WrapLine: Default {
    type Cell;
    fn push_cell(&mut self, cell: &Self::Cell);
    fn push_separator(&mut self);
    fn is_empty(&self) -> bool;
}

impl WrapLine for String {
    type Cell = char;

    fn push_cell(&mut self, cell: &char) {
        self.push(*cell);
    }

    fn push_separator(&mut self) {
        self.push(' ');
    }

    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }
}

impl WrapLine for crate::richtext::RichLine {
    type Cell = (char, crate::richtext::RichStyle);

    fn push_cell(&mut self, cell: &Self::Cell) {
        let mut buf = [0u8; 4];
        let s = cell.0.encode_utf8(&mut buf);
        push_rich_span(self, s, cell.1);
    }

    fn push_separator(&mut self) {
        push_rich_span(self, " ", crate::richtext::RichStyle::Plain);
    }

    fn is_empty(&self) -> bool {
        <[_]>::is_empty(self)
    }
}

/// The one greedy word-wrap algorithm shared by `wrap_text` and `wrap_rich` (ADR 0048).
///
/// `cells` is split on newline cells into segments; every segment yields at least one
/// output line, so an empty segment (a blank line between paragraphs) produces one empty
/// line instead of being dropped — the canonical contract that fixes the prior rich-only
/// blank-line drop. Within a segment, words (maximal runs of non-separator cells) are
/// greedily placed on the current line, joined by a single separator when they fit within
/// `width`; a word wider than `width` on its own is hard-split by accumulated display width.
fn greedy_wrap<C: WrapCell, L: WrapLine<Cell = C>>(cells: &[C], width: usize) -> Vec<L> {
    let mut result = Vec::new();
    for segment in cells.split(|c| c.is_newline()) {
        result.extend(wrap_segment(segment, width));
    }
    result
}

/// Maximal runs of non-separator cells within a segment — the words to place.
fn words<C: WrapCell>(segment: &[C]) -> impl Iterator<Item = &[C]> {
    segment
        .split(|c| c.is_word_separator())
        .filter(|word| !word.is_empty())
}

/// Greedy-wrap one newline-delimited segment into its own output line(s).
///
/// A wordless segment (empty, or whitespace only) still yields exactly one empty line.
fn wrap_segment<C: WrapCell, L: WrapLine<Cell = C>>(segment: &[C], width: usize) -> Vec<L> {
    let mut result = Vec::new();
    let mut current = L::default();
    let mut current_dw = 0usize;

    for word in words(segment) {
        place_word(word, width, &mut current, &mut current_dw, &mut result);
    }

    if !current.is_empty() || result.is_empty() {
        result.push(current);
    }
    result
}

/// Place one word on the current line, joined by a separator when it fits within `width`;
/// otherwise flush the current line and start a new one with this word.
fn place_word<C: WrapCell, L: WrapLine<Cell = C>>(
    word: &[C],
    width: usize,
    current: &mut L,
    current_dw: &mut usize,
    result: &mut Vec<L>,
) {
    let word_dw: usize = word.iter().map(WrapCell::display_width).sum();

    if *current_dw > 0 {
        if *current_dw + 1 + word_dw <= width {
            current.push_separator();
            for cell in word {
                current.push_cell(cell);
            }
            *current_dw += 1 + word_dw;
            return;
        }
        result.push(std::mem::take(current));
        *current_dw = 0;
    }

    append_word(word, word_dw, width, current, current_dw, result);
}

/// Append a word to the (empty) current line, or hard-split it if wider than `width`.
fn append_word<C: WrapCell, L: WrapLine<Cell = C>>(
    word: &[C],
    word_dw: usize,
    width: usize,
    current: &mut L,
    current_dw: &mut usize,
    result: &mut Vec<L>,
) {
    if word_dw <= width {
        for cell in word {
            current.push_cell(cell);
        }
        *current_dw = word_dw;
    } else {
        hard_split(word, width, current, current_dw, result);
    }
}

/// Hard-split a word wider than `width` columns, flushing full chunks as separate lines.
fn hard_split<C: WrapCell, L: WrapLine<Cell = C>>(
    word: &[C],
    width: usize,
    current: &mut L,
    current_dw: &mut usize,
    result: &mut Vec<L>,
) {
    let mut acc = 0usize;
    for cell in word {
        let cw = cell.display_width();
        if acc + cw > width {
            result.push(std::mem::take(current));
            acc = 0;
        }
        current.push_cell(cell);
        acc += cw;
    }
    *current_dw = acc;
}

/// Parity: Python tui.py wrap_text.
///
/// Greedy word-wrap on ascii whitespace to at most `width` DISPLAY columns per line.
/// Display width is measured via unicode-width (same crate ratatui uses), so CJK and
/// combining characters are handled correctly. A single word wider than `width` is
/// hard-split by accumulated display width. Existing line breaks are preserved, including
/// blank lines between them (ADR 0048). Empty input yields an empty Vec.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return vec![];
    }
    let cells: Vec<char> = text.chars().collect();
    greedy_wrap::<char, String>(&cells, width)
}

/// Greedy word-wrap for a `RichLine`, preserving span emphasis across breaks.
///
/// Shares `wrap_text`'s greedy display-width core (ADR 0048) over `(char, RichStyle)`
/// cells, so a span whose style is Bold/Italic/Code carries that style on every wrapped
/// fragment, and blank lines between paragraphs are preserved rather than dropped. Style
/// is threaded per-character so repeated words with different emphasis and substring words
/// each keep their own style (ADR 0030, BDR 0023). Empty input yields an empty Vec.
pub fn wrap_rich(line: &crate::richtext::RichLine, width: usize) -> Vec<crate::richtext::RichLine> {
    if line.is_empty() || width == 0 {
        return vec![];
    }
    let cells = expand_to_styled_chars(line);
    greedy_wrap::<_, crate::richtext::RichLine>(&cells, width)
}

/// Expand a `RichLine` to an ordered sequence of `(char, RichStyle)` pairs.
///
/// This is the single place where per-character origin is preserved: each span
/// contributes its characters tagged with that span's style.  The wrap operates
/// on this stream instead of re-deriving style by substring matching.
fn expand_to_styled_chars(
    line: &crate::richtext::RichLine,
) -> Vec<(char, crate::richtext::RichStyle)> {
    line.iter()
        .flat_map(|span| span.text.chars().map(move |ch| (ch, span.style)))
        .collect()
}

/// Push a text fragment onto a `RichLine`, merging adjacent same-style spans.
fn push_rich_span(
    line: &mut crate::richtext::RichLine,
    text: &str,
    style: crate::richtext::RichStyle,
) {
    use crate::richtext::RichSpan;
    if text.is_empty() {
        return;
    }
    if let Some(last) = line.last_mut() {
        if last.style == style {
            last.text.push_str(text);
            return;
        }
    }
    line.push(RichSpan {
        text: text.to_string(),
        style,
    });
}

#[cfg(test)]
#[path = "../../tests/unit/wrap.rs"]
mod tests;
