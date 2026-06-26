use super::*;
use serde_json::json;

#[test]
fn is_openable_url_accepts_http() {
    assert!(is_openable_url("http://example.com/file.pdf"));
}

#[test]
fn is_openable_url_accepts_https() {
    assert!(is_openable_url("https://example.com/path/to/image.png"));
}

#[test]
fn is_openable_url_rejects_empty_string() {
    assert!(!is_openable_url(""));
}

#[test]
fn is_openable_url_rejects_file_scheme() {
    assert!(!is_openable_url("file:///etc/passwd"));
}

#[test]
fn is_openable_url_rejects_javascript_scheme() {
    assert!(!is_openable_url("javascript:alert(1)"));
}

#[test]
fn is_openable_url_rejects_data_scheme() {
    assert!(!is_openable_url("data:text/html,<script>alert(1)</script>"));
}

#[test]
fn is_openable_url_rejects_mailto_scheme() {
    assert!(!is_openable_url("mailto:user@example.com"));
}

#[test]
fn is_openable_url_rejects_relative_path() {
    assert!(!is_openable_url("/relative/path"));
}

#[test]
fn is_openable_url_rejects_bare_path_no_scheme() {
    assert!(!is_openable_url("just-a-word"));
}

#[test]
fn html_to_text_empty_string_returns_empty() {
    assert_eq!(html_to_text(""), "");
}

#[test]
fn html_to_text_block_tags_become_newlines() {
    // Block tags become \n; leading/trailing \n are trimmed by the final strip
    assert_eq!(html_to_text("<p>hello</p>"), "hello");
    assert_eq!(html_to_text("<br>world"), "world");
    assert_eq!(html_to_text("<div>a</div><div>b</div>"), "a\nb");
    assert_eq!(html_to_text("<li>item</li>"), "item");
    assert_eq!(html_to_text("<tr>row</tr>"), "row");
    assert_eq!(html_to_text("<h1>Title</h1>"), "Title");
    assert_eq!(html_to_text("<h6>tiny</h6>"), "tiny");
    // Content before a block tag preserves the newline separator
    assert_eq!(html_to_text("intro<br>world"), "intro\nworld");
}

#[test]
fn html_to_text_strips_remaining_tags() {
    assert_eq!(html_to_text("<span>text</span>"), "text");
    assert_eq!(html_to_text("<strong>bold</strong>"), "bold");
    assert_eq!(html_to_text("<a href=\"x\">link</a>"), "link");
}

#[test]
fn html_to_text_decodes_named_entities() {
    assert_eq!(html_to_text("&amp;"), "&");
    assert_eq!(html_to_text("&lt;"), "<");
    assert_eq!(html_to_text("&gt;"), ">");
    assert_eq!(html_to_text("&quot;"), "\"");
    assert_eq!(html_to_text("&apos;"), "'");
    assert_eq!(html_to_text("&nbsp;"), "\u{00a0}");
}

#[test]
fn html_to_text_decodes_numeric_entities() {
    assert_eq!(html_to_text("&#65;"), "A");
    assert_eq!(html_to_text("&#x41;"), "A");
}

#[test]
fn html_to_text_collapses_3_or_more_newlines_to_2() {
    let input = "a\n\n\n\nb";
    assert_eq!(html_to_text(input), "a\n\nb");
    let input2 = "x\n\n\ny";
    assert_eq!(html_to_text(input2), "x\n\ny");
}

#[test]
fn html_to_text_trims_surrounding_whitespace() {
    assert_eq!(html_to_text("  hello  "), "hello");
    assert_eq!(html_to_text("\nhello\n"), "hello");
}

#[test]
fn html_to_text_block_tag_case_insensitive() {
    // Leading \n from block tag is trimmed
    assert_eq!(html_to_text("<BR>test"), "test");
    assert_eq!(html_to_text("<P>para</P>"), "para");
    // When there's content before, the \n is preserved as a separator
    assert_eq!(html_to_text("x<BR>test"), "x\ntest");
}

#[test]
fn html_to_text_block_tag_with_attributes() {
    assert_eq!(html_to_text("<p class=\"x\">text</p>"), "text");
    assert_eq!(html_to_text("<div id=\"main\">body</div>"), "body");
}

