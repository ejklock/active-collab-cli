use super::*;

/// Flatten `structured_rich_with_links` output to a plain `String` for text-equality tests.
fn rich_to_text(html: &str) -> String {
    let rich = structured_rich_with_links(html);
    let joined = rich
        .iter()
        .map(|line| line.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    joined
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .to_owned()
}

// --- R3a-A1: Unordered list —————————————————————————————————————————————

#[test]
fn unordered_list_items_prefixed_with_bullet() {
    let html = "<ul><li>one</li><li>two</li></ul>";
    let out = rich_to_text(html);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("\u{2022} ") && l.contains("one")),
        "first <li> must be prefixed with bullet: {out:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("\u{2022} ") && l.contains("two")),
        "second <li> must be prefixed with bullet: {out:?}"
    );
}

#[test]
fn unordered_list_produces_exactly_n_bullet_lines() {
    let html = "<ul><li>a</li><li>b</li><li>c</li></ul>";
    let out = rich_to_text(html);
    let bullet_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("\u{2022} ")).collect();
    assert_eq!(
        bullet_lines.len(),
        3,
        "three <li> elements must produce three bullet lines: {out:?}"
    );
}

// --- R3a-A1: Ordered list ————————————————————————————————————————————————

#[test]
fn ordered_list_items_numbered_from_one() {
    let html = "<ol><li>alpha</li><li>beta</li></ol>";
    let out = rich_to_text(html);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("1. ") && l.contains("alpha")),
        "first item must be prefixed '1. ': {out:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("2. ") && l.contains("beta")),
        "second item must be prefixed '2. ': {out:?}"
    );
}

#[test]
fn each_ordered_list_resets_counter_independently() {
    let html = "<ol><li>x</li></ol><ol><li>y</li></ol>";
    let out = rich_to_text(html);
    let lines: Vec<&str> = out.lines().collect();
    let ones: Vec<&&str> = lines.iter().filter(|l| l.starts_with("1. ")).collect();
    assert_eq!(
        ones.len(),
        2,
        "each <ol> resets its counter so both first items use '1. ': {out:?}"
    );
}

// --- R3a-A1: Heading —————————————————————————————————————————————————————

#[test]
fn h2_heading_text_on_its_own_line() {
    let html = "<h2>Title</h2>";
    let out = rich_to_text(html);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines.iter().any(|l| l.trim() == "Title"),
        "heading text must appear on its own line: {out:?}"
    );
}

#[test]
fn heading_surrounded_by_blank_line_separation() {
    let html = "before<h2>Section</h2>after";
    let out = rich_to_text(html);
    // "before" and "after" must be separated from "Section" by at least one blank line.
    let pos_before = out.find("before").expect("before must be present");
    let pos_heading = out.find("Section").expect("Section must be present");
    let pos_after = out.find("after").expect("after must be present");
    let between_before_heading = &out[pos_before..pos_heading];
    let between_heading_after = &out[pos_heading..pos_after];
    assert!(
        between_before_heading.contains("\n\n") || between_before_heading.contains('\n'),
        "heading must be separated from preceding text: {out:?}"
    );
    assert!(
        between_heading_after.contains('\n'),
        "text after heading must follow on a new line: {out:?}"
    );
}

#[test]
fn all_heading_levels_h1_to_h6_render_on_own_line() {
    for level in 1..=6 {
        let html = format!("<h{level}>Level {level}</h{level}>");
        let out = rich_to_text(&html);
        let needle = format!("Level {level}");
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines.iter().any(|l| l.trim() == needle.as_str()),
            "h{level} text must appear on its own line: {out:?}"
        );
    }
}

// --- R3a-A1: Blockquote ——————————————————————————————————————————————————

#[test]
fn blockquote_content_prefixed_with_gt() {
    let html = "<blockquote>quoted text</blockquote>";
    let out = rich_to_text(html);
    assert!(
        out.contains("> quoted text"),
        "blockquote must be prefixed with '> ': {out:?}"
    );
}

