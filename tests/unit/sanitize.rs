use super::*;

#[test]
fn strip_control_chars_drops_esc() {
    let input = "before\x1b[2Jafter";
    let out = strip_control_chars(input);
    assert!(!out.contains('\u{001b}'), "ESC survived: {out:?}");
    assert_eq!(out, "before[2Jafter");
}

#[test]
fn strip_control_chars_drops_bel() {
    let input = "ding\x07dong";
    let out = strip_control_chars(input);
    assert!(!out.contains('\u{0007}'), "BEL survived: {out:?}");
    assert_eq!(out, "dingdong");
}

#[test]
fn strip_control_chars_drops_del() {
    let input = "a\x7fb";
    let out = strip_control_chars(input);
    assert!(!out.contains('\u{007f}'), "DEL survived: {out:?}");
    assert_eq!(out, "ab");
}

#[test]
fn strip_control_chars_drops_c1_byte() {
    let input = "a\u{009b}b";
    let out = strip_control_chars(input);
    assert!(!out.contains('\u{009b}'), "C1 byte survived: {out:?}");
    assert_eq!(out, "ab");
}

#[test]
fn strip_control_chars_preserves_newline_and_tab() {
    let input = "line1\nline2\tindented";
    let out = strip_control_chars(input);
    assert_eq!(out, input);
}

#[test]
fn strip_control_chars_preserves_multibyte_utf8() {
    let input = "Ação café naïve 🎉 emoji";
    let out = strip_control_chars(input);
    assert_eq!(out, input, "multi-byte UTF-8 must survive byte-identical");
}

#[test]
fn strip_control_chars_leaves_clean_string_unchanged() {
    let input = "already clean, nothing to strip here.";
    let out = strip_control_chars(input);
    assert_eq!(out, input);
}

#[test]
fn strip_control_chars_is_idempotent() {
    let input = "a\x1bb\x07c\nd\te\u{009b}f";
    let once = strip_control_chars(input);
    let twice = strip_control_chars(&once);
    assert_eq!(once, twice);
}

#[test]
fn strip_control_chars_empty_string_returns_empty() {
    assert_eq!(strip_control_chars(""), "");
}
