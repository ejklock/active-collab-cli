mod common;

use std::path::{Path, PathBuf};

use common::{lex, Span, SpanKind};

#[derive(Debug, PartialEq, Clone, Copy)]
enum ViolationKind {
    Banner,
    CommentedOutCode,
    RecordCitation,
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationKind::Banner => write!(f, "Banner"),
            ViolationKind::CommentedOutCode => write!(f, "CommentedOutCode"),
            ViolationKind::RecordCitation => write!(f, "RecordCitation"),
        }
    }
}

fn find_line_comment(line: &str) -> Option<usize> {
    lex(line)
        .into_iter()
        .find(|s| matches!(s.kind, SpanKind::Line | SpanKind::Doc))
        .map(|s| s.range.start)
}

fn is_banner(body: &str) -> bool {
    let stripped: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    if stripped.is_empty() {
        return true;
    }
    let is_box = |c: &char| ('\u{2500}'..='\u{257F}').contains(c);
    let glyphs = stripped.chars().filter(is_box).count();
    if glyphs * 2 > stripped.chars().count() {
        return true;
    }
    if body
        .chars()
        .collect::<Vec<char>>()
        .windows(3)
        .any(|w| w.iter().all(is_box))
    {
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
        "let ", "fn ", "pub ", "use ", "if ", "for ", "while ", "match ", "return;", "struct ",
        "impl ", "enum ", "mod ", "} else", "});", "self.", "Self::",
    ];
    for prefix in code_prefixes {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }
    let last = trimmed.chars().last().unwrap_or(' ');
    if trimmed.starts_with("return ") && last == ';' {
        return true;
    }
    matches!(last, ';' | '{' | '}')
        && (trimmed.contains('(') || trimmed.contains('=') || trimmed.contains("::"))
}

fn cites_record(comment: &str) -> bool {
    static CITATION: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    CITATION
        .get_or_init(|| {
            regex::Regex::new(
                r"\b(?i:adr|bdr|prd|issue)s?\s?[#-]?\s?[0-9]|\b(?i:the) (?:ADR|BDR|PRD)\b|\bAC[0-9]+\b|\bSc\.\s?[0-9]|\b(?i:slice) [0-9]+(?:[^.0-9]|\.(?:[^.0-9]|$)|$)|\b[A-Za-z]{2}-[0-9]{4}\b",
            )
                .expect("citation pattern is valid")
        })
        .is_match(comment)
}

fn classify_line_comment(comment: &str) -> Option<ViolationKind> {
    if comment.starts_with("///") || comment.starts_with("//!") {
        return None;
    }
    let body = comment.strip_prefix("//").unwrap_or(comment);
    let body = body.strip_prefix(' ').unwrap_or(body);
    if is_banner(body) {
        return Some(ViolationKind::Banner);
    }
    if is_commented_out_code(body) {
        return Some(ViolationKind::CommentedOutCode);
    }
    None
}

/// Violations of one lexed span that starts on `first_line`. A block comment gets only
/// the citation rule, checked on each of its lines; a line comment also gets the
/// banner and commented-out-code rules, a doc comment only the citation rule.
fn span_violations(
    kind: SpanKind,
    text: &str,
    first_line: usize,
    paragraph_break: bool,
) -> Vec<(usize, ViolationKind)> {
    match kind {
        SpanKind::Code | SpanKind::Literal => Vec::new(),
        SpanKind::Block => text
            .lines()
            .enumerate()
            .filter(|(_, line)| cites_record(line))
            .map(|(offset, _)| (first_line + offset, ViolationKind::RecordCitation))
            .collect(),
        SpanKind::Line | SpanKind::Doc => {
            let found = if cites_record(text) {
                Some(ViolationKind::RecordCitation)
            } else if paragraph_break {
                None
            } else {
                classify_line_comment(text)
            };
            found.map(|k| (first_line, k)).into_iter().collect()
        }
    }
}