#[test]
fn fmt_ts_null_returns_empty() {
    assert_eq!(fmt_ts(&json!(null)), "");
}

#[test]
fn fmt_ts_integer_unix_seconds_utc() {
    // 2021-03-01 00:00:00 UTC = 1614556800
    assert_eq!(fmt_ts(&json!(1614556800i64)), "2021-03-01 00:00");
}

#[test]
fn fmt_ts_float_unix_seconds_utc() {
    assert_eq!(fmt_ts(&json!(1614556800.5f64)), "2021-03-01 00:00");
}

#[test]
fn fmt_ts_zero_seconds() {
    assert_eq!(fmt_ts(&json!(0i64)), "1970-01-01 00:00");
}

#[test]
fn fmt_ts_string_returns_as_is() {
    assert_eq!(fmt_ts(&json!("2024-01-15")), "2024-01-15");
}

#[test]
fn fmt_date_null_returns_empty() {
    assert_eq!(fmt_date(&json!(null)), "");
}

#[test]
fn fmt_date_integer_unix_seconds_utc() {
    assert_eq!(fmt_date(&json!(1614556800i64)), "2021-03-01");
}

#[test]
fn fmt_date_float_unix_seconds_utc() {
    assert_eq!(fmt_date(&json!(1614556800.5f64)), "2021-03-01");
}

#[test]
fn fmt_date_string_returns_as_is() {
    assert_eq!(fmt_date(&json!("2024-06-15")), "2024-06-15");
}

#[test]
fn fmt_hours_null_returns_zero_string() {
    assert_eq!(fmt_hours(&json!(null)), "0");
}

#[test]
fn fmt_hours_whole_integer_returns_integer_string() {
    assert_eq!(fmt_hours(&json!(4i64)), "4");
    assert_eq!(fmt_hours(&json!(0i64)), "0");
}

#[test]
fn fmt_hours_whole_float_returns_integer_string() {
    assert_eq!(fmt_hours(&json!(4.0f64)), "4");
}

#[test]
fn fmt_hours_fractional_returns_fractional_string() {
    assert_eq!(fmt_hours(&json!(3.5f64)), "3.5");
    assert_eq!(fmt_hours(&json!(0.25f64)), "0.25");
}

#[test]
fn fmt_hours_string_numeric_whole_returns_integer() {
    assert_eq!(fmt_hours(&json!("8")), "8");
}

#[test]
fn fmt_hours_string_fractional_returns_fractional() {
    assert_eq!(fmt_hours(&json!("2.5")), "2.5");
}

#[test]
fn fmt_hours_non_numeric_string_returns_as_is() {
    assert_eq!(fmt_hours(&json!("n/a")), "n/a");
}

#[test]
fn render_meta_unassigned_when_assignee_id_null() {
    let task = json!({ "assignee_id": null });
    let map = HashMap::new();
    let s = render_meta_to_str(&task, &map);
    assert!(s.contains("(unassigned)"), "got: {s}");
    assert!(s.contains("Assignee:"), "got: {s}");
}

#[test]
fn render_meta_assignee_with_name_in_map() {
    let task = json!({ "assignee_id": 5 });
    let mut map = HashMap::new();
    map.insert(5i64, "Alice".to_owned());
    let s = render_meta_to_str(&task, &map);
    assert!(s.contains("Alice (5)"), "got: {s}");
}

#[test]
fn render_meta_assignee_id_not_in_map() {
    let task = json!({ "assignee_id": 99 });
    let map = HashMap::new();
    let s = render_meta_to_str(&task, &map);
    assert!(s.contains("(99)"), "got: {s}");
}

#[test]
fn render_meta_start_and_due_present() {
    let task = json!({
        "start_on": 1614556800i64,
        "due_on": 1614643200i64
    });
    let map = HashMap::new();
    let s = render_meta_to_str(&task, &map);
    assert!(s.contains("Start:"), "got: {s}");
    assert!(s.contains("Due:"), "got: {s}");
    assert!(s.contains("2021-03-01"), "got: {s}");
    assert!(s.contains("2021-03-02"), "got: {s}");
}

#[test]
fn render_meta_start_omitted_when_null() {
    let task = json!({ "start_on": null, "due_on": null });
    let map = HashMap::new();
    let s = render_meta_to_str(&task, &map);
    assert!(!s.contains("Start:"), "should omit Start: got: {s}");
    assert!(!s.contains("Due:"), "should omit Due: got: {s}");
}

