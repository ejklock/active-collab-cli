use std::path::{Path, PathBuf};

/// Replace comments and string or char literals with spaces, keeping every newline,
/// so a pattern search sees only real code and reports the right line numbers.
fn blank_non_code(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        let end = if b[i..].starts_with(b"//") {
            line_end(b, i)
        } else if b[i..].starts_with(b"/*") {
            block_comment_end(b, i)
        } else if b[i] == b'"' {
            quoted_end(b, i + 1, b'"')
        } else if let Some(end) = raw_string_end(b, i) {
            end
        } else if b[i] == b'\'' {
            char_literal_end(b, i)
        } else {
            i += 1;
            continue;
        };
        for byte in &mut out[i..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        i = end;
    }
    String::from_utf8_lossy(&out).into_owned()
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

/// End of a raw string such as `r#"..."#` starting at `from`, or `None` when `from`
/// does not open one.
fn raw_string_end(b: &[u8], from: usize) -> Option<usize> {
    let prev_is_ident = from > 0 && (b[from - 1].is_ascii_alphanumeric() || b[from - 1] == b'_');
    if b[from] != b'r' || prev_is_ident {
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
/// escape; otherwise it marks a lifetime and only the quote itself is skipped.
fn char_literal_end(b: &[u8], from: usize) -> usize {
    if b.get(from + 1) == Some(&b'\\') {
        return quoted_end(b, from + 1, b'\'');
    }
    let width = std::str::from_utf8(&b[from + 1..])
        .ok()
        .and_then(|s| s.chars().next())
        .map_or(1, char::len_utf8);
    if b.get(from + 1 + width) == Some(&b'\'') {
        from + 2 + width
    } else {
        from + 1
    }
}

/// Lines where `t(` (also `i18n::t(` and `crate::i18n::t(`) takes a `&format!(...)`
/// argument, across whitespace and line breaks.
fn find_translated_format_calls(text: &str) -> Vec<usize> {
    static CALL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let call = CALL.get_or_init(|| {
        regex::Regex::new(r"\bt\s*\(\s*&\s*format\s*!").expect("static pattern is valid")
    });
    let code = blank_non_code(text);
    call.find_iter(&code)
        .map(|m| code[..m.start()].matches('\n').count() + 1)
        .collect()
}

fn walk_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walk_rs_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                result.push(path);
            }
        }
    }
    result
}

#[test]
fn flags_a_format_built_inside_t() {
    assert_eq!(
        find_translated_format_calls("fn f() {\n    let s = t(&format!(\"x {}\", 1));\n}\n"),
        vec![2]
    );
}

#[test]
fn flags_a_format_on_the_line_after_t_open_paren() {
    assert_eq!(
        find_translated_format_calls("let s = t(\n    &format!(\"x\"));\n"),
        vec![1]
    );
}

#[test]
fn flags_the_module_qualified_forms() {
    let text = "a(i18n::t(&format!(\"a\")));\nb(crate::i18n::t(&format!(\"b\")));\n";
    assert_eq!(find_translated_format_calls(text), vec![1, 2]);
}

#[test]
fn ignores_the_pattern_inside_a_string_literal() {
    let text = "let s = \"t(&format!(x))\";\nlet r = r#\"t(&format!(y))\"#;\n";
    assert!(find_translated_format_calls(text).is_empty());
}

#[test]
fn ignores_the_pattern_inside_comments() {
    let text = "// t(&format!(x))\n/* t(&format!(y)) */\n/// t(&format!(z))\n";
    assert!(find_translated_format_calls(text).is_empty());
}

#[test]
fn ignores_a_variable_or_a_literal_argument() {
    let text = "let a = t(&some_var);\nlet b = t(\"plain\");\nlet c = not_t(&format!(\"x\"));\n";
    assert!(find_translated_format_calls(text).is_empty());
}

#[test]
fn line_numbers_survive_a_multiline_string_before_the_call() {
    let text = "let a = \"one\ntwo\";\nlet b = t(&format!(\"x\"));\n";
    assert_eq!(find_translated_format_calls(text), vec![3]);
}

#[test]
fn a_lifetime_does_not_hide_a_following_call() {
    let text = "fn f<'a>(x: &'a str) { t(&format!(\"{x}\")); }\n";
    assert_eq!(find_translated_format_calls(text), vec![1]);
}

#[test]
fn sources_never_translate_a_formatted_string() {
    let mut found: Vec<String> = Vec::new();
    let files = walk_rs_files(Path::new("src"));
    assert!(
        !files.is_empty(),
        "walk_rs_files found no .rs files under src/"
    );
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
        for line in find_translated_format_calls(&text) {
            found.push(format!("{}:{}", file.display(), line));
        }
    }
    assert!(
        found.is_empty(),
        "t(&format!(..)) hides the template from the catalog; translate the template, then fill it:\n{}",
        found.join("\n")
    );
}