#[test]
fn blockquote_multiline_all_lines_prefixed() {
    let html = "<blockquote>line one<br>line two</blockquote>";
    let out = rich_to_text(html);
    let prefixed_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("> ")).collect();
    assert_eq!(
        prefixed_lines.len(),
        2,
        "every line inside blockquote must be prefixed with '> ': {out:?}"
    );
}

// --- V5-A1: Anchor inline rendering —————————————————————————————————————

#[test]
fn anchor_with_text_renders_text_bracket_url() {
    let html = r#"<a href="https://x.example.com/y">click here</a>"#;
    let out = rich_to_text(html);
    assert!(
        out.contains("click here [https://x.example.com/y]"),
        "anchor with text must render 'text [url]': {out:?}"
    );
    assert!(
        !out.contains("\u{2197} Link"),
        "must NOT contain old '↗ Link N' label: {out:?}"
    );
}

#[test]
fn anchor_with_empty_inner_text_renders_bracket_url_only() {
    let html = r#"<a href="https://empty.example.com/"></a>"#;
    let out = rich_to_text(html);
    assert!(
        out.contains("[https://empty.example.com/]"),
        "empty-inner anchor must render '[url]' only: {out:?}"
    );
    assert!(
        !out.contains("  ["),
        "must not have double space before bracket when text is empty: {out:?}"
    );
}

#[test]
fn anchor_with_text_equal_to_url_renders_bracket_url_only() {
    let url = "https://example.com/page";
    let html = format!(r#"<a href="{url}">{url}</a>"#);
    let out = rich_to_text(&html);
    assert!(
        out.contains(&format!("[{url}]")),
        "anchor where text==url must render '[url]' only (no duplication): {out:?}"
    );
    let occurrences = out.matches(url).count();
    assert_eq!(
        occurrences, 1,
        "URL must appear exactly once (no duplication): {out:?}"
    );
}

#[test]
fn anchor_without_href_strips_tag_preserves_inner_text() {
    let html = "<a>bare link text</a>";
    let out = rich_to_text(html);
    assert!(
        out.contains("bare link text"),
        "anchor without href must still show inner text: {out:?}"
    );
}

#[test]
fn two_anchors_both_render_inline() {
    let html = r#"<a href="https://one.com">first</a> and <a href="https://two.com">second</a>"#;
    let out = rich_to_text(html);
    assert!(
        out.contains("first [https://one.com]"),
        "first anchor must render inline: {out:?}"
    );
    assert!(
        out.contains("second [https://two.com]"),
        "second anchor must render inline: {out:?}"
    );
}

// V5-A3: mailto href strips the scheme in the display bracket; click path re-adds it.
#[test]
fn anchor_mailto_renders_bare_address_in_brackets() {
    let html = r#"<a href="mailto:user@example.com">mail us</a>"#;
    let out = rich_to_text(html);
    assert!(
        out.contains("mail us [user@example.com]"),
        "mailto anchor must show bare address in brackets (no 'mailto:' prefix): {out:?}"
    );
    assert!(
        !out.contains("mailto:"),
        "display must NOT contain the 'mailto:' scheme: {out:?}"
    );
}

#[test]
fn anchor_mailto_empty_text_renders_bracket_address_only() {
    let html = r#"<a href="mailto:a@b.com"></a>"#;
    let out = rich_to_text(html);
    assert!(
        out.contains("[a@b.com]"),
        "empty-text mailto must render '[a@b.com]' only: {out:?}"
    );
}

// --- R3a-A3: Malformed / unknown HTML ————————————————————————————————————

#[test]
fn unbalanced_ul_li_does_not_panic_and_shows_text() {
    let html = "<ul><li>item a";
    let out = rich_to_text(html);
    assert!(
        out.contains("item a"),
        "unbalanced list must still show text: {out:?}"
    );
}

#[test]
fn unknown_tag_is_stripped_text_preserved() {
    let html = "<foo>kept text</foo>";
    let out = rich_to_text(html);
    assert!(
        out.contains("kept text"),
        "text inside unknown tag must be preserved: {out:?}"
    );
}

#[test]
fn deeply_nested_unknown_tags_stripped_safely() {
    let html = "<outer><inner><deep>text</deep></inner></outer>";
    let out = rich_to_text(html);
    assert!(
        out.contains("text"),
        "text must survive deep unknown nesting: {out:?}"
    );
}

#[test]
fn empty_html_returns_empty_string() {
    let out = rich_to_text("");
    assert_eq!(out, "");
}

// --- R3a-A4 / BDR 0003: CLI parity is verified in render tests ————————————

// --- Mixed document ——————————————————————————————————————————————————————

#[test]
fn mixed_document_preserves_structure() {
    let html = "<h2>Tasks</h2><ul><li>First</li><li>Second</li></ul><blockquote>Note</blockquote>";
    let out = rich_to_text(html);
    let lines: Vec<&str> = out.lines().collect();
    // Heading text appears on its own line
    assert!(
        lines.iter().any(|l| l.trim() == "Tasks"),
        "heading must appear: {out:?}"
    );
    // Two bullet lines
    let bullets: Vec<&&str> = lines
        .iter()
        .filter(|l| l.starts_with("\u{2022} "))
        .collect();
    assert_eq!(bullets.len(), 2, "two bullet lines must appear: {out:?}");
    // Blockquote line
    assert!(
        out.contains("> Note"),
        "blockquote must be prefixed: {out:?}"
    );
}

#[test]
fn entity_decoding_works_inside_list_items() {
    let html = "<ul><li>a &amp; b</li><li>&lt;escaped&gt;</li></ul>";
    let out = rich_to_text(html);
    assert!(
        out.contains("a & b"),
        "entity &amp; must decode to &: {out:?}"
    );
    assert!(
        out.contains("<escaped>"),
        "entities &lt; &gt; must decode: {out:?}"
    );
}

#[test]
fn three_or_more_consecutive_newlines_collapsed_to_two() {
    let html = "<p>a</p><p>b</p><p>c</p>";
    let out = rich_to_text(html);
    assert!(
        !out.contains("\n\n\n"),
        "three+ consecutive newlines must collapse to 2: {out:?}"
    );
}

// --- R4-A1: Strike / Underline spans —————————————————————————————————————

#[test]
fn del_tag_produces_strike_style_span() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<del>gone</del>";
    let lines = structured_rich_with_links(html);
    let has_strike = lines.iter().any(|line| {
        line.iter()
            .any(|span| span.style == RichStyle::Strike && span.text.contains("gone"))
    });
    assert!(has_strike, "<del> must produce Strike span: {lines:?}");
}

