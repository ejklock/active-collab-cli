use crate::i18n::t;
use crate::store::secs_to_utc_parts;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;

/// A displayable asset extracted from a task body or attachments list.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

static IMG_SRC_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
static HREF_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

fn img_src_re() -> &'static Regex {
    IMG_SRC_RE.get_or_init(|| {
        Regex::new(r#"(?i)<img\b[^>]*\bsrc=["']([^"']+)["']"#)
            .expect("img_src_re is a valid pattern")
    })
}

fn href_re() -> &'static Regex {
    HREF_RE.get_or_init(|| {
        Regex::new(r#"(?i)<a\b[^>]*\bhref=["']([^"']+)["']"#).expect("href_re is a valid pattern")
    })
}

fn assets_from_html(html: &str) -> Vec<Asset> {
    let mut assets = vec![];
    for cap in img_src_re().captures_iter(html) {
        let url = cap[1].to_string();
        let name = url_basename(&url);
        assets.push(Asset { name, url });
    }
    for cap in href_re().captures_iter(html) {
        let url = cap[1].to_string();
        let name = url_basename(&url);
        assets.push(Asset { name, url });
    }
    assets
}

fn assets_from_attachments(attachments: &Value) -> Vec<Asset> {
    let arr = match attachments.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    arr.iter()
        .filter_map(|att| {
            let url = att
                .get("url")
                .or_else(|| att.get("download_url"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())?
                .to_string();
            let name = att
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| url_basename(&url));
            Some(Asset { name, url })
        })
        .collect()
}

fn url_basename(url: &str) -> String {
    url.split('/')
        .next_back()
        .filter(|s| !s.is_empty())
        .unwrap_or(url)
        .to_string()
}

/// Extract all assets (images, links, attachments) from a task JSON.
///
/// Deduplicates by URL, preserving first-seen order.
pub fn extract_assets(task: &Value, comments: &[Value]) -> Vec<Asset> {
    let mut seen = std::collections::HashSet::new();
    let mut result = vec![];

    let mut add = |asset: Asset| {
        if seen.insert(asset.url.clone()) {
            result.push(asset);
        }
    };

    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    for asset in assets_from_html(body_html) {
        add(asset);
    }

    for comment in comments {
        let comment_html = comment.get("body").and_then(|v| v.as_str()).unwrap_or("");
        for asset in assets_from_html(comment_html) {
            add(asset);
        }
    }

    if let Some(attachments) = task.get("attachments") {
        for asset in assets_from_attachments(attachments) {
            add(asset);
        }
    }

    result
}

/// Parity: Python tui/view.py is_openable_url.
///
/// Returns true only for http and https schemes. Rejects file://, javascript:,
/// data:, mailto:, relative, and empty URLs. Uses the url crate for parsing —
/// never hand-rolls scheme extraction.
pub fn is_openable_url(url: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    match url::Url::parse(url) {
        Ok(parsed) => matches!(parsed.scheme(), "http" | "https"),
        Err(_) => false,
    }
}

/// One row in the mine/list table.
pub struct MineTableRow {
    pub instance: String,
    pub project_id: i64,
    pub task_number: i64,
    pub task_id: i64,
    pub name: String,
}

/// Parity: render.py render_mine_table.
///
/// Returns the formatted table as a String (header + separator + rows).
pub fn render_mine_table(rows: &[MineTableRow]) -> String {
    let header = format!(
        "{:<15} {:<10} {:<8} {:<10} {}",
        t("INSTANCE"),
        t("PROJECT"),
        t("TASK#"),
        t("TASK_ID"),
        t("NAME")
    );
    let separator = "-".repeat(80);
    let mut lines = vec![header, separator];
    for row in rows {
        lines.push(format!(
            "{:<15} {:<10} {:<8} {:<10} {}",
            row.instance, row.project_id, row.task_number, row.task_id, row.name
        ));
    }
    lines.join("\n")
}

fn block_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)<(?:br|p|div|li|tr|h[1-6])\b[^>]*>")
            .expect("block_tag_re is a valid pattern")
    })
}

