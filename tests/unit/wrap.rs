use super::*;
use unicode_width::UnicodeWidthStr;

#[test]
fn wrap_text_empty_input_returns_empty_vec() {
    let result = wrap_text("", 40);
    assert!(
        result.is_empty(),
        "empty input must yield empty vec: {:?}",
        result
    );
}

#[test]
fn wrap_text_single_short_word_fits_on_one_line() {
    let result = wrap_text("hello", 10);
    assert_eq!(result, vec!["hello"]);
}

#[test]
fn wrap_text_greedy_wrap_at_width() {
    let result = wrap_text("one two three four", 9);
    assert_eq!(
        result,
        vec!["one two", "three", "four"],
        "got: {:?}",
        result
    );
}

#[test]
fn wrap_text_hard_splits_word_longer_than_width() {
    let result = wrap_text("abcdefghij", 4);
    assert_eq!(result, vec!["abcd", "efgh", "ij"], "got: {:?}", result);
}

#[test]
fn wrap_text_preserves_embedded_newlines_as_independent_wraps() {
    let result = wrap_text("foo bar\nbaz qux", 20);
    assert_eq!(result, vec!["foo bar", "baz qux"], "got: {:?}", result);
}

/// Blank line between paragraphs preserved (ADR 0048's canonical wrap contract) — a
/// mutant that drops the wordless-segment fallback would collapse the empty middle
/// segment, producing 2 lines instead of 3.
#[test]
fn wrap_text_blank_line_between_paragraphs_preserved() {
    let result = wrap_text("a\n\nb", 10);
    assert_eq!(
        result,
        vec!["a".to_string(), "".to_string(), "b".to_string()],
        "got: {:?}",
        result
    );
}

#[test]
fn wrap_text_embedded_newline_line_that_wraps() {
    let result = wrap_text("short\na very long line here", 10);
    assert_eq!(
        result,
        vec!["short", "a very", "long line", "here"],
        "got: {:?}",
        result
    );
}

#[test]
fn wrap_text_multibyte_chars_counted_by_char_not_byte() {
    let s = "Ação muito longa para caber";
    let result = wrap_text(s, 10);
    for line in &result {
        assert!(
            line.chars().count() <= 10,
            "line exceeds width: {:?} (len={})",
            line,
            line.chars().count()
        );
    }
}

// --- D1-A3: wrap_text wraps by DISPLAY width ---

#[test]
fn wrap_text_cjk_no_line_exceeds_display_width() {
    let text = "日本語テスト文字列が長すぎる場合のラップ動作を確認する";
    let target = 10usize;
    let lines = wrap_text(text, target);
    assert!(!lines.is_empty(), "must produce wrapped lines for CJK text");
    for line in &lines {
        let dw = UnicodeWidthStr::width(line.as_str());
        assert!(
            dw <= target,
            "CJK wrapped line display_width={dw} exceeds target={target}: {line:?}"
        );
    }
}

#[test]
fn wrap_text_wide_single_word_hard_split_by_display_width() {
    let word = "日本語テスト文字列";
    let target = 6usize;
    let lines = wrap_text(word, target);
    assert!(!lines.is_empty(), "must hard-split over-wide CJK word");
    for line in &lines {
        let dw = UnicodeWidthStr::width(line.as_str());
        assert!(
            dw <= target,
            "hard-split line display_width={dw} exceeds target={target}: {line:?}"
        );
    }
}

#[test]
fn wrap_text_decomposed_accent_no_line_exceeds_display_width() {
    let text = "Março Otimização Renovação Programação";
    let target = 12usize;
    let lines = wrap_text(text, target);
    for line in &lines {
        let dw = UnicodeWidthStr::width(line.as_str());
        assert!(
            dw <= target,
            "accent line display_width={dw} exceeds target={target}: {line:?}"
        );
    }
}
