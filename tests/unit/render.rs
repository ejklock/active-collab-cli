use super::*;
use serde_json::json;
use unicode_width::UnicodeWidthStr;

// --- D3: link_segments ---

#[test]
fn link_segments_no_url_returns_single_non_link_segment() {
    let segs = link_segments("plain text with no URL");
    assert_eq!(segs.len(), 1);
    assert!(!segs[0].is_link);
    assert_eq!(segs[0].text, "plain text with no URL");
}

#[test]
fn link_segments_https_url_splits_into_three_ordered_segments() {
    let line = "See https://example.com/path for details";
    let segs = link_segments(line);
    assert_eq!(segs.len(), 3, "expected [before][url][after]: {segs:?}");
    assert!(!segs[0].is_link, "prefix must be non-link: {segs:?}");
    assert!(segs[1].is_link, "url must be link: {segs:?}");
    assert!(!segs[2].is_link, "suffix must be non-link: {segs:?}");
    assert_eq!(segs[0].text, "See ");
    assert_eq!(segs[1].text, "https://example.com/path");
    assert_eq!(segs[2].text, " for details");
}

#[test]
fn link_segments_http_url_is_detected() {
    let line = "Visit http://example.com now";
    let segs = link_segments(line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(link_segs.len(), 1, "expected one link segment: {segs:?}");
    assert_eq!(link_segs[0].text, "http://example.com");
}

#[test]
fn link_segments_bare_www_url_is_detected() {
    let line = "Go to www.example.com for info";
    let segs = link_segments(line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(
        link_segs.len(),
        1,
        "bare www. URL must be detected: {segs:?}"
    );
    assert_eq!(link_segs[0].text, "www.example.com");
}

#[test]
fn link_segments_border_chars_and_padding_are_not_links() {
    // Simulate a panel body line: │ {hpad}content with https://url.com{hpad} │
    let line = "\u{2502} some text https://url.com/path more \u{2502}";
    let segs = link_segments(line);
    // The leading │ and spaces must be non-link
    assert!(
        !segs[0].is_link,
        "leading border+padding must be non-link: {:?}",
        segs[0]
    );
    assert!(
        segs[0].text.starts_with('\u{2502}'),
        "first segment must start with │"
    );
    // The trailing │ must be non-link
    let last = segs.last().unwrap();
    assert!(
        !last.is_link,
        "trailing border must be non-link: {:?}",
        last
    );
    assert!(
        last.text.ends_with('\u{2502}'),
        "last segment must end with │"
    );
}

#[test]
fn link_segments_url_stops_at_whitespace_not_including_trailing_border() {
    // URL must end before the space before │, so │ is not included in the link text
    let line = "\u{2502} https://example.com/page \u{2502}";
    let segs = link_segments(line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(link_segs.len(), 1);
    let url_text = &link_segs[0].text;
    assert!(
        !url_text.contains('\u{2502}'),
        "URL must not include │ border: {url_text:?}"
    );
    assert!(
        !url_text.ends_with(' '),
        "URL must not include trailing space: {url_text:?}"
    );
}

#[test]
fn link_segments_accented_text_adjacent_to_url_does_not_panic() {
    // Char-boundary safety: UTF-8 multibyte chars before and after a URL must not cause panic
    let line = "Ação: https://example.com/ação see também";
    let segs = link_segments(line);
    let all_text: String = segs.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        all_text, line,
        "segments must reconstruct original line: {segs:?}"
    );
}

#[test]
fn link_segments_empty_line_returns_single_non_link_segment() {
    let segs = link_segments("");
    assert_eq!(segs.len(), 1);
    assert!(!segs[0].is_link);
    assert_eq!(segs[0].text, "");
}

#[test]
fn link_segments_multiple_urls_all_tagged() {
    let line = "A https://first.com B https://second.org C";
    let segs = link_segments(line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(
        link_segs.len(),
        2,
        "two URLs must produce two link segments: {segs:?}"
    );
    assert_eq!(link_segs[0].text, "https://first.com");
    assert_eq!(link_segs[1].text, "https://second.org");
}

#[test]
fn link_segments_segments_reconstruct_original_line() {
    let line = "Before https://example.com/foo after";
    let segs = link_segments(line);
    let reconstructed: String = segs.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        reconstructed, line,
        "concatenated segments must equal original line"
    );
}

// --- V5-A1: link_segments handles bracketed [url] tokens ---

#[test]
fn link_segments_bracketed_url_inner_is_link_brackets_are_not() {
    let url = "https://example.com/path";
    let line = format!("text [{url}] more");
    let segs = link_segments(&line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(
        link_segs.len(),
        1,
        "bracketed URL must produce one link segment: {segs:?}"
    );
    assert_eq!(
        link_segs[0].text, url,
        "link segment must be the URL without brackets: {segs:?}"
    );
    let non_link_text: String = segs
        .iter()
        .filter(|s| !s.is_link)
        .map(|s| s.text.as_str())
        .collect();
    assert!(
        non_link_text.contains('['),
        "opening bracket must be in a non-link segment: {segs:?}"
    );
    assert!(
        non_link_text.contains(']'),
        "closing bracket must be in a non-link segment: {segs:?}"
    );
}

#[test]
fn link_segments_non_url_bracket_token_not_tagged_as_link() {
    let line = "see [note] for details";
    let segs = link_segments(line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert!(
        link_segs.is_empty(),
        "non-url '[note]' must NOT be tagged is_link: {segs:?}"
    );
}

#[test]
fn link_segments_bracketed_email_inner_is_link() {
    let line = "contact [user@example.com] for info";
    let segs = link_segments(line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(
        link_segs.len(),
        1,
        "bracketed email must be one link segment: {segs:?}"
    );
    assert_eq!(
        link_segs[0].text, "user@example.com",
        "link segment must be the email without brackets"
    );
}

#[test]
fn link_segments_bracketed_url_never_includes_trailing_bracket_in_link_span() {
    let url = "https://example.com/page";
    let line = format!("[{url}]");
    let segs = link_segments(&line);
    let link_segs: Vec<_> = segs.iter().filter(|s| s.is_link).collect();
    assert_eq!(link_segs.len(), 1);
    assert!(
        !link_segs[0].text.ends_with(']'),
        "link span must NEVER include a trailing ']': {:?}",
        link_segs[0].text
    );
    assert_eq!(link_segs[0].text, url);
}

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

fn char_width(s: &str) -> usize {
    s.chars().count()
}

fn dw(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

// --- U10: panel_box ---

#[test]
fn panel_box_returns_empty_when_width_less_than_4() {
    assert!(panel_box("label", &[], 3).is_empty());
    assert!(panel_box("label", &[], 0).is_empty());
    assert!(panel_box("x", &[String::new()], 1).is_empty());
}

#[test]
fn panel_box_top_border_starts_with_tl_and_ends_with_tr() {
    let lines = panel_box("My Label", &["body".into()], 30);
    assert!(!lines.is_empty(), "must produce lines");
    let top = &lines[0];
    assert!(
        top.starts_with('\u{256D}'),
        "top must start with BOX_TL ╭: {:?}",
        top
    );
    assert!(
        top.ends_with('\u{256E}'),
        "top must end with BOX_TR ╮: {:?}",
        top
    );
}

#[test]
fn panel_box_top_border_embeds_label() {
    let lines = panel_box("Details", &["row".into()], 40);
    let top = &lines[0];
    assert!(
        top.contains("Details"),
        "top border must contain the label: {:?}",
        top
    );
}

#[test]
fn panel_box_bottom_border_has_rounded_corners() {
    let lines = panel_box("L", &["x".into()], 20);
    let bottom = lines.last().unwrap();
    assert!(
        bottom.starts_with('\u{2570}'),
        "bottom must start with BOX_BL ╰: {:?}",
        bottom
    );
    assert!(
        bottom.ends_with('\u{256F}'),
        "bottom must end with BOX_BR ╯: {:?}",
        bottom
    );
}

#[test]
fn panel_box_body_lines_bounded_by_v_chars() {
    let inner_lines = vec!["hello".into(), "world".into()];
    let lines = panel_box("L", &inner_lines, 20);
    // Lines 1..(len-1) are body lines
    for line in &lines[1..lines.len() - 1] {
        assert!(
            line.starts_with('\u{2502}'),
            "body line must start with BOX_V │: {:?}",
            line
        );
        assert!(
            line.ends_with('\u{2502}'),
            "body line must end with BOX_V │: {:?}",
            line
        );
    }
}

#[test]
fn panel_box_every_line_exactly_width_chars() {
    let width = 40;
    let inner_lines: Vec<String> = vec!["short".into(), "a bit longer line here".into()];
    let lines = panel_box("Test Panel", &inner_lines, width);
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
fn panel_box_body_line_padded_to_inner_width() {
    let width = 20;
    let inner_lines = vec!["hi".into()];
    let lines = panel_box("L", &inner_lines, width);
    // With PANEL_VPAD=1, lines are: top, blank, body, blank, bottom (5 lines total)
    // The actual content body is at index 2 (after the VPAD blank row)
    let body_line = &lines[2];
    assert_eq!(
        char_width(body_line),
        width,
        "body line must be exactly {width} chars: {:?}",
        body_line
    );
    // Line structure: BOX_V + HPAD(1 space) + content_width chars + HPAD(1 space) + BOX_V
    // content_width = width - 2 - 2*PANEL_HPAD = 20 - 2 - 2 = 16
    // Content starts at position 2 (after BOX_V + HPAD)
    let inner_content: String = body_line.chars().skip(2).take(16).collect();
    assert!(
        inner_content.starts_with("hi"),
        "body content must start with 'hi' (after BOX_V + HPAD): {:?}",
        inner_content
    );
    assert!(
        inner_content.ends_with("  "),
        "body content must be right-padded with spaces: {:?}",
        inner_content
    );
}

#[test]
fn panel_box_long_label_clipped_with_ellipsis() {
    let width = 20;
    let long_label = "A Very Long Label That Does Not Fit";
    let lines = panel_box(long_label, &["x".into()], width);
    let top = &lines[0];
    assert_eq!(
        char_width(top),
        width,
        "top must be exactly {width} chars: {:?}",
        top
    );
    assert!(
        top.contains('\u{2026}'),
        "clipped label must contain ellipsis: {:?}",
        top
    );
    assert!(
        top.ends_with('\u{256E}'),
        "top must still end with TR corner: {:?}",
        top
    );
}

#[test]
fn panel_box_body_line_longer_than_inner_truncated() {
    // A body line longer than inner should be truncated to fit
    let width = 10;
    let long_line = "abcdefghijklmnopqrstuvwxyz".to_string();
    let lines = panel_box("L", &[long_line], width);
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
fn panel_box_empty_inner_lines_produces_empty_body_line() {
    let lines = panel_box("Label", &[String::new()], 20);
    // With PANEL_VPAD=1: top + blank + body + blank + bottom = 5 lines
    assert_eq!(
        lines.len(),
        5,
        "must have 5 lines (top+vpad+body+vpad+bottom): {:?}",
        lines
    );
    let body = &lines[2];
    assert_eq!(
        char_width(body),
        20,
        "empty body line must be exactly 20 chars: {:?}",
        body
    );
}

// --- D1-A1: panel_box display-width invariant (every line exactly `width` DISPLAY cols) ---

#[test]
fn panel_box_every_line_exactly_width_display_cols_ascii() {
    let width = 40usize;
    let lines = panel_box(
        "Details",
        &["short".into(), "a longer line here".into()],
        width,
    );
    for line in &lines {
        assert_eq!(
            dw(line),
            width,
            "ASCII: every line must be exactly {width} display cols: {line:?}"
        );
    }
}

#[test]
fn panel_box_every_line_exactly_width_display_cols_cjk() {
    let width = 40usize;
    let cjk_line = "日本語タスク".to_string();
    let lines = panel_box("CJK", &[cjk_line], width);
    for line in &lines {
        assert_eq!(
            dw(line),
            width,
            "CJK: every line must be exactly {width} display cols (right │ must close): {line:?}"
        );
        assert!(
            line.ends_with('\u{2502}') || line.ends_with('\u{256E}') || line.ends_with('\u{256F}'),
            "CJK: line must end with a box char: {line:?}"
        );
    }
}

#[test]
fn panel_box_every_line_exactly_width_display_cols_diamond() {
    let width = 40usize;
    let diamond_line = "◆ item one ◆ item two".to_string();
    let lines = panel_box("Diamonds", &[diamond_line], width);
    for line in &lines {
        assert_eq!(
            dw(line),
            width,
            "◆: every line must be exactly {width} display cols: {line:?}"
        );
    }
}

#[test]
fn panel_box_every_line_exactly_width_display_cols_decomposed_accent() {
    let width = 40usize;
    let accent_line = "Ma\u{0301}rço Otimizac\u{0327}a\u{0303}o".to_string();
    let lines = panel_box("Accent", &[accent_line], width);
    for line in &lines {
        assert_eq!(
            dw(line),
            width,
            "accent: every line must be exactly {width} display cols: {line:?}"
        );
    }
}

#[test]
fn panel_box_over_long_inner_line_right_border_closes() {
    let width = 20usize;
    let long_line = "abcdefghijklmnopqrstuvwxyz0123456789".to_string();
    let lines = panel_box("L", &[long_line], width);
    for line in &lines {
        assert_eq!(
            dw(line),
            width,
            "over-long: line must be exactly {width} display cols: {line:?}"
        );
    }
    let body_line = &lines[2];
    assert!(
        body_line.ends_with('\u{2502}'),
        "over-long: body line right border must be BOX_V: {body_line:?}"
    );
}

// --- D1-A2: panel_box horizontal + vertical padding invariants ---

#[test]
fn panel_box_vpad_first_and_last_body_rows_are_blank() {
    let width = 30usize;
    let lines = panel_box("Test", &["content".into()], width);
    // lines: top(0), blank_vpad(1), content(2), blank_vpad(3), bottom(4)
    assert_eq!(lines.len(), 5, "must have 5 lines with VPAD=1: {lines:?}");
    let first_body = &lines[1];
    let last_body = &lines[3];
    let inner = width - 2;
    let expected_blank = format!("\u{2502}{}\u{2502}", " ".repeat(inner));
    assert_eq!(
        first_body, &expected_blank,
        "first body row must be blank VPAD row: {first_body:?}"
    );
    assert_eq!(
        last_body, &expected_blank,
        "last body row must be blank VPAD row: {last_body:?}"
    );
}

#[test]
fn panel_box_hpad_separates_content_from_borders() {
    let width = 30usize;
    let lines = panel_box("P", &["content".into()], width);
    let content_line = &lines[2];
    let chars: Vec<char> = content_line.chars().collect();
    assert_eq!(chars[0], '\u{2502}', "must start with BOX_V");
    assert_eq!(chars[1], ' ', "char after BOX_V must be HPAD space");
    assert_eq!(chars[chars.len() - 1], '\u{2502}', "must end with BOX_V");
    assert_eq!(
        chars[chars.len() - 2],
        ' ',
        "char before BOX_V must be HPAD space"
    );
}

// --- U10: comment_box regression (delegates to panel_box, output byte-identical) ---

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

// --- U10: build_header_lines (Details panel) ---

#[test]
fn build_header_lines_returns_panel_with_details_label() {
    let task = json!({
        "id": 7,
        "project_id": 3,
        "project_name": "Alpha",
        "name": "Do the thing",
        "is_completed": false,
        "assignee_id": null,
        "estimate": 2.0f64,
        "tracked_time": 1.0f64
    });
    let lines = build_header_lines(&task, &HashMap::new(), 80);
    let joined = lines.join("\n");
    // Must be a panel with rounded borders
    assert!(
        lines[0].starts_with('\u{256D}'),
        "first line must be panel top border: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains("Details"),
        "panel top must contain 'Details' label: {joined}"
    );
    assert!(
        lines.last().unwrap().starts_with('\u{2570}'),
        "last line must be panel bottom border"
    );
    // Meta rows are inside
    assert!(joined.contains("Task"), "missing Task row: {joined}");
    assert!(joined.contains("3-7"), "missing task ref: {joined}");
    assert!(joined.contains("Project"), "missing Project: {joined}");
    assert!(joined.contains("Alpha"), "missing project name: {joined}");
    assert!(joined.contains("Status"), "missing Status: {joined}");
    assert!(joined.contains("Assignee"), "missing Assignee: {joined}");
    assert!(joined.contains("Estimate"), "missing Estimate: {joined}");
    assert!(joined.contains("Logged"), "missing Logged: {joined}");
}

#[test]
fn build_header_lines_no_title_row() {
    // Title row must be absent — the title band in build_detail_content shows the name
    let task = json!({
        "id": 1,
        "name": "Fix bug"
    });
    let lines = build_header_lines(&task, &HashMap::new(), 80);
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Title"),
        "Details panel must NOT include a Title row: {joined}"
    );
}

#[test]
fn build_header_lines_contains_no_description_or_comment_lines() {
    let task = json!({
        "id": 1,
        "body": "<p>Some body text</p>"
    });
    let lines = build_header_lines(&task, &HashMap::new(), 80);
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Description"),
        "must not include Description: {joined}"
    );
    assert!(
        !joined.contains("Some body text"),
        "must not include body text: {joined}"
    );
}

#[test]
fn build_header_lines_no_line_exceeds_inner_width() {
    let task = json!({
        "id": 99,
        "project_id": 10,
        "project_name": "A Very Long Project Name That Could Overflow The Line Width",
        "name": "A task with an extremely verbose name that also goes on too long",
        "is_completed": true,
        "assignee_id": 42,
        "start_on": 1614556800i64,
        "due_on": 1614643200i64,
        "estimate": 100.0f64,
        "tracked_time": 50.5f64
    });
    let mut user_map = HashMap::new();
    user_map.insert(42i64, "Josephine Longname".to_string());
    let inner_width = 40;
    let lines = build_header_lines(&task, &user_map, inner_width);
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
fn build_header_lines_optional_dates_included_when_present() {
    let task = json!({
        "id": 1,
        "start_on": 1614556800i64,
        "due_on": 1614643200i64
    });
    let lines = build_header_lines(&task, &HashMap::new(), 80);
    let joined = lines.join("\n");
    assert!(joined.contains("Start"), "missing Start: {joined}");
    assert!(joined.contains("Due"), "missing Due: {joined}");
}

#[test]
fn build_header_lines_optional_dates_omitted_when_null() {
    let task = json!({ "id": 1, "start_on": null, "due_on": null });
    let lines = build_header_lines(&task, &HashMap::new(), 80);
    let joined = lines.join("\n");
    // The label "Start" must not appear in a table row (it may appear in truncated label)
    // We check that date values are absent
    assert!(
        !joined.contains("2021-03-01"),
        "Start date value must be omitted: {joined}"
    );
    assert!(
        !joined.contains("2021-03-02"),
        "Due date value must be omitted: {joined}"
    );
}

#[test]
fn build_header_lines_2_column_alignment() {
    // All value columns must start at the same horizontal position (2-col aligned table).
    // Row format: "{label:<label_col$}  {value}" — label padded to label_col, then 2 spaces, then value.
    // The value start offset = label_col + 2 chars from the inner content start.
    let task = json!({
        "id": 42,
        "project_id": 7,
        "project_name": "Acme",
        "name": "My Task",
        "is_completed": false,
        "assignee_id": null,
        "estimate": 8.0f64,
        "tracked_time": 0.0f64
    });
    let lines = build_header_lines(&task, &HashMap::new(), 80);
    // Collect inner content of body lines (strip BOX_V from start and end, then strip HPAD).
    // With PANEL_VPAD=1, body lines start at index 2 (skip top border and VPAD blank row)
    // and end at len-2 (skip VPAD blank row and bottom border). Also skip blank VPAD rows.
    let body_lines: Vec<Vec<char>> = lines[1..lines.len() - 1]
        .iter()
        .filter_map(|line| {
            let chars: Vec<char> = line.chars().collect();
            // inner = chars[1..len-1], then strip leading/trailing HPAD (1 space each side)
            let inner = &chars[1..chars.len() - 1];
            // Skip blank padding rows (all spaces)
            if inner.iter().all(|c| *c == ' ') {
                return None;
            }
            // Strip 1 char of HPAD from each side
            if inner.len() >= 2 {
                Some(inner[1..inner.len() - 1].to_vec())
            } else {
                Some(inner.to_vec())
            }
        })
        .collect();

    // For each row, find the start of the value: scan backward from the first non-space
    // char's position, finding the transition from all-spaces to non-space-prefix.
    // Since format is "{label:<N$}  {value}", the value starts immediately after the "  " separator.
    // We find value_start as the position of the first non-space char AFTER all leading
    // label+padding+separator spaces — i.e., skip label text, skip all spaces, the next non-space
    // is the value. The position of that non-space char is the value_start.
    //
    // Specifically: scan from position 0, skip non-spaces (label chars), then skip spaces
    // (padding + separator). The first non-space after that run of spaces is the value start.
    fn value_start_offset(inner: &[char]) -> Option<usize> {
        let mut i = 0;
        // Skip label chars (non-space)
        while i < inner.len() && inner[i] != ' ' {
            i += 1;
        }
        if i == 0 || i >= inner.len() {
            return None;
        }
        // Skip all spaces (padding + separator)
        while i < inner.len() && inner[i] == ' ' {
            i += 1;
        }
        Some(i)
    }

    let value_positions: Vec<usize> = body_lines
        .iter()
        .filter_map(|chars| value_start_offset(chars))
        .collect();

    assert!(
        !value_positions.is_empty(),
        "must detect value column positions in body lines"
    );
    let first = value_positions[0];
    for &pos in &value_positions {
        assert_eq!(
            pos, first,
            "all value columns must start at the same offset (2-col alignment): positions={value_positions:?}"
        );
    }
}

// --- U10: build_body_lines_with_collector (Description panel) ---

#[test]
fn build_body_lines_is_panel_with_description_label() {
    let task = json!({ "id": 1, "body": "<p>Hello world</p>" });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    assert!(
        lines[0].starts_with('\u{256D}'),
        "first line must be panel top border: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains("Description"),
        "panel top must contain 'Description': {:?}",
        lines[0]
    );
    assert!(
        lines.last().unwrap().starts_with('\u{2570}'),
        "last line must be panel bottom border"
    );
}

#[test]
fn build_body_lines_includes_wrapped_body_text() {
    let task = json!({ "id": 1, "body": "<p>Some details here</p>" });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        joined.contains("Some details here"),
        "missing body text: {joined}"
    );
}

#[test]
fn build_body_lines_fallback_when_body_empty() {
    let task = json!({ "id": 1, "body": null });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        joined.contains("(no description)"),
        "missing fallback: {joined}"
    );
}

#[test]
fn build_body_lines_contains_no_meta_or_comment_lines() {
    let task = json!({
        "id": 42,
        "project_name": "MyProject",
        "name": "My Task",
        "body": "<p>Body content</p>"
    });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        !joined.contains("Task:"),
        "must not include Task row: {joined}"
    );
    assert!(
        !joined.contains("Project:"),
        "must not include Project row: {joined}"
    );
}

#[test]
fn build_body_lines_no_line_exceeds_inner_width() {
    let task = json!({
        "id": 1,
        "body": "<p>This is a fairly long body text that should wrap to stay within the width boundary set by the caller</p>"
    });
    let inner_width = 30;
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, inner_width, &mut collector);
    for line in &lines {
        let len = line.chars().count();
        assert!(
            len <= inner_width,
            "line exceeds {inner_width} chars ({len}): {:?}",
            line
        );
    }
}

// --- U10: build_comment_lines_with_collector (Comments panel) ---

#[test]
fn build_comment_lines_empty_for_zero_comments() {
    let mut collector = LinkCollector::new();
    let (lines, _) = build_comment_lines_with_collector(&[], 80, &mut collector);
    assert!(
        lines.is_empty(),
        "must return empty vec for no comments: {:?}",
        lines
    );
}

#[test]
fn build_comment_lines_returns_outer_panel_for_single_comment() {
    let comments = vec![json!({
        "created_by_name": "Alice",
        "created_on": 1614556800i64,
        "body_plain_text": "LGTM!"
    })];
    let mut collector = LinkCollector::new();
    let (lines, _) = build_comment_lines_with_collector(&comments, 60, &mut collector);
    let joined = lines.join("\n");
    // Must be wrapped in an outer panel
    assert!(
        lines[0].starts_with('\u{256D}'),
        "outer panel top must start with BOX_TL: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains("Comments"),
        "outer panel must have 'Comments' label: {joined}"
    );
    assert!(
        lines[0].contains("(1)"),
        "outer panel must show count (1): {joined}"
    );
    // Inner comment card must be present
    assert!(joined.contains("Alice"), "must contain author: {joined}");
    assert!(joined.contains("LGTM!"), "must contain body: {joined}");
}

#[test]
fn build_comment_lines_returns_outer_panel_for_multiple_comments() {
    let comments = vec![
        json!({
            "created_by_name": "Alice",
            "created_on": 1614556800i64,
            "body_plain_text": "First"
        }),
        json!({
            "created_by_name": "Bob",
            "created_on": 1614556801i64,
            "body_plain_text": "Second"
        }),
    ];
    let mut collector = LinkCollector::new();
    let (lines, _) = build_comment_lines_with_collector(&comments, 60, &mut collector);
    let joined = lines.join("\n");
    assert!(
        lines[0].contains("Comments"),
        "outer panel must have 'Comments' label: {joined}"
    );
    assert!(
        lines[0].contains("(2)"),
        "outer panel must show count (2): {joined}"
    );
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
}

#[test]
fn build_comment_lines_no_line_exceeds_inner_width() {
    let comments = vec![json!({
        "created_by_name": "Josephine With A Very Long Name",
        "created_on": 1614556800i64,
        "body_plain_text": "A comment body that is quite long and must be wrapped to fit inside the column width"
    })];
    let inner_width = 40;
    let mut collector = LinkCollector::new();
    let (lines, _) = build_comment_lines_with_collector(&comments, inner_width, &mut collector);
    for line in &lines {
        let len = line.chars().count();
        assert!(
            len <= inner_width,
            "line exceeds {inner_width} chars ({len}): {:?}",
            line
        );
    }
}

// --- U10: build_detail_content (full detail layout) ---

#[test]
fn build_detail_lines_first_line_is_details_panel_top_border() {
    let task = json!({
        "id": 99,
        "project_id": 10,
        "project_name": "Acme",
        "name": "Fix bug",
        "is_completed": false,
    });
    let lines = build_detail_content(&task, &[], &HashMap::new(), 80).lines;
    assert!(!lines.is_empty(), "must produce lines");
    let first = &lines[0];
    assert!(
        first.starts_with('\u{256D}'),
        "first line must be the Details panel top border (╭): {:?}",
        first
    );
    assert!(
        first.contains("Details"),
        "first line must contain 'Details' panel label: {:?}",
        first
    );
    assert!(
        !first.contains("Fix bug"),
        "task name must NOT appear in the scroll body (it is in the frame border): {:?}",
        first
    );
}

#[test]
fn build_detail_lines_details_panel_is_first() {
    let task = json!({
        "id": 5,
        "project_id": 2,
        "name": "My Task",
        "is_completed": false
    });
    let lines = build_detail_content(&task, &[], &HashMap::new(), 60).lines;
    assert!(
        !lines.is_empty(),
        "must have at least one line: {:?}",
        lines
    );
    assert!(
        lines[0].starts_with('\u{256D}'),
        "lines[0] must be Details panel top: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains("Details"),
        "Details panel must have 'Details' label: {:?}",
        lines[0]
    );
}

#[test]
fn build_detail_lines_description_panel_present() {
    let task = json!({ "id": 1, "name": "T", "body": "<p>Some details here</p>" });
    let lines = build_detail_content(&task, &[], &HashMap::new(), 80).lines;
    let joined = lines.join("\n");
    assert!(
        joined.contains("Description"),
        "must contain Description panel: {joined}"
    );
    assert!(
        joined.contains("Some details here"),
        "missing body text: {joined}"
    );
}

#[test]
fn build_detail_lines_no_description_fallback() {
    let task = json!({ "id": 1, "name": "T", "body": null });
    let lines = build_detail_content(&task, &[], &HashMap::new(), 80).lines;
    let joined = lines.join("\n");
    assert!(
        joined.contains("(no description)"),
        "missing fallback: {joined}"
    );
}

#[test]
fn build_detail_lines_no_comments_panel_when_empty() {
    let task = json!({ "id": 1, "name": "T" });
    let lines = build_detail_content(&task, &[], &HashMap::new(), 80).lines;
    let joined = lines.join("\n");
    // Comments panel must be absent; Details and Description panels ARE present
    assert!(
        !joined.contains("Comments"),
        "must not have Comments panel when no comments: {joined}"
    );
    // But Details panel is present
    assert!(
        joined.contains("Details"),
        "Details panel must be present: {joined}"
    );
}

#[test]
fn build_detail_lines_comments_panel_present_when_non_empty() {
    let task = json!({ "id": 1, "name": "T" });
    let comments = vec![json!({
        "created_by_name": "Bob",
        "created_on": 1614556800i64,
        "body_plain_text": "LGTM!"
    })];
    let lines = build_detail_content(&task, &comments, &HashMap::new(), 60).lines;
    let joined = lines.join("\n");
    assert!(
        joined.contains("Comments"),
        "must have Comments panel when comments present: {joined}"
    );
    assert!(
        joined.contains("Bob"),
        "must contain comment author: {joined}"
    );
    assert!(
        joined.contains("LGTM!"),
        "must contain comment body: {joined}"
    );
    // Must have box corners (from panels)
    assert!(
        joined.contains('\u{256D}'),
        "must have rounded box corners: {joined}"
    );
}

#[test]
fn build_detail_lines_no_title_row_in_meta() {
    let task = json!({
        "id": 1,
        "name": "My Important Task",
        "project_id": 5,
        "project_name": "Acme"
    });
    let lines = build_detail_content(&task, &[], &HashMap::new(), 80).lines;
    let joined = lines.join("\n");
    // The title band contains the name; the Details panel must NOT have a "Title" label row
    assert!(
        !joined.contains("Title"),
        "Details panel must NOT contain a Title row: {joined}"
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
    let lines = build_detail_content(&task, &comments, &HashMap::new(), inner_width).lines;
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
fn build_detail_lines_panels_appear_in_order() {
    // title band -> blank -> Details -> blank -> Description -> [blank + Comments]
    let task = json!({
        "id": 1,
        "name": "Task Name",
        "project_id": 3,
        "project_name": "Proj",
        "body": "<p>Body text</p>"
    });
    let comments = vec![json!({
        "created_by_name": "Carol",
        "created_on": 1614556800i64,
        "body_plain_text": "A comment"
    })];
    let lines = build_detail_content(&task, &comments, &HashMap::new(), 60).lines;
    let joined = lines.join("\n");

    // Find positions of panel labels
    let details_pos = joined.find("Details").expect("Details must be present");
    let desc_pos = joined
        .find("Description")
        .expect("Description must be present");
    let comments_pos = joined.find("Comments").expect("Comments must be present");

    assert!(
        details_pos < desc_pos,
        "Details must appear before Description: details_pos={details_pos} desc_pos={desc_pos}"
    );
    assert!(
        desc_pos < comments_pos,
        "Description must appear before Comments: desc_pos={desc_pos} comments_pos={comments_pos}"
    );
}

#[test]
fn build_detail_lines_multiple_comments_in_outer_panel() {
    let task = json!({ "id": 1, "name": "T" });
    let comments = vec![
        json!({ "created_by_name": "Alice", "created_on": 1614556800i64, "body_plain_text": "First" }),
        json!({ "created_by_name": "Bob", "created_on": 1614556801i64, "body_plain_text": "Second" }),
    ];
    let lines = build_detail_content(&task, &comments, &HashMap::new(), 50).lines;
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
    // Outer panel shows count
    assert!(
        joined.contains("(2)"),
        "outer Comments panel must show (2): {joined}"
    );
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

// --- U11: looks_like_filename and asset_link_line ---

#[test]
fn looks_like_filename_accepts_real_filename() {
    assert!(
        looks_like_filename("report.pdf"),
        "report.pdf must be a valid filename"
    );
}

#[test]
fn looks_like_filename_accepts_short_extension() {
    assert!(
        looks_like_filename("image.png"),
        "image.png must be a valid filename"
    );
}

#[test]
fn looks_like_filename_accepts_six_char_extension() {
    assert!(
        looks_like_filename("archive.tar123"),
        "six-char alphanumeric extension must be accepted"
    );
}

#[test]
fn looks_like_filename_rejects_empty() {
    assert!(!looks_like_filename(""), "empty name must be rejected");
}

#[test]
fn looks_like_filename_rejects_no_extension() {
    assert!(
        !looks_like_filename("a8f3c920-deadbeef"),
        "name with no dot extension must be rejected"
    );
}

#[test]
fn looks_like_filename_rejects_extension_too_long() {
    assert!(
        !looks_like_filename("file.abcdefg"),
        "seven-char extension must be rejected"
    );
}

#[test]
fn looks_like_filename_rejects_over_48_chars_even_with_valid_extension() {
    let long_name = format!("{}.pdf", "a".repeat(46));
    assert_eq!(long_name.chars().count(), 50);
    assert!(
        !looks_like_filename(&long_name),
        "name longer than 48 chars must be rejected: {long_name:?}"
    );
}

#[test]
fn looks_like_filename_accepts_exactly_48_chars_with_valid_extension() {
    let name = format!("{}.pdf", "a".repeat(44));
    assert_eq!(name.chars().count(), 48);
    assert!(
        looks_like_filename(&name),
        "48-char name with valid extension must be accepted: {name:?}"
    );
}

#[test]
fn looks_like_filename_rejects_non_alphanumeric_extension() {
    assert!(
        !looks_like_filename("file.tar.gz-sig"),
        "extension with non-alphanumeric chars after last dot must be rejected"
    );
}

#[test]
fn looks_like_filename_rejects_empty_extension_after_trailing_dot() {
    assert!(
        !looks_like_filename("file."),
        "trailing dot with empty extension must be rejected"
    );
}

#[test]
fn asset_link_line_real_filename_uses_name_as_label() {
    let asset = Asset {
        name: "report.pdf".to_owned(),
        url: "https://example.com/files/report.pdf".to_owned(),
    };
    let line = asset_link_line(1, &asset);
    assert_eq!(
        line, "[1] \u{2197} report.pdf",
        "real filename must appear as label: {line:?}"
    );
}

#[test]
fn asset_link_line_ugly_fragment_falls_back_to_open_link() {
    let asset = Asset {
        name: "a8f3c920-deadbeef".to_owned(),
        url: "https://example.com/a8f3c920-deadbeef".to_owned(),
    };
    let line = asset_link_line(2, &asset);
    assert_eq!(
        line, "[2] \u{2197} Open link",
        "ugly fragment must fall back to 'Open link': {line:?}"
    );
}

#[test]
fn asset_link_line_empty_name_falls_back_to_open_link() {
    let asset = Asset {
        name: String::new(),
        url: "https://example.com/resource".to_owned(),
    };
    let line = asset_link_line(3, &asset);
    assert_eq!(
        line, "[3] \u{2197} Open link",
        "empty name must fall back to 'Open link': {line:?}"
    );
}

#[test]
fn asset_link_line_over_long_name_falls_back_to_open_link() {
    let long_name = format!("{}.pdf", "x".repeat(46));
    assert_eq!(long_name.chars().count(), 50);
    let asset = Asset {
        name: long_name,
        url: "https://example.com/long".to_owned(),
    };
    let line = asset_link_line(4, &asset);
    assert_eq!(
        line, "[4] \u{2197} Open link",
        "over-long name must fall back to 'Open link': {line:?}"
    );
}

#[test]
fn asset_link_line_index_is_1_based_and_matches_position() {
    let asset = Asset {
        name: "doc.docx".to_owned(),
        url: "https://example.com/doc.docx".to_owned(),
    };
    let line_1 = asset_link_line(1, &asset);
    let line_9 = asset_link_line(9, &asset);
    assert!(
        line_1.starts_with("[1]"),
        "index 1 must appear as [1]: {line_1:?}"
    );
    assert!(
        line_9.starts_with("[9]"),
        "index 9 must appear as [9]: {line_9:?}"
    );
}

// --- V5-A2: build_detail_content (inline URLs) ---

#[test]
fn build_detail_content_url_in_description_renders_inline() {
    let task = json!({
        "id": 1,
        "name": "T",
        "body": "<p>See https://example.com/info for details.</p>"
    });
    let content = build_detail_content(&task, &[], &HashMap::new(), 80);
    let joined = content.lines.join("\n");
    assert!(
        joined.contains("https://example.com/info"),
        "inline URL must appear in lines: {joined}"
    );
}

#[test]
fn build_detail_content_url_in_comment_body_renders_inline() {
    let task = json!({ "id": 1, "name": "T", "body": "<p>No URL here.</p>" });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1614556800i64,
        "body_plain_text": "Ref: https://api.example.com/v1"
    });
    let content = build_detail_content(&task, &[comment], &HashMap::new(), 80);
    let joined = content.lines.join("\n");
    assert!(
        joined.contains("https://api.example.com/v1"),
        "comment URL must appear inline: {joined}"
    );
}

#[test]
fn build_detail_content_no_url_produces_lines_with_no_url() {
    let task = json!({ "id": 1, "name": "T", "body": "<p>No links here.</p>" });
    let content = build_detail_content(&task, &[], &HashMap::new(), 80);
    let joined = content.lines.join("\n");
    assert!(
        !joined.contains("https://"),
        "no URL in body must produce no URL in lines: {joined}"
    );
}

// --- V5-A3: url_at resolves click column to URL ---

#[test]
fn url_at_bracketed_url_returns_inner_without_brackets() {
    let url = "https://example.com/path";
    let line = format!("text [{url}] more");
    // "text [" is 6 chars wide; url starts at col 6
    let col = 6;
    let result = url_at(&line, col);
    assert_eq!(
        result.as_deref(),
        Some(url),
        "url_at inside bracketed URL must return the URL without brackets: {result:?}"
    );
}

#[test]
fn url_at_returns_raw_body_url_when_clicked_on_raw_url() {
    let url = "https://direct.example.com/page";
    let line = format!("Visit {url} now");
    let col = 6; // "Visit " is 6 chars wide
    let result = url_at(&line, col);
    assert_eq!(
        result.as_deref(),
        Some(url),
        "url_at must return raw URL for click on it: {result:?}"
    );
}

#[test]
fn url_at_non_url_bracket_token_returns_none() {
    let line = "see [note] for details";
    // col 5 is inside "[note]" — but it's not a URL
    let result = url_at(&line, 5);
    assert!(
        result.is_none(),
        "url_at on non-url '[note]' must return None: {result:?}"
    );
}

#[test]
fn url_at_plain_text_returns_none() {
    let result = url_at("just plain text with no links", 5);
    assert!(
        result.is_none(),
        "url_at on plain text must return None: {result:?}"
    );
}

#[test]
fn url_at_col_on_opening_bracket_returns_none() {
    let url = "https://example.com/path";
    let line = format!("[{url}]");
    // col 0 is the '[' character itself (not inside the URL)
    let result = url_at(&line, 0);
    assert!(
        result.is_none(),
        "url_at on the '[' bracket must return None: {result:?}"
    );
}

#[test]
fn url_at_col_on_closing_bracket_returns_none() {
    let url = "https://example.com/path";
    let line = format!("[{url}]");
    let closing_col = 1 + url.len(); // '[' (1) + url length
    let result = url_at(&line, closing_col);
    assert!(
        result.is_none(),
        "url_at on the ']' bracket must return None: {result:?}"
    );
}

#[test]
fn url_at_wide_glyph_before_bracketed_url_shifts_col_correctly() {
    // "中" has display width 2, so "[url]" starts at col 2
    let url = "https://example.com";
    let line = format!("\u{4E2D}[{url}]");
    // col 3 is inside the URL span (after '[' at col 2)
    let result = url_at(&line, 3);
    assert_eq!(
        result.as_deref(),
        Some(url),
        "wide glyph before bracketed URL must shift cols correctly: {result:?}"
    );
}

#[test]
fn url_at_empty_line_returns_none() {
    assert!(
        url_at("", 0).is_none(),
        "url_at on empty line must return None"
    );
}

#[test]
fn url_at_col_past_end_returns_none() {
    let result = url_at("hello", 9999);
    assert!(result.is_none(), "url_at past end of line must return None");
}

// --- R3a: build_body_lines_with_collector richtext structure (TUI path) ---

#[test]
fn build_body_lines_ul_produces_bullet_lines() {
    let task = json!({ "id": 1, "body": "<ul><li>alpha</li><li>beta</li></ul>" });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        joined.contains("\u{2022} alpha"),
        "unordered list must produce bullet lines: {joined}"
    );
    assert!(
        joined.contains("\u{2022} beta"),
        "second bullet item must appear: {joined}"
    );
}

#[test]
fn build_body_lines_ol_produces_numbered_lines() {
    let task = json!({ "id": 1, "body": "<ol><li>first</li><li>second</li></ol>" });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        joined.contains("1. first"),
        "ordered list item 1 must be '1. first': {joined}"
    );
    assert!(
        joined.contains("2. second"),
        "ordered list item 2 must be '2. second': {joined}"
    );
}

#[test]
fn build_body_lines_blockquote_produces_gt_prefix() {
    let task = json!({ "id": 1, "body": "<blockquote>quoted text</blockquote>" });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        joined.contains("> quoted text"),
        "blockquote must produce '> ' prefix in TUI body: {joined}"
    );
}

#[test]
fn build_body_lines_h2_produces_heading_on_own_line() {
    let task = json!({ "id": 1, "body": "<h2>Section Title</h2>" });
    let mut collector = LinkCollector::new();
    let (lines, _) = build_body_lines_with_collector(&task, 80, &mut collector);
    let joined = lines.join("\n");
    assert!(
        joined.contains("Section Title"),
        "heading must appear in body lines: {joined}"
    );
}

#[test]
fn build_detail_content_with_list_body_produces_bullet_lines() {
    let task = json!({
        "id": 1,
        "name": "T",
        "body": "<ul><li>one</li><li>two</li></ul>"
    });
    let content = build_detail_content(&task, &[], &HashMap::new(), 80);
    let joined = content.lines.join("\n");
    assert!(
        joined.contains("\u{2022} one"),
        "detail content must contain bullet 'one': {joined}"
    );
    assert!(
        joined.contains("\u{2022} two"),
        "detail content must contain bullet 'two': {joined}"
    );
}

#[test]
fn build_detail_content_comment_with_list_body_produces_bullet_lines() {
    let task = json!({ "id": 1, "name": "T", "body": "" });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1614556800i64,
        "body": "<ul><li>item A</li><li>item B</li></ul>"
    });
    let content = build_detail_content(&task, &[comment], &HashMap::new(), 80);
    let joined = content.lines.join("\n");
    assert!(
        joined.contains("\u{2022} item A"),
        "comment body list must produce bullets in TUI: {joined}"
    );
}

