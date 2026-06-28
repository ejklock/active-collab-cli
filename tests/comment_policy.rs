use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Clone, Copy)]
enum ViolationKind {
    Banner,
    CommentedOutCode,
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationKind::Banner => write!(f, "Banner"),
            ViolationKind::CommentedOutCode => write!(f, "CommentedOutCode"),
        }
    }
}

fn find_line_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        let ch = bytes[i];
        if ch == b'"' {
            i += 1;
            while i < len {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else if ch == b'\'' {
            i += 1;
            while i < len {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == b'\'' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else if ch == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

fn is_banner(body: &str) -> bool {
    let stripped: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    if stripped.is_empty() {
        return true;
    }
    let divider_chars: &[char] = &['-', '=', '#', '*', '~', '_'];
    let chars: Vec<char> = body.chars().collect();
    let divider_run = chars
        .windows(4)
        .any(|w| w.iter().all(|c| divider_chars.contains(c)));
    if divider_run {
        return true;
    }
    let has_box_drawing = chars.iter().any(|c| ('\u{2500}'..='\u{257F}').contains(c));
    if has_box_drawing {
        return true;
    }
    let trimmed = body.trim();
    let leading_dividers = trimmed
        .chars()
        .take_while(|c| divider_chars.contains(c))
        .count();
    if leading_dividers >= 3 {
        let trailing_dividers = trimmed
            .chars()
            .rev()
            .take_while(|c| divider_chars.contains(c))
            .count();
        if trailing_dividers >= 3 {
            return true;
        }
    }
    false
}

fn is_commented_out_code(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }
    let code_prefixes: &[&str] = &[
        "let ", "fn ", "pub ", "use ", "if ", "for ", "while ", "match ", "return ", "return;",
        "struct ", "impl ", "enum ", "mod ", "} else", "});", "self.", "Self::",
    ];
    for prefix in code_prefixes {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }
    let last = trimmed.chars().last().unwrap_or(' ');
    if matches!(last, ';' | '{' | '}')
        && (trimmed.contains('(') || trimmed.contains('=') || trimmed.contains("::"))
    {
        return true;
    }
    false
}

fn classify_comment(line: &str) -> Option<ViolationKind> {
    let comment_start = find_line_comment(line)?;
    let comment_slice = &line[comment_start..];
    if comment_slice.starts_with("///") || comment_slice.starts_with("//!") {
        return None;
    }
    let body = comment_slice.strip_prefix("//").unwrap_or(comment_slice);
    let body = body.strip_prefix(' ').unwrap_or(body);
    if is_banner(body) {
        return Some(ViolationKind::Banner);
    }
    if is_commented_out_code(body) {
        return Some(ViolationKind::CommentedOutCode);
    }
    None
}

fn scan_source(text: &str) -> Vec<(usize, ViolationKind)> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, line)| classify_comment(line).map(|kind| (idx + 1, kind)))
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
fn find_line_comment_ignores_url_in_string_literal() {
    let line = r#"let s = "https://example.com/foo";"#;
    assert_eq!(find_line_comment(line), None);
}

#[test]
fn find_line_comment_ignores_slashes_inside_double_quoted_string() {
    let line = r#"let s = "a // b";"#;
    assert_eq!(find_line_comment(line), None);
}

#[test]
fn find_line_comment_finds_real_trailing_comment() {
    let line = r#"let x = 1; // note"#;
    let idx = find_line_comment(line).expect("should find comment");
    assert_eq!(&line[idx..], "// note");
}

#[test]
fn find_line_comment_finds_comment_in_line_with_string_and_comment() {
    let line = r#"let s = "a // b"; // real"#;
    let idx = find_line_comment(line).expect("should find comment");
    assert_eq!(&line[idx..], "// real");
}

#[test]
fn find_line_comment_returns_none_when_no_comment() {
    let line = "let x = 1 + 2;";
    assert_eq!(find_line_comment(line), None);
}

#[test]
fn find_line_comment_handles_escaped_quote_in_string() {
    let line = r#"let s = "say \"hello\" // not a comment"; // real"#;
    let idx = find_line_comment(line).expect("should find comment");
    assert!(line[idx..].starts_with("// real"));
}

#[test]
fn find_line_comment_ignores_slashes_in_char_literal() {
    let line = "let c = '/'; // comment";
    let idx = find_line_comment(line).expect("should find comment");
    assert_eq!(&line[idx..], "// comment");
}

#[test]
fn classify_doc_comment_triple_slash_returns_none() {
    assert_eq!(classify_comment("/// This is a doc comment"), None);
}

#[test]
fn classify_doc_comment_bang_returns_none() {
    assert_eq!(classify_comment("//! Module doc"), None);
}

#[test]
fn classify_banner_dash_run_returns_banner() {
    assert_eq!(classify_comment("// ----"), Some(ViolationKind::Banner));
}

#[test]
fn classify_banner_equals_run_returns_banner() {
    assert_eq!(classify_comment("// ====="), Some(ViolationKind::Banner));
}

#[test]
fn classify_banner_hash_run_returns_banner() {
    assert_eq!(classify_comment("// #####"), Some(ViolationKind::Banner));
}

#[test]
fn classify_banner_box_drawing_returns_banner() {
    assert_eq!(
        classify_comment("// \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}"),
        Some(ViolationKind::Banner)
    );
}

#[test]
fn classify_banner_section_divider_with_label_returns_banner() {
    assert_eq!(
        classify_comment("// --- setup_list ---"),
        Some(ViolationKind::Banner)
    );
}

#[test]
fn classify_commented_out_let_returns_code() {
    assert_eq!(
        classify_comment("// let x = 1;"),
        Some(ViolationKind::CommentedOutCode)
    );
}

#[test]
fn classify_commented_out_self_dot_returns_code() {
    assert_eq!(
        classify_comment("// self.update();"),
        Some(ViolationKind::CommentedOutCode)
    );
}

#[test]
fn classify_commented_out_fn_returns_code() {
    assert_eq!(
        classify_comment("// fn foo() {"),
        Some(ViolationKind::CommentedOutCode)
    );
}

#[test]
fn classify_commented_out_return_returns_code() {
    assert_eq!(
        classify_comment("// return x;"),
        Some(ViolationKind::CommentedOutCode)
    );
}

#[test]
fn classify_commented_out_bare_return_semicolon_returns_code() {
    assert_eq!(
        classify_comment("// return;"),
        Some(ViolationKind::CommentedOutCode)
    );
}

#[test]
fn classify_prose_returns_early_returns_none() {
    assert_eq!(
        classify_comment("// returns early when the cache is warm"),
        None
    );
}

#[test]
fn classify_prose_returned_value_returns_none() {
    assert_eq!(
        classify_comment("// returned value is cached for the session"),
        None
    );
}

#[test]
fn classify_prose_returning_none_returns_none() {
    assert_eq!(
        classify_comment("// returning None on miss keeps callers simple"),
        None
    );
}

#[test]
fn classify_prose_why_comment_keep_mod_returns_none() {
    assert_eq!(
        classify_comment(
            "// Keep the TEA core declared so app.rs (R0) and its tests are preserved."
        ),
        None
    );
}

#[test]
fn classify_prose_why_comment_drop_password_returns_none() {
    assert_eq!(
        classify_comment("// drop the password after the token exchange"),
        None
    );
}

#[test]
fn classify_prose_why_comment_nbsp_returns_none() {
    assert_eq!(
        classify_comment("// NBSP is preserved to match Python str.strip()"),
        None
    );
}

#[test]
fn classify_prose_ending_with_period_returns_none() {
    assert_eq!(
        classify_comment("// Mirror Python str.strip(): trim only ASCII whitespace."),
        None
    );
}

#[test]
fn classify_prose_ending_with_colon_returns_none() {
    assert_eq!(classify_comment("// Parity: Python cmd_setup_list."), None);
}

#[test]
fn scan_source_returns_violations_for_mixed_input() {
    let text = "\
fn foo() {}\n\
// --- section ---\n\
/// This is fine\n\
// let x = 1;\n\
// A normal why-comment.\n\
";
    let violations = scan_source(text);
    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0], (2, ViolationKind::Banner));
    assert_eq!(violations[1], (4, ViolationKind::CommentedOutCode));
}