#[test]
fn render_meta_estimate_and_logged_always_present() {
    let task = json!({ "estimate": 8.0f64, "tracked_time": 3.5f64 });
    let map = HashMap::new();
    let s = render_meta_to_str(&task, &map);
    assert!(s.contains("Estimate:  8h"), "got: {s}");
    assert!(s.contains("Logged:    3.5h"), "got: {s}");
}

#[test]
fn render_meta_estimate_null_shows_zero() {
    let task = json!({ "estimate": null, "tracked_time": null });
    let map = HashMap::new();
    let s = render_meta_to_str(&task, &map);
    assert!(s.contains("Estimate:  0h"), "got: {s}");
    assert!(s.contains("Logged:    0h"), "got: {s}");
}

#[test]
fn render_comments_empty_returns_empty() {
    assert_eq!(render_comments_to_str(&[]), "");
}

#[test]
fn render_comments_single_comment_with_name() {
    let comments = vec![json!({
        "created_by_name": "Bob",
        "created_on": 1614556800i64,
        "body_plain_text": "Nice work!"
    })];
    let s = render_comments_to_str(&comments);
    assert!(s.contains("Comments (1):"), "got: {s}");
    assert!(s.contains("[1] Bob"), "got: {s}");
    assert!(s.contains("2021-03-01 00:00"), "got: {s}");
    assert!(s.contains("Nice work!"), "got: {s}");
}

#[test]
fn render_comments_uses_created_by_id_when_no_name() {
    let comments = vec![json!({
        "created_by_id": 42,
        "created_on": 1614556800i64,
        "body": "<p>Hello</p>"
    })];
    let s = render_comments_to_str(&comments);
    assert!(s.contains("[1] 42"), "got: {s}");
}

#[test]
fn render_comments_falls_back_to_unknown() {
    let comments = vec![json!({
        "created_on": 1614556800i64,
        "body": "text"
    })];
    let s = render_comments_to_str(&comments);
    assert!(s.contains("(unknown)"), "got: {s}");
}

#[test]
fn render_comments_uses_html_to_text_when_no_plain_text() {
    let comments = vec![json!({
        "created_by_name": "Eve",
        "created_on": 1614556800i64,
        "body": "<p>Paragraph text</p>"
    })];
    let s = render_comments_to_str(&comments);
    assert!(s.contains("Paragraph text"), "got: {s}");
}

#[test]
fn render_comments_multiple_comments_numbered() {
    let comments = vec![
        json!({"created_by_name": "Alice", "created_on": 1614556800i64, "body_plain_text": "First"}),
        json!({"created_by_name": "Bob",   "created_on": 1614556801i64, "body_plain_text": "Second"}),
    ];
    let s = render_comments_to_str(&comments);
    assert!(s.contains("[1] Alice"), "got: {s}");
    assert!(s.contains("[2] Bob"), "got: {s}");
    assert!(s.contains("Comments (2):"), "got: {s}");
}

#[test]
fn render_task_to_str_completed_status() {
    let task = json!({ "id": 1, "name": "A task", "is_completed": true });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("Completed"), "got: {s}");
}

#[test]
fn render_task_to_str_open_status() {
    let task = json!({ "id": 1, "name": "A task", "is_completed": false });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("Open"), "got: {s}");
}

#[test]
fn render_task_to_str_uses_task_number_when_present() {
    let task = json!({ "id": 99, "task_number": 42, "name": "X" });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("Task:      42"), "got: {s}");
}

#[test]
fn render_task_to_str_falls_back_to_id_when_no_task_number() {
    let task = json!({ "id": 99, "task_number": null, "name": "X" });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("Task:      99"), "got: {s}");
}

#[test]
fn render_task_to_str_no_description_fallback() {
    let task = json!({ "id": 1, "name": "X", "body": null });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("(no description)"), "got: {s}");
}

#[test]
fn render_task_to_str_description_from_body() {
    let task = json!({ "id": 1, "name": "X", "body": "<p>Some details</p>" });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("Some details"), "got: {s}");
}