fn any_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("any_tag_re is a valid pattern"))
}

fn blank_lines_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").expect("blank_lines_re is a valid pattern"))
}

/// Parity: render.py html_to_text.
///
/// Strip HTML tags and decode entities. Block/br tags become newlines.
/// 3+ consecutive newlines collapse to 2. Result is trimmed.
pub fn html_to_text(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }
    let text = block_tag_re().replace_all(html, "\n");
    let text = any_tag_re().replace_all(&text, "");
    let text = html_escape::decode_html_entities(&text).into_owned();
    let text = blank_lines_re().replace_all(&text, "\n\n");
    // Mirror Python str.strip(): trim only ASCII whitespace, not Unicode whitespace like U+00A0
    let text = text.trim_matches(|c: char| c.is_ascii_whitespace());
    text.to_owned()
}

/// Parity: render.py fmt_ts.
///
/// null/missing -> ""; numeric unix seconds -> UTC "YYYY-MM-DD HH:MM"; else str.
pub fn fmt_ts(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Number(n) => {
            let secs = if let Some(i) = n.as_i64() {
                i as u64
            } else if let Some(f) = n.as_f64() {
                f as u64
            } else {
                return value.to_string();
            };
            let (year, month, day, hour, min, _sec) = secs_to_utc_parts(secs);
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}")
        }
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

/// Parity: render.py fmt_date.
///
/// null/missing -> ""; numeric unix seconds -> UTC "YYYY-MM-DD"; else str.
pub fn fmt_date(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Number(n) => {
            let secs = if let Some(i) = n.as_i64() {
                i as u64
            } else if let Some(f) = n.as_f64() {
                f as u64
            } else {
                return value.to_string();
            };
            let (year, month, day, _hour, _min, _sec) = secs_to_utc_parts(secs);
            format!("{year:04}-{month:02}-{day:02}")
        }
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

/// Parity: render.py fmt_hours.
///
/// null -> "0"; numeric -> integer string when whole, fractional otherwise; non-numeric -> its string.
pub fn fmt_hours(value: &Value) -> String {
    match value {
        Value::Null => "0".to_owned(),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f == f.trunc() {
                    return (f as i64).to_string();
                }
                return f.to_string();
            }
            n.to_string()
        }
        Value::String(s) => match s.parse::<f64>() {
            Ok(f) => {
                if f == f.trunc() {
                    (f as i64).to_string()
                } else {
                    f.to_string()
                }
            }
            Err(_) => s.clone(),
        },
        _ => "0".to_owned(),
    }
}

/// Parity: render.py render_meta_to_str.
///
/// Returns assignee, dates, estimate, and logged hours as a string.
pub fn render_meta_to_str(task: &Value, user_map: &HashMap<i64, String>) -> String {
    let assignee_label = match task.get("assignee_id").and_then(|v| v.as_i64()) {
        None => t("(unassigned)"),
        Some(id) => match user_map.get(&id) {
            Some(name) => format!("{name} ({id})"),
            None => format!("({id})"),
        },
    };

    let mut lines = vec![format!("{}:  {}", t("Assignee"), assignee_label)];

    let start = fmt_date(task.get("start_on").unwrap_or(&Value::Null));
    if !start.is_empty() {
        lines.push(format!("{}:     {}", t("Start"), start));
    }

    let due = fmt_date(task.get("due_on").unwrap_or(&Value::Null));
    if !due.is_empty() {
        lines.push(format!("{}:       {}", t("Due"), due));
    }

    lines.push(format!(
        "{}:  {}h",
        t("Estimate"),
        fmt_hours(task.get("estimate").unwrap_or(&Value::Null))
    ));
    lines.push(format!(
        "{}:    {}h",
        t("Logged"),
        fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null))
    ));

    lines.join("\n")
}