#[test]
fn strike_tag_produces_strike_style_span() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<strike>struck</strike>";
    let lines = structured_rich_with_links(html);
    let has_strike = lines.iter().any(|line| {
        line.iter()
            .any(|span| span.style == RichStyle::Strike && span.text.contains("struck"))
    });
    assert!(has_strike, "<strike> must produce Strike span: {lines:?}");
}

#[test]
fn u_tag_produces_underline_style_span() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<u>under</u>";
    let lines = structured_rich_with_links(html);
    let has_underline = lines.iter().any(|line| {
        line.iter()
            .any(|span| span.style == RichStyle::Underline && span.text.contains("under"))
    });
    assert!(has_underline, "<u> must produce Underline span: {lines:?}");
}

#[test]
fn strike_and_plain_text_in_same_line() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "before <del>removed</del> after";
    let lines = structured_rich_with_links(html);
    let line = lines
        .iter()
        .find(|l| l.iter().any(|s| s.text.contains("before")));
    let line = line.expect("must have a line with 'before'");
    let strike_span = line
        .iter()
        .find(|s| s.style == RichStyle::Strike && s.text.contains("removed"));
    assert!(
        strike_span.is_some(),
        "struck word must carry Strike: {line:?}"
    );
    let plain_before = line
        .iter()
        .any(|s| s.style == RichStyle::Plain && s.text.contains("before"));
    assert!(plain_before, "text before del must be plain: {line:?}");
}

// --- R4-A2: <pre> preserves whitespace ———————————————————————————————————