/// True when the span next to `index` (before it, or after it) is a line comment on the
/// very next line, with only blanks between.
fn comment_on_adjacent_line(spans: &[Span], text: &str, index: usize, before: bool) -> bool {
    let gap_index = if before {
        index.checked_sub(1)
    } else {
        Some(index + 1)
    };
    let Some(gap) = gap_index.and_then(|g| spans.get(g)) else {
        return false;
    };
    let gap_text = &text[gap.range.clone()];
    if gap.kind != SpanKind::Code
        || gap_text.matches('\n').count() != 1
        || !gap_text.trim().is_empty()
    {
        return false;
    }
    let neighbor = if before {
        index.checked_sub(2)
    } else {
        Some(index + 2)
    };
    neighbor
        .and_then(|n| spans.get(n))
        .is_some_and(|s| matches!(s.kind, SpanKind::Line | SpanKind::Doc))
}

/// The first violation per line of a source text.
fn scan_source(text: &str) -> Vec<(usize, ViolationKind)> {
    let mut counted_to = 0;
    let mut line = 1;
    let mut found: Vec<(usize, ViolationKind)> = Vec::new();
    let spans = lex(text);
    for (index, span) in spans.iter().enumerate() {
        line += text[counted_to..span.range.start].matches('\n').count();
        counted_to = span.range.start;
        let body = &text[span.range.clone()];
        let paragraph_break = body == "//"
            && comment_on_adjacent_line(&spans, text, index, true)
            && comment_on_adjacent_line(&spans, text, index, false);
        found.extend(span_violations(span.kind, body, line, paragraph_break));
    }
    found.dedup_by_key(|(line, _)| *line);
    found
}