/// Parity: render.py render_comments_to_str.
///
/// Returns the comments section for a task view as a string.
pub fn render_comments_to_str(comments: &[Value]) -> String {
    if comments.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = vec![format!("\n{} ({}):", t("Comments"), comments.len())];

    for (i, c) in comments.iter().enumerate() {
        let author = {
            let by_name = c
                .get("created_by_name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let by_id = c
                .get("created_by_id")
                .map(|v| v.to_string())
                .filter(|s| !s.is_empty() && s != "null");
            by_name
                .map(|s| s.to_owned())
                .or(by_id)
                .unwrap_or_else(|| t("(unknown)"))
        };

        let created = fmt_ts(c.get("created_on").unwrap_or(&Value::Null));

        let body_text = {
            let plain = c
                .get("body_plain_text")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            match plain {
                Some(s) => s.to_owned(),
                None => {
                    let html = c.get("body").and_then(|v| v.as_str()).unwrap_or("");
                    html_to_text(html)
                }
            }
        };

        lines.push(format!("\n  [{}] {} \u{2014} {}", i + 1, author, created));
        for line in body_text.lines() {
            lines.push(format!("  {line}"));
        }
    }

    lines.join("\n")
}

/// Parity: render.py render_task_to_str.
///
/// Returns a human-readable task view as a string.
pub fn render_task_to_str(
    task: &Value,
    comments: &[Value],
    no_comments: bool,
    user_map: &HashMap<i64, String>,
) -> String {
    let status_label = if task
        .get("is_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        t("Completed")
    } else {
        t("Open")
    };

    let task_num = task
        .get("task_number")
        .filter(|v| !v.is_null())
        .or_else(|| task.get("id"))
        .and_then(|v| {
            if let Some(n) = v.as_i64() {
                Some(n.to_string())
            } else {
                v.as_str().map(|s| s.to_owned())
            }
        })
        .unwrap_or_default();

    let name = task.get("name").and_then(|v| v.as_str()).unwrap_or("");

    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let description = {
        let text = html_to_text(body_html);
        if text.is_empty() {
            t("(no description)")
        } else {
            text
        }
    };

    let meta = render_meta_to_str(task, user_map);

    let mut lines = vec![
        format!("{}:      {}", t("Task"), task_num),
        format!("{}:      {}", t("Name"), name),
        format!("{}:    {}", t("Status"), status_label),
        meta,
        String::new(),
        format!("{}:", t("Description")),
        description,
    ];

    if !no_comments {
        let comments_str = render_comments_to_str(comments);
        if !comments_str.is_empty() {
            lines.push(comments_str);
        }
    }

    lines.join("\n")
}

/// Parity: render.py render_task.
///
/// Prints a human-readable task view to `out`.
pub fn render_task(
    task: &Value,
    comments: &[Value],
    no_comments: bool,
    user_map: &HashMap<i64, String>,
    out: &mut dyn Write,
) {
    let s = render_task_to_str(task, comments, no_comments, user_map);
    writeln!(out, "{s}").ok();
}

/// Write `msg` followed by a newline to stderr.
/// This is the render.print_error parity seam.
pub fn print_error(msg: &str) {
    eprintln!("{msg}");
}

const BOX_TL: &str = "\u{256D}";
const BOX_TR: &str = "\u{256E}";
const BOX_BL: &str = "\u{2570}";
const BOX_BR: &str = "\u{256F}";
const BOX_H: &str = "\u{2500}";
const BOX_V: &str = "\u{2502}";
const ELLIPSIS: &str = "\u{2026}";
const MIDDOT: &str = "\u{00B7}";

/// Parity: Python tui.py wrap_text.
///
/// Greedy word-wrap on whitespace to at most `width` columns per line (char count).
/// A single word longer than `width` is hard-split at `width`. Existing line breaks
/// in `text` are preserved — each input line is wrapped independently. Empty input
/// yields an empty Vec (callers use `or vec!["".into()]`).
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() || width == 0 {
        return vec![];
    }
    let mut result = Vec::new();
    for input_line in text.split('\n') {
        wrap_single_line(input_line, width, &mut result);
    }
    result
}

fn append_word_to_line(
    word: &str,
    word_len: usize,
    width: usize,
    current: &mut String,
    current_len: &mut usize,
    out: &mut Vec<String>,
) {
    if word_len <= width {
        current.push_str(word);
        *current_len = word_len;
    } else {
        hard_split_word(word, width, current, current_len, out);
    }
}

