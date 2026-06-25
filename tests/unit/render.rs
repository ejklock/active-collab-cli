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

fn full_task() -> serde_json::Value {
    json!({
        "id": 99,
        "task_number": 7,
        "project_id": 10,
        "project_name": "Acme Project",
        "name": "Fix the bug",
        "is_completed": false,
        "assignee_id": 5,
        "start_on": 1614556800i64,
        "due_on": 1614643200i64,
        "estimate": 4.0f64,
        "tracked_time": 2.5f64,
        "body": "<p>Some details here</p>"
    })
}

#[test]
fn render_detail_lines_meta_rows_present() {
    let task = full_task();
    let mut user_map = HashMap::new();
    user_map.insert(5i64, "Alice".to_string());
    let lines = render_detail_lines(&task, &[], &[], &user_map);
    let joined = lines.join("\n");
    assert!(joined.contains("Task:"), "missing Task row: {joined}");
    assert!(joined.contains("10-99"), "missing task ref: {joined}");
    assert!(joined.contains("Project:"), "missing Project row: {joined}");
    assert!(
        joined.contains("Acme Project"),
        "missing project name: {joined}"
    );
    assert!(joined.contains("Title:"), "missing Title row: {joined}");
    assert!(joined.contains("Fix the bug"), "missing title: {joined}");
    assert!(joined.contains("Status:"), "missing Status row: {joined}");
    assert!(joined.contains("Open"), "missing status: {joined}");
    assert!(
        joined.contains("Assignee:"),
        "missing Assignee row: {joined}"
    );
    assert!(
        joined.contains("Alice (5)"),
        "missing assignee name: {joined}"
    );
    assert!(joined.contains("Start:"), "missing Start row: {joined}");
    assert!(joined.contains("Due:"), "missing Due row: {joined}");
    assert!(
        joined.contains("Estimate:"),
        "missing Estimate row: {joined}"
    );
    assert!(joined.contains("4h"), "missing estimate value: {joined}");
    assert!(joined.contains("Logged:"), "missing Logged row: {joined}");
    assert!(joined.contains("2.5h"), "missing logged value: {joined}");
}

#[test]
fn render_detail_lines_description_from_body_html() {
    let task = full_task();
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("Description:"),
        "missing Description header: {joined}"
    );
    assert!(
        joined.contains("Some details here"),
        "missing description body: {joined}"
    );
}

#[test]
fn render_detail_lines_no_description_fallback() {
    let task = json!({ "id": 1, "body": null });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("(no description)"),
        "missing fallback: {joined}"
    );
}

#[test]
fn render_detail_lines_empty_body_falls_back() {
    let task = json!({ "id": 1, "body": "" });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("(no description)"),
        "empty body must fall back: {joined}"
    );
}

#[test]
fn render_detail_lines_artifacts_section_with_assets() {
    let task = json!({ "id": 1 });
    let assets = vec![
        Asset {
            name: "doc.pdf".into(),
            url: "https://example.com/doc.pdf".into(),
        },
        Asset {
            name: "image.png".into(),
            url: "https://example.com/image.png".into(),
        },
    ];
    let lines = render_detail_lines(&task, &[], &assets, &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("Artifacts:"),
        "missing Artifacts header: {joined}"
    );
    assert!(
        joined.contains("[1] doc.pdf"),
        "missing first asset name: {joined}"
    );
    assert!(
        joined.contains("  https://example.com/doc.pdf"),
        "missing first url: {joined}"
    );
    assert!(
        joined.contains("[2] image.png"),
        "missing second asset name: {joined}"
    );
}

#[test]
fn render_detail_lines_no_artifacts_section_when_empty() {
    let task = json!({ "id": 1 });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Artifacts:"),
        "must omit Artifacts when empty: {joined}"
    );
}

#[test]
fn render_detail_lines_comments_section() {
    let comments = vec![json!({
        "created_by_name": "Bob",
        "created_on": 1614556800i64,
        "body_plain_text": "LGTM!"
    })];
    let task = json!({ "id": 1 });
    let lines = render_detail_lines(&task, &comments, &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("Comments:"),
        "missing Comments header: {joined}"
    );
    assert!(joined.contains("Bob"), "missing comment author: {joined}");
    assert!(joined.contains("LGTM!"), "missing comment body: {joined}");
}

#[test]
fn render_detail_lines_no_comments_section_when_empty() {
    let task = json!({ "id": 1 });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Comments:"),
        "must omit Comments when empty: {joined}"
    );
}

#[test]
fn render_detail_lines_unassigned_fallback() {
    let task = json!({ "id": 1, "assignee_id": null });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("(unassigned)"),
        "missing unassigned fallback: {joined}"
    );
}

#[test]
fn render_detail_lines_assignee_id_not_in_map() {
    let task = json!({ "id": 1, "assignee_id": 77 });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("(77)"),
        "missing bare id fallback: {joined}"
    );
}

#[test]
fn render_detail_lines_start_due_omitted_when_null() {
    let task = json!({ "id": 1, "start_on": null, "due_on": null });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Start:"),
        "must omit Start when null: {joined}"
    );
    assert!(
        !joined.contains("Due:"),
        "must omit Due when null: {joined}"
    );
}

#[test]
fn render_detail_lines_comment_falls_back_to_html_to_text() {
    let comments = vec![json!({
        "created_by_name": "Alice",
        "created_on": 1614556800i64,
        "body": "<p>Paragraph comment</p>"
    })];
    let task = json!({ "id": 1 });
    let lines = render_detail_lines(&task, &comments, &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("Paragraph comment"),
        "html body must be converted: {joined}"
    );
}

#[test]
fn render_detail_lines_comment_unknown_author_fallback() {
    let comments = vec![json!({
        "created_on": 1614556800i64,
        "body_plain_text": "hello"
    })];
    let task = json!({ "id": 1 });
    let lines = render_detail_lines(&task, &comments, &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("(unknown)"),
        "missing unknown author fallback: {joined}"
    );
}

#[test]
fn render_detail_lines_completed_status() {
    let task = json!({ "id": 1, "is_completed": true });
    let lines = render_detail_lines(&task, &[], &[], &HashMap::new());
    let joined = lines.join("\n");
    assert!(
        joined.contains("Completed"),
        "missing Completed status: {joined}"
    );
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
