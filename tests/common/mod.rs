//! A small Rust lexer shared by the source-reading policy tests: it splits a file
//! into code, comments and literals over the whole text, so multi-line strings,
//! nested block comments, raw strings and lifetime ticks are read correctly.

use std::ops::Range;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SpanKind {
    Code,
    /// A `//` comment that is not a doc comment.
    Line,
    /// A `///` or `//!` comment.
    Doc,
    /// A `/* */` comment, nested ones included.
    Block,
    /// A string, raw string or char literal.
    Literal,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub kind: SpanKind,
    pub range: Range<usize>,
}

/// Split `text` into consecutive spans that cover every byte.
pub fn lex(text: &str) -> Vec<Span> {
    let b = text.as_bytes();
    let mut spans = Vec::new();
    let mut code_start = 0;
    let mut i = 0;
    while i < b.len() {
        let Some((kind, end)) = token_at(b, i) else {
            i += 1;
            continue;
        };
        if code_start < i {
            spans.push(Span {
                kind: SpanKind::Code,
                range: code_start..i,
            });
        }
        spans.push(Span {
            kind,
            range: i..end,
        });
        i = end;
        code_start = end;
    }
    if code_start < b.len() {
        spans.push(Span {
            kind: SpanKind::Code,
            range: code_start..b.len(),
        });
    }
    spans
}

fn token_at(b: &[u8], i: usize) -> Option<(SpanKind, usize)> {
    if b[i..].starts_with(b"//") {
        let doc = b[i..].starts_with(b"///") || b[i..].starts_with(b"//!");
        let kind = if doc { SpanKind::Doc } else { SpanKind::Line };
        return Some((kind, line_end(b, i)));
    }
    if b[i..].starts_with(b"/*") {
        return Some((SpanKind::Block, block_comment_end(b, i)));
    }
    let literal_end = match b[i] {
        b'"' => Some(quoted_end(b, i + 1, b'"')),
        b'\'' => char_literal_end(b, i),
        b'r' => raw_string_end(b, i),
        _ => None,
    };
    literal_end.map(|end| (SpanKind::Literal, end))
}

fn line_end(b: &[u8], from: usize) -> usize {
    b[from..]
        .iter()
        .position(|&c| c == b'\n')
        .map_or(b.len(), |p| from + p)
}

fn block_comment_end(b: &[u8], from: usize) -> usize {
    let mut depth = 0;
    let mut i = from;
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            depth += 1;
            i += 2;
        } else if b[i..].starts_with(b"*/") {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    b.len()
}

fn quoted_end(b: &[u8], from: usize, quote: u8) -> usize {
    let mut i = from;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
        } else if b[i] == quote {
            return i + 1;
        } else {
            i += 1;
        }
    }
    b.len()
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// True when the `r` at `from` starts a raw string prefix: alone, or after a lone `b`
/// or `c` that is not itself the tail of an identifier.
fn raw_prefix_starts_at(b: &[u8], from: usize) -> bool {
    match from.checked_sub(1).map(|p| b[p]) {
        None => true,
        Some(prev) if !is_ident_byte(prev) => true,
        Some(b'b' | b'c') => from < 2 || !is_ident_byte(b[from - 2]),
        Some(_) => false,
    }
}

/// End of a raw string such as `r#"..."#` starting at `from`, or `None` when `from`
/// does not open one.
fn raw_string_end(b: &[u8], from: usize) -> Option<usize> {
    if !raw_prefix_starts_at(b, from) {
        return None;
    }
    let hashes = b[from + 1..].iter().take_while(|&&c| c == b'#').count();
    if b.get(from + 1 + hashes) != Some(&b'"') {
        return None;
    }
    let mut closing = vec![b'"'];
    closing.extend(std::iter::repeat_n(b'#', hashes));
    let body = from + 2 + hashes;
    let found = b[body..]
        .windows(closing.len())
        .position(|w| w == closing.as_slice());
    Some(found.map_or(b.len(), |p| body + p + closing.len()))
}

/// A `'` opens a char literal only when it closes right after one character or an
/// escape; otherwise it marks a lifetime, which is code.
fn char_literal_end(b: &[u8], from: usize) -> Option<usize> {
    if b.get(from + 1) == Some(&b'\\') {
        return Some(quoted_end(b, from + 1, b'\''));
    }
    let width = std::str::from_utf8(&b[from + 1..])
        .ok()
        .and_then(|s| s.chars().next())
        .map_or(1, char::len_utf8);
    (b.get(from + 1 + width) == Some(&b'\'')).then_some(from + 2 + width)
}