fn classify_comment(line: &str) -> Option<ViolationKind> {
    scan_source(line).first().map(|(_, kind)| *kind)
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
fn classify_line_comment_citing_adr_returns_record_citation() {
    assert_eq!(
        classify_comment(concat!("// see ADR", " 0066")),
        Some(ViolationKind::RecordCitation)
    );
}

#[test]
fn classify_doc_comment_citing_adr_and_issue_returns_record_citation() {
    assert_eq!(
        classify_comment(concat!("/// (ADR", " 0068, issue", " 0062 C1).")),
        Some(ViolationKind::RecordCitation)
    );
}

#[test]
fn classify_module_doc_citing_issue_returns_record_citation() {
    assert_eq!(
        classify_comment(concat!("//! ... (issue", " 0062 C1).")),
        Some(ViolationKind::RecordCitation)
    );
}

#[test]
fn classify_plural_adrs_citation_returns_record_citation() {
    assert_eq!(
        classify_comment(concat!("// ADRs", " 0033/0054")),
        Some(ViolationKind::RecordCitation)
    );
}

#[test]
fn classify_control_character_range_names_return_none() {
    assert_eq!(
        classify_comment("/// C0 (U+0000-U+001F), DEL (U+007F)"),
        None
    );
}

#[test]
fn classify_word_ending_in_issue_before_a_digit_returns_none() {
    assert_eq!(classify_comment("// a tissue 3 mm thick"), None);
}

#[test]
fn classify_url_with_issues_path_returns_none() {
    assert_eq!(classify_comment("// https://example.com/issues/12"), None);
}

#[test]
fn classify_citation_inside_string_literal_without_comment_returns_none() {
    assert_eq!(
        classify_comment(concat!(r#"let s = "ADR"#, r#" 0066";"#)),
        None
    );
}

#[test]
fn classify_issue_without_digit_returns_none() {
    assert_eq!(classify_comment("// the issue is a race"), None);
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

const CITATION_FORMS: [&str; 13] = [
    concat!("issue", " #45"),
    concat!("adr", "0012"),
    concat!("ADR", "-12"),
    concat!("Issues", " 7"),
    "the ADR",
    "the BDR",
    "the PRD",
    "AC4",
    "Sc.8",
    "Sc.8b",
    concat!("slice", " 0059"),
    concat!("Slice", " 3"),
    concat!("ek", "-0061"),
];

const NOT_CITATIONS: [&str; 10] = [
    "S1",
    "V6",
    "tissue 4",
    "PACKAGE",
    "ACK1",
    "AC alone",
    "the address",
    "x-1234",
    "abc-1234",
    "a2-0001",
];

#[test]
fn every_citation_form_is_a_violation_in_line_doc_and_block_comments() {
    for form in CITATION_FORMS {
        for template in [
            "// see {}",
            "/// see {}",
            "//! see {}",
            "/* see {} */",
            "/** see {} */",
        ] {
            let line = template.replace("{}", form);
            assert_eq!(
                classify_comment(&line),
                Some(ViolationKind::RecordCitation),
                "{line}"
            );
        }
    }
}

#[test]
fn citation_on_second_line_of_block_comment_is_reported_at_that_line() {
    for form in CITATION_FORMS {
        let text = format!("fn a() {{}}\n/* first\n   see {form}\n*/\nfn b() {{}}\n");
        assert_eq!(
            scan_source(&text),
            vec![(3, ViolationKind::RecordCitation)],
            "{form}"
        );
    }
}

#[test]
fn text_after_a_closed_block_comment_is_code_again() {
    let text = "/* note\n*/ let s = \"AC4\";\nlet t = 1; /* ok */ let u = 2;\n";
    assert_eq!(scan_source(text), vec![]);
}

#[test]
fn citation_after_a_nested_block_comment_closes_is_still_inside_the_outer_block() {
    let text = "/* outer /* inner */\n   see AC4\n*/\nlet x = 1;\n";
    assert_eq!(scan_source(text), vec![(2, ViolationKind::RecordCitation)]);
}

#[test]
fn non_citation_text_is_not_a_violation_in_any_comment_shape() {
    for text in NOT_CITATIONS {
        for template in ["// see {}", "/// see {}", "/* see {} */"] {
            let line = template.replace("{}", text);
            assert_eq!(classify_comment(&line), None, "{line}");
        }
    }
}

#[test]
fn citation_in_normal_or_raw_string_literal_is_not_a_violation() {
    assert_eq!(classify_comment(r#"let s = "see AC4";"#), None);
    assert_eq!(
        classify_comment(concat!(r##"let s = r#"see ADR"##, r##"-12"#;"##)),
        None
    );
    assert_eq!(classify_comment(r#"let s = "/* AC4 */";"#), None);
    assert_eq!(classify_comment("// https://example.com/issues/12"), None);
}

#[test]
fn the_adr_at_a_sentence_start_is_a_citation() {
    assert_eq!(
        classify_comment("// The ADR forbids this"),
        Some(ViolationKind::RecordCitation)
    );
}

#[test]
fn a_slice_range_in_prose_is_not_a_citation() {
    assert_eq!(classify_comment("// take slice 0..n of the buffer"), None);
    assert_eq!(classify_comment("// take slice 12..n of the buffer"), None);
    assert_eq!(classify_comment("// take slice 3.5 of the buffer"), None);
}

#[test]
fn a_slice_number_ending_a_sentence_is_a_citation() {
    assert_eq!(
        classify_comment("// see slice 3."),
        Some(ViolationKind::RecordCitation)
    );
}

#[test]
fn a_block_opener_inside_a_multiline_string_does_not_hide_later_lines() {
    let text = "let s = \"a\nsrc/*.rs\nend\";\n// see AC4\nlet t = \"AC4\";\n";
    assert_eq!(scan_source(text), vec![(4, ViolationKind::RecordCitation)]);
}

#[test]
fn a_comment_marker_inside_a_raw_string_is_not_a_comment() {
    assert_eq!(scan_source("let s = r#\"a\"/* AC4 */\"#;\n"), vec![]);
}

#[test]
fn a_lifetime_tick_does_not_hide_a_following_block_comment() {
    assert_eq!(
        scan_source("fn f(x: &'a str) /* see AC4 */ {}\n"),
        vec![(1, ViolationKind::RecordCitation)]
    );
}

#[test]
fn a_raw_byte_string_ending_in_a_backslash_does_not_hide_the_next_comment() {
    let text = "let s = br\"a\\\";\n// see AC4\n";
    assert_eq!(scan_source(text), vec![(2, ViolationKind::RecordCitation)]);
}

#[test]
fn a_raw_c_string_is_a_literal() {
    assert_eq!(scan_source("let s = cr#\"x\"// AC4\"#;\n"), vec![]);
}

#[test]
fn an_identifier_ending_in_r_keeps_the_following_string_normal() {
    let text = "bar\"x\"";
    let kinds: Vec<(SpanKind, &str)> = lex(text)
        .into_iter()
        .map(|s| (s.kind, &text[s.range]))
        .collect();
    assert_eq!(
        kinds,
        vec![(SpanKind::Code, "bar"), (SpanKind::Literal, "\"x\"")]
    );
}

#[test]
fn a_plain_raw_string_ending_in_a_backslash_does_not_hide_the_next_comment() {
    let text = "let s = r\"a\\\";\n// see AC4\n";
    assert_eq!(scan_source(text), vec![(2, ViolationKind::RecordCitation)]);
}

#[test]
fn a_raw_prefix_after_a_longer_identifier_keeps_the_string_normal() {
    let text = "xbr\"a\\\";\n// see AC4\nlet t = \"z\";\n";
    assert_eq!(scan_source(text), vec![]);
}

#[test]
fn a_box_drawing_glyph_inside_prose_is_not_a_banner() {
    assert_eq!(classify_comment("// Col 0 is the \u{2502} border"), None);
    assert_eq!(
        classify_comment("// Lines are boxed (\u{2502} \u{2026} \u{2502})"),
        None
    );
}

#[test]
fn a_labelled_box_drawing_divider_is_a_banner() {
    assert_eq!(
        classify_comment(
            "// \u{2500}\u{2500} Image viewer overlay render tests \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}"
        ),
        Some(ViolationKind::Banner)
    );
    assert_eq!(
        classify_comment(
            "// \u{2500}\u{2500} Hours-report modal: export outcome mapping and pointer isolation \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}"
        ),
        Some(ViolationKind::Banner)
    );
    assert_eq!(
        classify_comment("// \u{2500}\u{2500}\u{2500}"),
        Some(ViolationKind::Banner)
    );
}

#[test]
fn two_box_drawing_glyphs_in_prose_are_not_a_banner() {
    assert_eq!(classify_comment("// a \u{2500}\u{2500} b"), None);
}

#[test]
fn a_comment_made_mostly_of_glyphs_is_a_banner() {
    assert_eq!(
        classify_comment("// \u{2502} \u{2502} \u{2502} \u{2502}"),
        Some(ViolationKind::Banner)
    );
}

#[test]
fn a_bare_slashes_line_between_two_comment_lines_is_a_paragraph_break() {
    assert_eq!(scan_source("// one\n//\n// two\n"), vec![]);
    assert_eq!(scan_source("    // one\n    //\n    // two\n"), vec![]);
}

#[test]
fn a_bare_slashes_line_that_is_the_whole_comment_block_is_a_banner() {
    assert_eq!(
        scan_source("let x = 1;\n//\nlet y = 2;\n"),
        vec![(2, ViolationKind::Banner)]
    );
    assert_eq!(
        scan_source("// one\n//\nlet y = 2;\n"),
        vec![(2, ViolationKind::Banner)]
    );
    assert_eq!(classify_comment("//"), Some(ViolationKind::Banner));
}

#[test]
fn prose_starting_with_return_is_not_commented_out_code() {
    assert_eq!(
        classify_comment("// return the character after the emoji"),
        None
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
fn rust_sources_have_no_comment_policy_violations() {
    let mut all_violations: Vec<String> = Vec::new();
    for root in ["src", "tests", "examples"] {
        let files = walk_rs_files(Path::new(root));
        assert!(
            !files.is_empty(),
            "walk_rs_files found no .rs files under {root}/"
        );
        for file in &files {
            let text = std::fs::read_to_string(file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
            for (line_num, kind) in scan_source(&text) {
                all_violations.push(format!("{}:{}  {}", file.display(), line_num, kind));
            }
        }
    }

    if !all_violations.is_empty() {
        let report = all_violations.join("\n");
        panic!("Comment policy violations found:\n{report}");
    }
}