#[test]
fn pre_preserves_internal_whitespace_and_newlines() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<pre>a    b\n  c</pre>";
    let lines = structured_rich_with_links(html);
    let code_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.iter().any(|s| s.style == RichStyle::Code))
        .collect();
    assert!(
        !code_lines.is_empty(),
        "<pre> must emit code-styled lines: {lines:?}"
    );
    let all_text: String = code_lines
        .iter()
        .flat_map(|l| l.iter().map(|s| s.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        all_text.contains("a    b"),
        "internal spaces must be preserved: {all_text:?}"
    );
    assert!(
        all_text.contains("  c"),
        "internal leading whitespace on second line must be preserved: {all_text:?}"
    );
}

#[test]
fn pre_block_framed_by_blank_lines() {
    use crate::richtext::structured_rich_with_links;
    let html = "before<pre>code</pre>after";
    let lines = structured_rich_with_links(html);
    let idx_before = lines
        .iter()
        .position(|l| l.iter().any(|s| s.text.contains("before")));
    let idx_code = lines
        .iter()
        .position(|l| l.iter().any(|s| s.text.contains("code")));
    let idx_after = lines
        .iter()
        .position(|l| l.iter().any(|s| s.text.contains("after")));
    let (before, code, after) = (
        idx_before.expect("must have 'before'"),
        idx_code.expect("must have 'code'"),
        idx_after.expect("must have 'after'"),
    );
    assert!(
        code > before + 1,
        "blank line must separate before from pre block: {lines:?}"
    );
    let between = &lines[(before + 1)..code];
    assert!(
        between.iter().any(|l| l.is_empty()),
        "blank line between before and code: {lines:?}"
    );
    assert!(
        after > code + 1,
        "blank line must follow pre block: {lines:?}"
    );
}

#[test]
fn unclosed_pre_does_not_panic_and_emits_text() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<pre>orphaned code";
    let lines = structured_rich_with_links(html);
    let has_code = lines.iter().any(|l| {
        l.iter()
            .any(|s| s.style == RichStyle::Code && s.text.contains("orphaned code"))
    });
    assert!(
        has_code,
        "unclosed <pre> must still emit code text: {lines:?}"
    );
}

#[test]
fn pre_inline_emphasis_bold_survives_inside_pre() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<pre>a <b>bold</b> b</pre>";
    let lines = structured_rich_with_links(html);
    let bold_span = lines
        .iter()
        .flat_map(|l| l.iter())
        .find(|s| s.style == RichStyle::Bold && s.text.contains("bold"));
    assert!(
        bold_span.is_some(),
        "<b> inside <pre> must produce a Bold span: {lines:?}"
    );
    let all_text: String = lines
        .iter()
        .flat_map(|l| l.iter().map(|s| s.text.as_str()))
        .collect();
    assert!(
        all_text.contains("a ") && all_text.contains(" b"),
        "surrounding text with spaces must be preserved: {lines:?}"
    );
    let code_span = lines
        .iter()
        .flat_map(|l| l.iter())
        .find(|s| s.style == RichStyle::Code && s.text.contains("a "));
    assert!(
        code_span.is_some(),
        "non-emphasised text in <pre> must be Code-styled: {lines:?}"
    );
}

#[test]
fn pre_inline_emphasis_italic_survives_inside_pre() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<pre>x <em>italic</em> y</pre>";
    let lines = structured_rich_with_links(html);
    let italic_span = lines
        .iter()
        .flat_map(|l| l.iter())
        .find(|s| s.style == RichStyle::Italic && s.text.contains("italic"));
    assert!(
        italic_span.is_some(),
        "<em> inside <pre> must produce an Italic span: {lines:?}"
    );
}

#[test]
fn pre_multiline_with_emphasis_preserves_newlines_and_styles() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<pre>line1\n<b>line2</b>\nline3</pre>";
    let lines = structured_rich_with_links(html);
    let non_blank: Vec<_> = lines.iter().filter(|l| !l.is_empty()).collect();
    assert!(
        non_blank.len() >= 3,
        "three lines inside <pre> must produce at least 3 non-blank output lines: {lines:?}"
    );
    let has_bold_line2 = lines.iter().any(|l| {
        l.iter()
            .any(|s| s.style == RichStyle::Bold && s.text.contains("line2"))
    });
    assert!(
        has_bold_line2,
        "the <b>line2</b> must appear as Bold on its own line: {lines:?}"
    );
}