fn wrap_single_line(line: &str, width: usize, out: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_len = 0usize;

    for word in line.split_whitespace() {
        let word_len = word.chars().count();

        if current_len == 0 {
            append_word_to_line(word, word_len, width, &mut current, &mut current_len, out);
            continue;
        }

        if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
            continue;
        }

        out.push(current.clone());
        current.clear();
        current_len = 0;
        append_word_to_line(word, word_len, width, &mut current, &mut current_len, out);
    }

    if !current.is_empty() || line.chars().next().is_none() {
        out.push(current);
    }
}

fn hard_split_word(
    word: &str,
    width: usize,
    current: &mut String,
    current_len: &mut usize,
    out: &mut Vec<String>,
) {
    let mut chars = word.chars();
    loop {
        let chunk: String = chars.by_ref().take(width).collect();
        if chunk.is_empty() {
            break;
        }
        let chunk_len = chunk.chars().count();
        if chunk_len < width {
            *current = chunk;
            *current_len = chunk_len;
            break;
        } else {
            out.push(chunk);
        }
    }
}

/// Parity: Python tui.py _comment_box.
///
/// Returns a box of lines each exactly `width` chars wide (char count).
/// If `width` < 4 returns an empty Vec.
/// The top border embeds ' {author} {MIDDOT} {when} ' padded/clipped to fit.
/// A header longer than inner-2 is clipped with ELLIPSIS + space before the tr corner.
/// Body lines are BOX_V + ljust(inner) + BOX_V. Bottom is BOX_BL + h*inner + BOX_BR.
pub fn comment_box(author: &str, when: &str, body: &str, width: usize) -> Vec<String> {
    if width < 4 {
        return vec![];
    }
    let inner = width - 2;
    let header_text = format!(" {} {} {} ", author, MIDDOT, when);
    let max_header = inner.saturating_sub(2);
    let header_chars: Vec<char> = header_text.chars().collect();
    let header_fitted = if header_chars.len() > max_header {
        let clipped: String = header_chars[..max_header.saturating_sub(1)]
            .iter()
            .collect();
        format!("{}{} ", clipped, ELLIPSIS)
    } else {
        header_text.clone()
    };
    let fitted_len = header_fitted.chars().count();
    let h_right = if inner > 1 + fitted_len {
        BOX_H.repeat(inner - 1 - fitted_len)
    } else {
        String::new()
    };
    let top = format!("{}{}{}{}{}", BOX_TL, BOX_H, header_fitted, h_right, BOX_TR);

    let body_lines = {
        let wrapped = wrap_text(body, inner);
        if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        }
    };

    let middle: Vec<String> = body_lines
        .iter()
        .map(|line| {
            let line_len = line.chars().count();
            let padding = inner.saturating_sub(line_len);
            format!("{}{}{}{}", BOX_V, line, " ".repeat(padding), BOX_V)
        })
        .collect();

    let bottom = format!("{}{}{}", BOX_BL, BOX_H.repeat(inner), BOX_BR);

    let mut result = vec![top];
    result.extend(middle);
    result.push(bottom);
    result
}

/// Parity: Python tui.py build_detail_lines.
///
/// Composes the full detail content at `inner_width`:
/// (1) meta section, (2) blank, (3) Description + wrapped body,
/// (4) blank, (5) comment boxes (blank-separated, blank-prefixed).
/// No line exceeds `inner_width` chars. Pure: no I/O, no async.
pub fn build_detail_lines(
    task: &Value,
    comments: &[Value],
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    build_meta_rows(task, user_map, inner_width, &mut lines);
    lines.push(String::new());
    build_description_rows(task, inner_width, &mut lines);
    if !comments.is_empty() {
        lines.push(String::new());
        build_comments_rows(comments, inner_width, &mut lines);
    }
    lines
}