// --- R3a-A4 / BDR 0003: CLI path unchanged ——————————————————————————————

#[test]
fn render_task_to_str_uses_html_to_text_not_richtext_for_list_body() {
    // The CLI path must produce the same flat output html_to_text produces —
    // NOT the structured bullet format of structured_text_with_links.
    let html = "<ul><li>item one</li><li>item two</li></ul>";
    let task = json!({ "id": 1, "name": "T", "body": html });
    let cli_output = render_task_to_str(&task, &[], false, &HashMap::new());

    // CLI flattens html: list items become plain text lines without bullet prefix.
    // structured_text_with_links would produce "• item one\n• item two".
    // html_to_text produces "item one\nitem two" (li tag → newline, no prefix).
    let expected_flat = html_to_text(html);
    assert!(
        cli_output.contains(&expected_flat),
        "CLI output must contain html_to_text flat result, not richtext bullets: {cli_output}"
    );
    assert!(
        !cli_output.contains("\u{2022}"),
        "CLI output must NOT contain bullet character from richtext: {cli_output}"
    );
}

#[test]
fn render_comments_to_str_uses_html_to_text_not_richtext() {
    let comment = json!({
        "created_by_name": "Bob",
        "created_on": 1614556800i64,
        "body": "<ul><li>item one</li><li>item two</li></ul>"
    });
    let cli_output = render_comments_to_str(&[comment]);
    assert!(
        !cli_output.contains("\u{2022}"),
        "CLI comments output must NOT contain richtext bullet prefix: {cli_output}"
    );
    assert!(
        cli_output.contains("item one"),
        "CLI comments must still show list item text: {cli_output}"
    );
}

// --- V5-A4: url_at with bordered/padded lines (panel body simulation) ---

#[test]
fn url_at_col_on_border_char_returns_none() {
    let url = "https://example.com/info";
    let line = format!("\u{2502} [{url}] \u{2502}");
    // Col 0 is the │ border
    assert!(
        url_at(&line, 0).is_none(),
        "col on │ border must return None"
    );
}

#[test]
fn url_at_inside_bracketed_url_in_bordered_padded_line() {
    // "│ [https://example.com] │" — url starts at col 3 (│=1, space=1, [=1)
    let url = "https://example.com";
    let line = format!("\u{2502} [{url}] \u{2502}");
    let col_inside_url = 3; // first char of the URL (after '│ [')
    let result = url_at(&line, col_inside_url);
    assert_eq!(
        result.as_deref(),
        Some(url),
        "url_at inside bordered bracketed URL must return URL: {result:?}"
    );
}