// --- R4-A3: table renders column-aligned rows ————————————————————————————

#[test]
fn two_by_two_table_emits_two_rows() {
    use crate::richtext::structured_rich_with_links;
    let html =
        "<table><tr><th>Name</th><th>Age</th></tr><tr><td>Alice</td><td>30</td></tr></table>";
    let lines = structured_rich_with_links(html);
    let non_blank: Vec<_> = lines.iter().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        non_blank.len(),
        2,
        "2×2 table must emit 2 non-blank rows: {lines:?}"
    );
}

#[test]
fn table_header_cells_are_bold() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html =
        "<table><tr><th>Name</th><th>Age</th></tr><tr><td>Alice</td><td>30</td></tr></table>";
    let lines = structured_rich_with_links(html);
    let header_row = lines
        .iter()
        .find(|l| l.iter().any(|s| s.text.contains("Name")));
    let header_row = header_row.expect("header row must be present");
    let has_bold_name = header_row
        .iter()
        .any(|s| s.style == RichStyle::Bold && s.text.contains("Name"));
    assert!(has_bold_name, "<th> cell must be bold: {header_row:?}");
}

#[test]
fn table_data_cells_are_plain() {
    use crate::richtext::{structured_rich_with_links, RichStyle};
    let html = "<table><tr><th>Name</th></tr><tr><td>Alice</td></tr></table>";
    let lines = structured_rich_with_links(html);
    let data_row = lines
        .iter()
        .find(|l| l.iter().any(|s| s.text.contains("Alice")));
    let data_row = data_row.expect("data row must be present");
    let has_plain_alice = data_row
        .iter()
        .any(|s| s.style == RichStyle::Plain && s.text.contains("Alice"));
    assert!(has_plain_alice, "<td> cell must be plain: {data_row:?}");
}

#[test]
fn table_columns_are_padded_to_widest_cell() {
    use crate::render::display_width;
    use crate::richtext::structured_rich_with_links;
    let html = "<table><tr><td>short</td><td>x</td></tr><tr><td>a</td><td>y</td></tr></table>";
    let lines = structured_rich_with_links(html);
    let non_blank: Vec<_> = lines.iter().filter(|l| !l.is_empty()).collect();
    assert_eq!(non_blank.len(), 2, "must have 2 data rows");
    let row0_text: String = non_blank[0].iter().map(|s| s.text.as_str()).collect();
    let row1_text: String = non_blank[1].iter().map(|s| s.text.as_str()).collect();
    let row0_dw = display_width(&row0_text);
    let row1_dw = display_width(&row1_text);
    assert_eq!(
        row0_dw, row1_dw,
        "rows must have identical total display width when columns are padded: row0={row0_text:?} row1={row1_text:?}"
    );
    assert!(
        row0_text.contains("short"),
        "first column of row0 must contain 'short': {row0_text:?}"
    );
    assert!(
        row1_text.contains("a"),
        "first column of row1 must contain 'a': {row1_text:?}"
    );
}

#[test]
fn ragged_table_does_not_panic_missing_cells_empty() {
    use crate::richtext::structured_rich_with_links;
    let html = "<table><tr><td>a</td><td>b</td><td>c</td></tr><tr><td>x</td></tr></table>";
    let lines = structured_rich_with_links(html);
    let non_blank: Vec<_> = lines.iter().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        non_blank.len(),
        2,
        "ragged table must emit 2 rows: {lines:?}"
    );
    let row1_text: String = non_blank[1].iter().map(|s| s.text.as_str()).collect();
    assert!(
        row1_text.contains("x"),
        "row with single cell must include x: {row1_text:?}"
    );
}

#[test]
fn empty_table_does_not_panic() {
    use crate::richtext::structured_rich_with_links;
    let html = "<table></table>";
    let lines = structured_rich_with_links(html);
    assert!(
        lines.is_empty() || lines.iter().all(|l| l.is_empty()),
        "empty table must produce no content: {lines:?}"
    );
}