#[test]
fn render_task_to_str_exact_label_spacing() {
    let task = json!({ "id": 1, "task_number": 5, "name": "My Task" });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(s.contains("Task:      5"), "Task label spacing wrong: {s}");
    assert!(
        s.contains("Name:      My Task"),
        "Name label spacing wrong: {s}"
    );
    assert!(s.contains("Status:    "), "Status label spacing wrong: {s}");
    assert!(s.contains("Description:"), "Description label missing: {s}");
}

#[test]
fn render_task_to_str_no_comments_flag_suppresses_section() {
    let task = json!({ "id": 1, "name": "X" });
    let comments = vec![
        json!({"created_by_name": "Alice", "created_on": 1614556800i64, "body_plain_text": "hi"}),
    ];
    let with_comments = render_task_to_str(&task, &comments, false, &HashMap::new());
    let without_comments = render_task_to_str(&task, &comments, true, &HashMap::new());
    assert!(
        with_comments.contains("Comments"),
        "expected comments: {with_comments}"
    );
    assert!(
        !without_comments.contains("Comments"),
        "expected no comments: {without_comments}"
    );
}

#[test]
fn render_task_to_str_empty_comments_omits_section() {
    let task = json!({ "id": 1, "name": "X" });
    let s = render_task_to_str(&task, &[], false, &HashMap::new());
    assert!(!s.contains("Comments"), "got: {s}");
}

#[test]
fn print_error_does_not_panic_on_empty_string() {
    print_error("");
}

#[test]
fn print_error_does_not_panic_on_normal_message() {
    print_error("Error: something went wrong");
}

#[test]
fn render_mine_table_header_contains_expected_labels() {
    let s = render_mine_table(&[]);
    let first_line = s.lines().next().unwrap();
    assert!(
        first_line.contains("INSTANCE"),
        "header missing INSTANCE: {first_line}"
    );
    assert!(
        first_line.contains("PROJECT"),
        "header missing PROJECT: {first_line}"
    );
    assert!(
        first_line.contains("TASK#"),
        "header missing TASK#: {first_line}"
    );
    assert!(
        first_line.contains("TASK_ID"),
        "header missing TASK_ID: {first_line}"
    );
    assert!(
        first_line.contains("NAME"),
        "header missing NAME: {first_line}"
    );
}

#[test]
fn render_mine_table_header_column_widths_match_python() {
    let s = render_mine_table(&[]);
    let first_line = s.lines().next().unwrap();
    // INSTANCE is left-padded to 15, then a space, then PROJECT at 10
    assert!(
        first_line.starts_with("INSTANCE        "),
        "INSTANCE col width wrong: {first_line}"
    );
    // PROJECT starts at offset 16 (0-based: positions 16..25)
    let project_part = &first_line[16..];
    assert!(
        project_part.starts_with("PROJECT   "),
        "PROJECT col width wrong: {project_part}"
    );
}

#[test]
fn render_mine_table_separator_is_exactly_80_dashes() {
    let s = render_mine_table(&[]);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(
        lines[1],
        "-".repeat(80),
        "separator must be exactly 80 dashes"
    );
}

#[test]
fn render_mine_table_row_left_aligns_integers() {
    let rows = [MineTableRow {
        instance: "myinst".to_owned(),
        project_id: 42,
        task_number: 7,
        task_id: 100,
        name: "A task".to_owned(),
    }];
    let s = render_mine_table(&rows);
    let data_line = s.lines().nth(2).unwrap();
    // project_id 42 is left-aligned in a 10-wide column: "42        "
    assert!(
        data_line.contains("42        "),
        "project_id not left-aligned: {data_line}"
    );
    // task_number 7 is left-aligned in an 8-wide column: "7       "
    assert!(
        data_line.contains("7        "),
        "task_number not left-aligned: {data_line}"
    );
}

#[test]
fn render_mine_table_multi_row_body_order_preserved() {
    let rows = [
        MineTableRow {
            instance: "alpha".to_owned(),
            project_id: 1,
            task_number: 10,
            task_id: 1001,
            name: "First".to_owned(),
        },
        MineTableRow {
            instance: "beta".to_owned(),
            project_id: 2,
            task_number: 20,
            task_id: 2002,
            name: "Second".to_owned(),
        },
    ];
    let s = render_mine_table(&rows);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 4, "header + separator + 2 rows = 4 lines");
    assert!(lines[2].contains("alpha"), "first row: {}", lines[2]);
    assert!(lines[2].contains("First"), "first row name: {}", lines[2]);
    assert!(lines[3].contains("beta"), "second row: {}", lines[3]);
    assert!(lines[3].contains("Second"), "second row name: {}", lines[3]);
}