fn build_meta_rows(
    task: &Value,
    user_map: &HashMap<i64, String>,
    inner_width: usize,
    lines: &mut Vec<String>,
) {
    let project_id = task.get("project_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let task_id = task.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let project_name = task
        .get("project_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let title = task.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let status = if task
        .get("is_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        t("Completed")
    } else {
        t("Open")
    };
    let assignee = match task.get("assignee_id").and_then(|v| v.as_i64()) {
        None => t("(unassigned)"),
        Some(id) => match user_map.get(&id) {
            Some(name) => format!("{name} ({id})"),
            None => format!("({id})"),
        },
    };

    let meta_entries: &[(&str, String)] = &[
        ("Task", format!("{}-{}", project_id, task_id)),
        ("Project", project_name),
        ("Title", title.to_string()),
        ("Status", status),
        ("Assignee", assignee),
    ];

    for (label, value) in meta_entries {
        push_truncated(lines, &format!("{}:  {}", t(label), value), inner_width);
    }

    let start = fmt_date(task.get("start_on").unwrap_or(&Value::Null));
    if !start.is_empty() {
        push_truncated(lines, &format!("{}:  {}", t("Start"), start), inner_width);
    }
    let due = fmt_date(task.get("due_on").unwrap_or(&Value::Null));
    if !due.is_empty() {
        push_truncated(lines, &format!("{}:  {}", t("Due"), due), inner_width);
    }

    push_truncated(
        lines,
        &format!(
            "{}:  {}h",
            t("Estimate"),
            fmt_hours(task.get("estimate").unwrap_or(&Value::Null))
        ),
        inner_width,
    );
    push_truncated(
        lines,
        &format!(
            "{}:  {}h",
            t("Logged"),
            fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null))
        ),
        inner_width,
    );
}

fn build_description_rows(task: &Value, inner_width: usize, lines: &mut Vec<String>) {
    push_truncated(lines, &format!("{}:", t("Description")), inner_width);
    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let text = html_to_text(body_html);
    if text.is_empty() {
        push_truncated(lines, &t("(no description)"), inner_width);
    } else {
        for wrapped in wrap_text(&text, inner_width) {
            lines.push(wrapped);
        }
    }
}

fn build_comments_rows(comments: &[Value], inner_width: usize, lines: &mut Vec<String>) {
    let mut first = true;
    for comment in comments {
        if !first {
            lines.push(String::new());
        }
        first = false;

        let author = extract_comment_author(comment);
        let when = fmt_ts(comment.get("created_on").unwrap_or(&Value::Null));
        let body = extract_comment_body(comment);

        for line in comment_box(&author, &when, &body, inner_width) {
            lines.push(line);
        }
    }
}

fn extract_comment_author(comment: &Value) -> String {
    comment
        .get("created_by_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            comment
                .get("created_by_id")
                .map(|v| v.to_string())
                .filter(|s| !s.is_empty() && s != "null")
        })
        .unwrap_or_else(|| t("(unknown)"))
}

fn extract_comment_body(comment: &Value) -> String {
    let plain = comment
        .get("body_plain_text")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    match plain {
        Some(s) => s.to_string(),
        None => {
            let html = comment.get("body").and_then(|v| v.as_str()).unwrap_or("");
            html_to_text(html)
        }
    }
}

fn push_truncated(lines: &mut Vec<String>, s: &str, max_width: usize) {
    if max_width == 0 {
        lines.push(String::new());
        return;
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        lines.push(s.to_string());
    } else {
        lines.push(chars[..max_width].iter().collect());
    }
}

/// Returns `s` truncated to exactly `max_width` chars.
///
/// When `s` fits within `max_width`, it is returned unchanged. When it is longer,
/// the result is the first `max_width - 1` chars followed by "\u{2026}" so the
/// caller's column stays at exactly `max_width` chars. Edge cases: `max_width == 0`
/// returns an empty string; `max_width == 1` returns "\u{2026}".
pub fn truncate_cell(s: &str, max_width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let prefix: String = s.chars().take(max_width - 1).collect();
    format!("{}\u{2026}", prefix)
}

#[cfg(test)]
#[path = "../tests/unit/render.rs"]
mod tests;