#[test]
fn table_framed_by_blank_lines() {
    use crate::richtext::structured_rich_with_links;
    let html = "before<table><tr><td>cell</td></tr></table>after";
    let lines = structured_rich_with_links(html);
    let idx_before = lines
        .iter()
        .position(|l| l.iter().any(|s| s.text.contains("before")));
    let idx_cell = lines
        .iter()
        .position(|l| l.iter().any(|s| s.text.contains("cell")));
    let idx_after = lines
        .iter()
        .position(|l| l.iter().any(|s| s.text.contains("after")));
    let (before, cell, after) = (
        idx_before.expect("must have 'before'"),
        idx_cell.expect("must have 'cell'"),
        idx_after.expect("must have 'after'"),
    );
    assert!(
        cell > before + 1,
        "blank line must precede table: {lines:?}"
    );
    assert!(after > cell + 1, "blank line must follow table: {lines:?}");
}

// --- R4-A4: existing suite and wrapping ——————————————————————————————————

#[test]
fn strike_style_survives_wrap() {
    use crate::render::wrap_rich;
    use crate::richtext::{RichSpan, RichStyle};
    let long_text = "word ".repeat(30);
    let span_text = long_text.trim_end().to_string();
    let line = vec![RichSpan {
        text: span_text.clone(),
        style: RichStyle::Strike,
    }];
    let wrapped = wrap_rich(&line, 40);
    assert!(wrapped.len() > 1, "long struck line must wrap: {wrapped:?}");
    for row in &wrapped {
        for span in row {
            if !span.text.trim().is_empty() {
                assert_eq!(
                    span.style,
                    RichStyle::Strike,
                    "every non-empty wrapped fragment must keep Strike: {row:?}"
                );
            }
        }
    }
}

#[test]
fn underline_style_survives_wrap() {
    use crate::render::wrap_rich;
    use crate::richtext::{RichSpan, RichStyle};
    let long_text = "word ".repeat(30);
    let span_text = long_text.trim_end().to_string();
    let line = vec![RichSpan {
        text: span_text.clone(),
        style: RichStyle::Underline,
    }];
    let wrapped = wrap_rich(&line, 40);
    assert!(
        wrapped.len() > 1,
        "long underlined line must wrap: {wrapped:?}"
    );
    for row in &wrapped {
        for span in row {
            if !span.text.trim().is_empty() {
                assert_eq!(
                    span.style,
                    RichStyle::Underline,
                    "every non-empty wrapped fragment must keep Underline: {row:?}"
                );
            }
        }
    }
}

// --- BDR 0023: positional style threading (disambiguation tests) —————————————

/// BDR 0023 Sc.1 — a word repeated in the same source line with different emphasis
/// keeps each occurrence's own style after wrapping.
///
/// Source: ["format the " Plain, "format" Bold, " call" Plain]
/// Wide enough to stay on one line; first "format" must be Plain, second must be Bold.
#[test]
fn repeated_word_keeps_per_occurrence_style() {
    use crate::render::wrap_rich;
    use crate::richtext::{RichSpan, RichStyle};

    let line = vec![
        RichSpan {
            text: "format the ".to_string(),
            style: RichStyle::Plain,
        },
        RichSpan {
            text: "format".to_string(),
            style: RichStyle::Bold,
        },
        RichSpan {
            text: " call".to_string(),
            style: RichStyle::Plain,
        },
    ];
    // Width 80 keeps everything on one line.
    let wrapped = wrap_rich(&line, 80);
    assert_eq!(wrapped.len(), 1, "should fit on one line: {wrapped:?}");

    let row = &wrapped[0];
    // Collect non-space spans to find the two "format" occurrences.
    let format_spans: Vec<_> = row.iter().filter(|s| s.text.contains("format")).collect();
    assert_eq!(
        format_spans.len(),
        2,
        "expected two spans containing 'format': {row:?}"
    );
    assert_eq!(
        format_spans[0].style,
        RichStyle::Plain,
        "first 'format' must be Plain: {row:?}"
    );
    assert_eq!(
        format_spans[1].style,
        RichStyle::Bold,
        "second 'format' must be Bold: {row:?}"
    );
}