#[test]
fn render_mine_table_empty_rows_gives_two_lines() {
    let s = render_mine_table(&[]);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 2, "header + separator only: {s}");
}

#[test]
fn extract_assets_from_body_html() {
    let task = json!({
        "id": 1,
        "body": r#"<img src="https://example.com/img.png"><a href="https://example.com/file.pdf">link</a>"#
    });
    let assets = extract_assets(&task, &[]);
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0].name, "img.png");
    assert_eq!(assets[0].url, "https://example.com/img.png");
    assert_eq!(assets[1].name, "file.pdf");
}

#[test]
fn extract_assets_deduplicates_by_url() {
    let task = json!({
        "id": 1,
        "body": r#"<img src="https://example.com/img.png">"#
    });
    let comments = vec![json!({
        "body": r#"<img src="https://example.com/img.png">"#
    })];
    let assets = extract_assets(&task, &comments);
    assert_eq!(assets.len(), 1, "duplicate URLs must be deduplicated");
}

#[test]
fn extract_assets_from_attachments() {
    let task = json!({
        "id": 1,
        "attachments": [
            { "name": "report.pdf", "url": "https://example.com/report.pdf" }
        ]
    });
    let assets = extract_assets(&task, &[]);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].name, "report.pdf");
}

#[test]
fn extract_assets_empty_when_no_body_or_attachments() {
    let task = json!({ "id": 1 });
    let assets = extract_assets(&task, &[]);
    assert!(assets.is_empty());
}

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

fn char_width(s: &str) -> usize {
    s.chars().count()
}

#[test]
fn comment_box_returns_empty_when_width_less_than_4() {
    assert!(comment_box("Alice", "2024-01-01 10:00", "body", 3).is_empty());
    assert!(comment_box("Alice", "2024-01-01 10:00", "body", 0).is_empty());
}

#[test]
fn comment_box_short_comment_every_line_is_exactly_width_chars() {
    let width = 40;
    let lines = comment_box("Alice", "2024-01-01 10:00", "Nice work!", width);
    assert!(
        lines.len() >= 3,
        "need at least top+body+bottom: {:?}",
        lines
    );
    for line in &lines {
        assert_eq!(
            char_width(line),
            width,
            "line must be exactly {width} chars wide: {:?}",
            line
        );
    }
}

#[test]
fn comment_box_top_border_contains_author_and_when_with_rounded_corners() {
    let lines = comment_box("Alice", "2024-01-01 10:00", "body", 60);
    let top = &lines[0];
    assert!(
        top.starts_with('\u{256D}'),
        "top must start with corner: {:?}",
        top
    );
    assert!(
        top.ends_with('\u{256E}'),
        "top must end with corner: {:?}",
        top
    );
    assert!(top.contains("Alice"), "top must contain author: {:?}", top);
    assert!(
        top.contains("2024-01-01 10:00"),
        "top must contain when: {:?}",
        top
    );
    assert!(
        top.contains('\u{00B7}'),
        "top must contain middot: {:?}",
        top
    );
}

#[test]
fn comment_box_bottom_border_has_rounded_corners() {
    let lines = comment_box("Alice", "2024-01-01", "body", 40);
    let bottom = lines.last().unwrap();
    assert!(
        bottom.starts_with('\u{2570}'),
        "bottom must start with bl: {:?}",
        bottom
    );
    assert!(
        bottom.ends_with('\u{256F}'),
        "bottom must end with br: {:?}",
        bottom
    );
}

#[test]
fn comment_box_body_lines_start_and_end_with_v_char() {
    let lines = comment_box("Alice", "2024-01-01", "body text here", 30);
    for middle_line in &lines[1..lines.len() - 1] {
        assert!(
            middle_line.starts_with('\u{2502}'),
            "middle line must start with v: {:?}",
            middle_line
        );
        assert!(
            middle_line.ends_with('\u{2502}'),
            "middle line must end with v: {:?}",
            middle_line
        );
    }
}