#[test]
fn scan_source_returns_empty_for_clean_input() {
    let text = "\
fn foo() {}\n\
/// Doc comment\n\
// A genuine prose comment.\n\
";
    let violations = scan_source(text);
    assert!(
        violations.is_empty(),
        "expected no violations: {violations:?}"
    );
}

#[test]
fn walk_rs_files_collects_rs_files_recursively() {
    let tmp = std::env::temp_dir().join("comment_policy_walk_test");
    let sub = tmp.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.join("a.rs"), "fn a() {}").unwrap();
    std::fs::write(sub.join("b.rs"), "fn b() {}").unwrap();
    std::fs::write(tmp.join("c.txt"), "not rust").unwrap();

    let files = walk_rs_files(&tmp);
    assert!(
        files.iter().any(|p| p.ends_with("a.rs")),
        "should find a.rs"
    );
    assert!(
        files.iter().any(|p| p.ends_with("b.rs")),
        "should find sub/b.rs"
    );
    assert!(
        !files.iter().any(|p| p.ends_with("c.txt")),
        "should not include c.txt"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn rust_src_has_no_comment_policy_violations() {
    let src_dir = Path::new("src");
    let files = walk_rs_files(src_dir);
    assert!(
        !files.is_empty(),
        "walk_rs_files found no .rs files under src/"
    );

    let mut all_violations: Vec<String> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
        let violations = scan_source(&text);
        for (line_num, kind) in violations {
            all_violations.push(format!("{}:{}  {}", file.display(), line_num, kind));
        }
    }

    if !all_violations.is_empty() {
        let report = all_violations.join("\n");
        panic!("Comment policy violations found:\n{report}");
    }
}