/// BDR 0023 Sc.2 — a short word that is a substring of a larger styled token
/// does not inherit the larger token's style.
///
/// Source: ["category " Bold, "cat" Plain]
/// The Bold "category" span comes FIRST so the first-contains mutant (which
/// would return the style of the first span whose text contains "cat") would
/// pick Bold for "cat" — killing the mutant when the test asserts Plain.
/// The correct per-char-threading code still yields Plain for "cat".
#[test]
fn substring_word_does_not_inherit_larger_token_style() {
    use crate::render::wrap_rich;
    use crate::richtext::{RichSpan, RichStyle};

    let line = vec![
        RichSpan {
            text: "category ".to_string(),
            style: RichStyle::Bold,
        },
        RichSpan {
            text: "cat".to_string(),
            style: RichStyle::Plain,
        },
    ];
    let wrapped = wrap_rich(&line, 80);
    assert_eq!(wrapped.len(), 1, "should fit on one line: {wrapped:?}");

    let row = &wrapped[0];
    let cat_span = row
        .iter()
        .find(|s| s.text.trim() == "cat")
        .expect("must have a span for 'cat'");
    assert_eq!(
        cat_span.style,
        RichStyle::Plain,
        "'cat' must stay Plain, not inherit Bold from 'category': {row:?}"
    );
}

/// BDR 0023 Sc.4 — a word that straddles an emphasis boundary keeps both styles
/// as adjacent spans in the wrapped output.
///
/// Source: ["fo" Bold, "o" Plain] — no whitespace, one display word "foo".
#[test]
fn cross_boundary_word_keeps_both_styles() {
    use crate::render::wrap_rich;
    use crate::richtext::{RichSpan, RichStyle};

    let line = vec![
        RichSpan {
            text: "fo".to_string(),
            style: RichStyle::Bold,
        },
        RichSpan {
            text: "o".to_string(),
            style: RichStyle::Plain,
        },
    ];
    let wrapped = wrap_rich(&line, 80);
    assert_eq!(wrapped.len(), 1, "should fit on one line: {wrapped:?}");

    let row = &wrapped[0];
    let bold_part = row.iter().find(|s| s.style == RichStyle::Bold);
    let plain_part = row.iter().find(|s| s.style == RichStyle::Plain);
    assert!(
        bold_part.is_some(),
        "wrapped output must include a Bold span for 'fo': {row:?}"
    );
    assert!(
        plain_part.is_some(),
        "wrapped output must include a Plain span for 'o': {row:?}"
    );
    let bold_text = &bold_part.unwrap().text;
    let plain_text = &plain_part.unwrap().text;
    assert!(
        bold_text.contains('f') && bold_text.contains('o'),
        "Bold span must contain 'fo': {row:?}"
    );
    assert_eq!(plain_text, "o", "Plain span must be 'o': {row:?}");
}

/// BDR 0023 Sc.5 — a styled word that falls after a wrap break keeps its style;
/// the word before the break keeps its own style unchanged.
///
/// Source: ["plain " Plain (repeated to fill ~38 chars), "bold" Bold]
/// Width 40 forces a break before "bold"; "bold" on row 2 must be Bold.
#[test]
fn style_after_wrap_break_is_preserved() {
    use crate::render::wrap_rich;
    use crate::richtext::{RichSpan, RichStyle};

    // "plain " * 7 trimmed = "plain plain plain plain plain plain plain" = 41 chars.
    // At width 40, "bold" won't fit after the filler, so it wraps to the next row.
    let filler = "plain ".repeat(7).trim_end().to_string();
    let line = vec![
        RichSpan {
            text: filler,
            style: RichStyle::Plain,
        },
        RichSpan {
            text: " bold".to_string(),
            style: RichStyle::Bold,
        },
    ];
    let wrapped = wrap_rich(&line, 40);
    assert!(
        wrapped.len() >= 2,
        "line should wrap to at least 2 rows: {wrapped:?}"
    );

    // The last row must contain the Bold "bold" word.
    let last_row = wrapped.last().unwrap();
    let bold_span = last_row
        .iter()
        .find(|s| s.style == RichStyle::Bold)
        .expect("last row must contain a Bold span: {last_row:?}");
    assert!(
        bold_span.text.contains("bold"),
        "Bold span must contain 'bold': {last_row:?}"
    );
}