#[test]
fn comment_box_long_header_is_clipped_with_ellipsis_before_corner() {
    let width = 20;
    let lines = comment_box("VeryLongAuthorName", "2024-01-01 10:00:00", "body", width);
    let top = &lines[0];
    assert_eq!(
        char_width(top),
        width,
        "top must be exactly {width} chars: {:?}",
        top
    );
    assert!(
        top.ends_with('\u{256E}'),
        "top must end with corner: {:?}",
        top
    );
    let before_corner: String = top
        .chars()
        .rev()
        .skip(1)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    assert!(
        before_corner.ends_with('\u{2026}')
            || before_corner.ends_with('\u{2026}')
            || top.contains('\u{2026}'),
        "clipped header must contain ellipsis: {:?}",
        top
    );
}

#[test]
fn comment_box_multiline_body_each_middle_line_exactly_width() {
    let body = "This is a longer body text that should wrap across multiple lines in the box";
    let width = 30;
    let lines = comment_box("Author", "2024-01-01", body, width);
    assert!(
        lines.len() > 3,
        "must have more than 3 lines for wrapped body: {:?}",
        lines
    );
    for line in &lines {
        assert_eq!(
            char_width(line),
            width,
            "every line must be exactly {width} chars: {:?}",
            line
        );
    }
}

#[test]
fn build_detail_lines_meta_section_present() {
    let task = json!({
        "id": 99,
        "project_id": 10,
        "project_name": "Acme",
        "name": "Fix bug",
        "is_completed": false,
        "assignee_id": 5,
        "estimate": 4.0f64,
        "tracked_time": 2.5f64
    });
    let mut user_map = HashMap::new();
    user_map.insert(5i64, "Alice".to_string());
    let lines = build_detail_lines(&task, &[], &user_map, 80);
    let joined = lines.join("\n");
    assert!(joined.contains("Task:"), "missing Task: {joined}");
    assert!(joined.contains("10-99"), "missing task ref: {joined}");
    assert!(joined.contains("Project:"), "missing Project: {joined}");
    assert!(joined.contains("Acme"), "missing project name: {joined}");
    assert!(joined.contains("Title:"), "missing Title: {joined}");
    assert!(joined.contains("Fix bug"), "missing title: {joined}");
    assert!(joined.contains("Status:"), "missing Status: {joined}");
    assert!(joined.contains("Open"), "missing status: {joined}");
    assert!(joined.contains("Assignee:"), "missing Assignee: {joined}");
    assert!(joined.contains("Alice (5)"), "missing assignee: {joined}");
}

#[test]
fn build_detail_lines_description_present() {
    let task = json!({ "id": 1, "body": "<p>Some details here</p>" });
    let lines = build_detail_lines(&task, &[], &HashMap::new(), 80);
    let joined = lines.join("\n");
    assert!(
        joined.contains("Description:"),
        "missing Description: {joined}"
    );
    assert!(
        joined.contains("Some details here"),
        "missing body text: {joined}"
    );
}

#[test]
fn build_detail_lines_no_description_fallback() {
    let task = json!({ "id": 1, "body": null });
    let lines = build_detail_lines(&task, &[], &HashMap::new(), 80);
    let joined = lines.join("\n");
    assert!(
        joined.contains("(no description)"),
        "missing fallback: {joined}"
    );
}

#[test]
fn build_detail_lines_no_comment_block_when_empty() {
    let task = json!({ "id": 1 });
    let lines = build_detail_lines(&task, &[], &HashMap::new(), 80);
    let joined = lines.join("\n");
    assert!(
        !joined.contains('\u{256D}'),
        "must not have box corners when no comments: {joined}"
    );
}

#[test]
fn build_detail_lines_comment_boxes_present() {
    let task = json!({ "id": 1 });
    let comments = vec![json!({
        "created_by_name": "Bob",
        "created_on": 1614556800i64,
        "body_plain_text": "LGTM!"
    })];
    let lines = build_detail_lines(&task, &comments, &HashMap::new(), 60);
    let joined = lines.join("\n");
    assert!(
        joined.contains('\u{256D}'),
        "must have box top-left corner: {joined}"
    );
    assert!(
        joined.contains("Bob"),
        "must contain comment author: {joined}"
    );
    assert!(
        joined.contains("LGTM!"),
        "must contain comment body: {joined}"
    );
}

