mod common;

use std::path::{Path, PathBuf};

use common::{lex, SpanKind};

/// Replace comments and string or char literals with spaces, keeping every newline,
/// so a pattern search sees only real code and reports the right line numbers.
fn blank_non_code(text: &str) -> String {
    let mut out = text.as_bytes().to_vec();
    for span in lex(text).into_iter().filter(|s| s.kind != SpanKind::Code) {
        for byte in &mut out[span.range] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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
