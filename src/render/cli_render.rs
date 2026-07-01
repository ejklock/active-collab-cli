//! Non-TTY CLI string output adapter (ADR 0049).
//!
//! Owns the `render_*_to_str` family and `html_to_text`. These produce plain
//! `String`s for the CLI's non-interactive commands; the byte-for-byte parity
//! contract with the legacy Python `render.py` is unchanged by this module split.

use crate::i18n::t;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

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

    let start = super::fmt_date(task.get("start_on").unwrap_or(&Value::Null));
    if !start.is_empty() {
        lines.push(format!("{}:     {}", t("Start"), start));
    }

    let due = super::fmt_date(task.get("due_on").unwrap_or(&Value::Null));
    if !due.is_empty() {
        lines.push(format!("{}:       {}", t("Due"), due));
    }

    lines.push(format!(
        "{}:  {}h",
        t("Estimate"),
        super::fmt_hours(task.get("estimate").unwrap_or(&Value::Null))
    ));
    lines.push(format!(
        "{}:    {}h",
        t("Logged"),
        super::fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null))
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

        let created = super::fmt_ts(c.get("created_on").unwrap_or(&Value::Null));

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