#[test]
fn build_detail_lines_no_line_exceeds_inner_width() {
    let task = json!({
        "id": 99,
        "project_id": 10,
        "project_name": "A Very Long Project Name That Could Overflow The Line Width",
        "name": "A task with an extremely verbose name that also goes long",
        "body": "<p>Body text that is quite verbose and goes on for a while to test wrapping behavior</p>"
    });
    let comments = vec![json!({
        "created_by_name": "Alice Wonderland",
        "created_on": 1614556800i64,
        "body_plain_text": "This is a fairly long comment body that should be word-wrapped to fit within the box"
    })];
    let inner_width = 50;
    let lines = build_detail_lines(&task, &comments, &HashMap::new(), inner_width);
    for line in &lines {
        let len = line.chars().count();
        assert!(
            len <= inner_width,
            "line exceeds {inner_width} chars ({len}): {:?}",
            line
        );
    }
}

#[test]
fn truncate_cell_shorter_than_width_returns_unchanged() {
    assert_eq!(truncate_cell("hello", 10), "hello");
}

#[test]
fn truncate_cell_equal_to_width_returns_unchanged() {
    assert_eq!(truncate_cell("hello", 5), "hello");
}

#[test]
fn truncate_cell_longer_than_width_ends_with_ellipsis_and_has_exact_char_count() {
    let result = truncate_cell("abcdefghij", 6);
    assert_eq!(result.chars().count(), 6, "result must be exactly 6 chars");
    assert!(
        result.ends_with('\u{2026}'),
        "result must end with ellipsis: {result:?}"
    );
    assert_eq!(
        &result[..result.len() - '\u{2026}'.len_utf8()],
        "abcde",
        "first max_width-1 chars must be preserved"
    );
}

#[test]
fn truncate_cell_max_width_zero_returns_empty() {
    assert_eq!(truncate_cell("anything", 0), "");
}

#[test]
fn truncate_cell_max_width_one_returns_only_ellipsis() {
    let result = truncate_cell("abc", 1);
    assert_eq!(result, "\u{2026}");
    assert_eq!(result.chars().count(), 1);
}

#[test]
fn truncate_cell_multibyte_truncates_on_char_boundary_without_panic() {
    let s = "Ação longa demais para caber aqui no campo";
    let max_width = 10;
    let result = truncate_cell(s, max_width);
    assert_eq!(
        result.chars().count(),
        max_width,
        "multibyte result must be exactly {max_width} chars"
    );
    assert!(
        result.ends_with('\u{2026}'),
        "multibyte result must end with ellipsis: {result:?}"
    );
}

#[test]
fn truncate_cell_cjk_truncates_on_char_boundary_without_panic() {
    let s = "日本語テスト文字列が長すぎる場合";
    let max_width = 5;
    let result = truncate_cell(s, max_width);
    assert_eq!(
        result.chars().count(),
        max_width,
        "CJK result must be exactly {max_width} chars"
    );
    assert!(
        result.ends_with('\u{2026}'),
        "CJK result must end with ellipsis: {result:?}"
    );
}

#[test]
fn build_detail_lines_multiple_comments_separated_by_blank() {
    let task = json!({ "id": 1 });
    let comments = vec![
        json!({ "created_by_name": "Alice", "created_on": 1614556800i64, "body_plain_text": "First" }),
        json!({ "created_by_name": "Bob", "created_on": 1614556801i64, "body_plain_text": "Second" }),
    ];
    let lines = build_detail_lines(&task, &comments, &HashMap::new(), 50);
    let joined = lines.join("\n");
    assert!(
        joined.contains("Alice"),
        "must contain first author: {joined}"
    );
    assert!(
        joined.contains("Bob"),
        "must contain second author: {joined}"
    );
    assert!(
        joined.contains("First"),
        "must contain first body: {joined}"
    );
    assert!(
        joined.contains("Second"),
        "must contain second body: {joined}"
    );
    let top_corners: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with('\u{256D}'))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        top_corners.len(),
        2,
        "must have 2 comment boxes: lines={:?}",
        lines
    );
    let gap = top_corners[1] - top_corners[0];
    assert!(
        gap > 3,
        "boxes must be separated by at least the first box bottom + blank: gap={gap}"
    );
}
