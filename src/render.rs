use crate::i18n::t;
use crate::store::secs_to_utc_parts;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;
use unicode_width::UnicodeWidthStr;

/// A contiguous segment of a rendered panel body line, tagged by whether it is a URL.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkSegment {
    pub text: String,
    pub is_link: bool,
}

fn body_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https?://[^\s]+|www\.[^\s]+").expect("body_url_re is a valid pattern")
    })
}

fn link_label_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\u{2197} Link \d+").expect("link_label_re is a valid pattern"))
}

/// Accumulates URLs found in body text during URL-to-label replacement.
///
/// `next_index` is 1-based and incremented per matched URL. `urls` holds the
/// original URL strings in the order they were encountered — index N-1 in this
/// vec corresponds to label "↗ Link N".
pub struct LinkCollector {
    pub next_index: usize,
    pub urls: Vec<String>,
}

impl LinkCollector {
    pub fn new() -> Self {
        LinkCollector {
            next_index: 1,
            urls: Vec::new(),
        }
    }
}

/// Replace each URL in `text` with a short sequential label and collect the original URLs.
///
/// Each URL matched by the body_url_re (https?:// or www.) is replaced with the label
/// `"↗ Link N"` where N comes from `collector.next_index` (incremented per match).
/// The matched URL is pushed into `collector.urls` in the same order. This ensures a
/// 1:1 mapping between label index and collected URL position.
///
/// Text with no URLs is returned unchanged and the collector is unmodified.
pub fn replace_urls_with_labels(text: &str, collector: &mut LinkCollector) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_byte = 0usize;

    for m in body_url_re().find_iter(text) {
        result.push_str(&text[last_byte..m.start()]);
        result.push_str(&format!("\u{2197} Link {}", collector.next_index));
        collector.urls.push(m.as_str().to_string());
        collector.next_index += 1;
        last_byte = m.end();
    }

    result.push_str(&text[last_byte..]);
    result
}

/// Produced by `build_detail_content`: the rendered lines and the collected URLs.
///
/// `lines` is identical to what `build_detail_lines` returns, except every URL in
/// the description and comment bodies has been replaced with a "↗ Link N" label.
/// `links` holds the original URLs in the order the labels were assigned — element
/// at index N-1 corresponds to label "↗ Link N".
pub struct DetailContent {
    pub lines: Vec<String>,
    pub links: Vec<String>,
}

/// Map a display column position to a "↗ Link N" label index within `line`.
///
/// Walks `line` character-by-character, accumulating display-column offsets via
/// unicode-width (the same metric ratatui uses for layout). For every "↗ Link <N>"
/// match found by `link_label_re`, the function tests whether `target_col` falls
/// inside the label's [col_start, col_end) display-column span and returns the
/// parsed N (1-based, as `usize`) for the first match that contains it.
///
/// Returns `None` when `target_col` lies in border, padding, plain text, or a
/// label match whose N cannot be parsed. The function is char-boundary-safe and
/// pure (no I/O, no async, no time access).
pub fn link_index_at(line: &str, target_col: usize) -> Option<usize> {
    let re = link_label_re();
    for m in re.find_iter(line) {
        let col_start = display_col_of_byte(line, m.start());
        let label_width = display_width(m.as_str());
        let col_end = col_start + label_width;
        if target_col >= col_start && target_col < col_end {
            return parse_link_number(m.as_str());
        }
    }
    None
}

/// Compute the display-column offset of the character at `byte_pos` in `s`.
///
/// Walks all characters before `byte_pos`, summing their display widths.
/// Callers must pass a valid char boundary — the function stops at the first
/// character whose start byte equals or exceeds `byte_pos`.
fn display_col_of_byte(s: &str, byte_pos: usize) -> usize {
    let mut col = 0usize;
    for (start, ch) in s.char_indices() {
        if start >= byte_pos {
            break;
        }
        col += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
    }
    col
}

/// Extract the numeric N from a "↗ Link N" match string.
///
/// Returns `None` when the suffix after "Link " is not a valid positive integer.
fn parse_link_number(label: &str) -> Option<usize> {
    let prefix = "\u{2197} Link ";
    let digits = label.strip_prefix(prefix)?;
    digits.parse::<usize>().ok()
}

/// Split a single rendered panel body line into ordered `LinkSegment`s.
///
/// URL substrings (matched by `https?://…` or `www.…`, stopped at whitespace) are
/// tagged `is_link: true`. Link-label substrings (`↗ Link <digits>`) produced by
/// `replace_urls_with_labels` are also tagged `is_link: true`. Everything else
/// (including leading/trailing `│` border chars and padding spaces) is tagged
/// `is_link: false`. When the line contains no URL or label the function returns a
/// single non-link segment. Slicing is char-boundary-safe for UTF-8.
pub fn link_segments(line: &str) -> Vec<LinkSegment> {
    let mut segments = Vec::new();
    let mut last_byte = 0usize;

    let url_re = body_url_re();
    let label_re = link_label_re();

    let mut matches: Vec<(usize, usize)> = url_re
        .find_iter(line)
        .map(|m| (m.start(), m.end()))
        .chain(label_re.find_iter(line).map(|m| (m.start(), m.end())))
        .collect();
    matches.sort_by_key(|(start, _)| *start);

    for (start, end) in matches {
        if start < last_byte {
            continue;
        }
        if start > last_byte {
            segments.push(LinkSegment {
                text: line[last_byte..start].to_string(),
                is_link: false,
            });
        }
        segments.push(LinkSegment {
            text: line[start..end].to_string(),
            is_link: true,
        });
        last_byte = end;
    }

    if last_byte < line.len() {
        segments.push(LinkSegment {
            text: line[last_byte..].to_string(),
            is_link: false,
        });
    }

    if segments.is_empty() {
        segments.push(LinkSegment {
            text: line.to_string(),
            is_link: false,
        });
    }

    segments
}

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

/// Returns true when `name` looks like a real filename: non-empty, at most 48
/// chars, and ends with a dot followed by 1–6 ASCII-alphanumeric characters.
fn looks_like_filename(name: &str) -> bool {
    if name.is_empty() || name.chars().count() > 48 {
        return false;
    }
    let last_dot = match name.rfind('.') {
        Some(pos) => pos,
        None => return false,
    };
    let ext = &name[last_dot + 1..];
    !ext.is_empty() && ext.len() <= 6 && ext.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Returns the display line for a single asset entry in the Artifacts panel.
///
/// Format: `"[{index}] \u{2197} {label}"` where `label` is `asset.name` when it
/// looks like a real filename, otherwise the locale-aware "Open link" fallback.
pub fn asset_link_line(index: usize, asset: &Asset) -> String {
    let label = if looks_like_filename(&asset.name) {
        asset.name.clone()
    } else {
        t("Open link")
    };
    format!("[{}] \u{2197} {}", index, label)
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
const PANEL_HPAD: usize = 1;
const PANEL_VPAD: usize = 1;

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn fit_to_display_width(s: &str, cols: usize) -> String {
    let w = display_width(s);
    if w <= cols {
        let padding = cols - w;
        let mut out = s.to_string();
        for _ in 0..padding {
            out.push(' ');
        }
        return out;
    }
    let mut acc = 0usize;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > cols {
            break;
        }
        result.push(ch);
        acc += cw;
    }
    let padding = cols - acc;
    for _ in 0..padding {
        result.push(' ');
    }
    result
}

fn panel_content_width(width: usize) -> usize {
    width.saturating_sub(2 + 2 * PANEL_HPAD)
}

/// Parity: Python tui.py wrap_text.
///
/// Greedy word-wrap on whitespace to at most `width` DISPLAY columns per line.
/// Display width is measured via unicode-width (same crate ratatui uses), so CJK and
/// combining characters are handled correctly. A single word wider than `width` is
/// hard-split by accumulated display width. Existing line breaks are preserved —
/// each input line is wrapped independently. Empty input yields an empty Vec.
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
    word_dw: usize,
    width: usize,
    current: &mut String,
    current_dw: &mut usize,
    out: &mut Vec<String>,
) {
    if word_dw <= width {
        current.push_str(word);
        *current_dw = word_dw;
    } else {
        hard_split_word(word, width, current, current_dw, out);
    }
}

fn wrap_single_line(line: &str, width: usize, out: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_dw = 0usize;

    for word in line.split_whitespace() {
        let word_dw = display_width(word);

        if current_dw == 0 {
            append_word_to_line(word, word_dw, width, &mut current, &mut current_dw, out);
            continue;
        }

        if current_dw + 1 + word_dw <= width {
            current.push(' ');
            current.push_str(word);
            current_dw += 1 + word_dw;
            continue;
        }

        out.push(current.clone());
        current.clear();
        current_dw = 0;
        append_word_to_line(word, word_dw, width, &mut current, &mut current_dw, out);
    }

    if !current.is_empty() || line.chars().next().is_none() {
        out.push(current);
    }
}

fn hard_split_word(
    word: &str,
    width: usize,
    current: &mut String,
    current_dw: &mut usize,
    out: &mut Vec<String>,
) {
    let mut acc = 0usize;
    let mut chunk = String::new();
    for ch in word.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > width {
            out.push(chunk.clone());
            chunk.clear();
            acc = 0;
        }
        chunk.push(ch);
        acc += cw;
    }
    *current = chunk;
    *current_dw = acc;
}

/// Single rounded-box primitive used by all section panels and comment cards.
///
/// Draws a rounded box of exactly `width` DISPLAY columns per line. Returns `vec![]`
/// when `width` < 4. Adds PANEL_HPAD spaces between each vertical border and content,
/// and PANEL_VPAD blank rows at the top and bottom of the body. The top border embeds
/// ` {label} ` clipped with ELLIPSIS when it does not fit. Every returned line is
/// exactly `width` display columns wide (proven by fit_to_display_width for body lines).
pub fn panel_box(label: &str, inner_lines: &[String], width: usize) -> Vec<String> {
    if width < 4 {
        return vec![];
    }
    let inner = width - 2;
    let content_width = panel_content_width(width);
    let hpad = " ".repeat(PANEL_HPAD);

    let header_text = format!(" {} ", label);
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
    let fitted_dw = display_width(&header_fitted);
    let h_right = if inner > 1 + fitted_dw {
        BOX_H.repeat(inner - 1 - fitted_dw)
    } else {
        String::new()
    };
    let top = format!("{}{}{}{}{}", BOX_TL, BOX_H, header_fitted, h_right, BOX_TR);

    let blank_body_line = format!("{}{}{}", BOX_V, " ".repeat(inner), BOX_V);

    let mut middle: Vec<String> = Vec::new();
    for _ in 0..PANEL_VPAD {
        middle.push(blank_body_line.clone());
    }
    for line in inner_lines {
        let fitted = fit_to_display_width(line, content_width);
        middle.push(format!("{}{}{}{}{}", BOX_V, hpad, fitted, hpad, BOX_V));
    }
    for _ in 0..PANEL_VPAD {
        middle.push(blank_body_line.clone());
    }

    let bottom = format!("{}{}{}", BOX_BL, BOX_H.repeat(inner), BOX_BR);

    let mut result = vec![top];
    result.extend(middle);
    result.push(bottom);
    result
}

/// Parity: Python tui.py _comment_box.
///
/// Delegates to panel_box with label = "{author} {MIDDOT} {when}".
/// Body is word-wrapped to panel_content_width(width) by display columns.
pub fn comment_box(author: &str, when: &str, body: &str, width: usize) -> Vec<String> {
    let label = format!("{} {} {}", author, MIDDOT, when);
    let body_lines = {
        let wrapped = wrap_text(body, panel_content_width(width));
        if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        }
    };
    panel_box(&label, &body_lines, width)
}

/// Returns the Details panel for a task detail view (a rounded panel with 2-column meta table).
///
/// The Title row is omitted — the task name appears in the title band above.
/// Every produced line is <= `inner_width` chars.
pub fn build_header_lines(
    task: &Value,
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> Vec<String> {
    let meta_rows = build_meta_table_rows(task, user_map, inner_width);
    panel_box(&t("Details"), &meta_rows, inner_width)
}

/// Returns the Description panel for a task detail view (a rounded panel with wrapped body).
///
/// Falls back to `(no description)` when body is empty. Every line is <= `inner_width` chars.
#[allow(dead_code)]
pub fn build_body_lines(task: &Value, inner_width: usize) -> Vec<String> {
    let mut collector = LinkCollector::new();
    build_body_lines_with_collector(task, inner_width, &mut collector)
}

fn build_body_lines_with_collector(
    task: &Value,
    inner_width: usize,
    collector: &mut LinkCollector,
) -> Vec<String> {
    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let text = html_to_text(body_html);
    let transformed = replace_urls_with_labels(&text, collector);
    let content_width = panel_content_width(inner_width);
    let body_rows = if transformed.is_empty() {
        vec![t("(no description)")]
    } else {
        let wrapped = wrap_text(&transformed, content_width);
        if wrapped.is_empty() {
            vec![t("(no description)")]
        } else {
            wrapped
        }
    };
    panel_box(&t("Description"), &body_rows, inner_width)
}

/// Returns the Comments panel containing nested per-comment cards, or empty when no comments.
///
/// When comments are present, wraps all comment cards in an outer panel labelled
/// "Comments (N)". Each inner card is indented by one space and sized to fit the
/// outer panel content area. Every line is <= `inner_width` chars.
#[allow(dead_code)]
pub fn build_comment_lines(comments: &[Value], inner_width: usize) -> Vec<String> {
    let mut collector = LinkCollector::new();
    build_comment_lines_with_collector(comments, inner_width, &mut collector)
}

fn build_comment_lines_with_collector(
    comments: &[Value],
    inner_width: usize,
    collector: &mut LinkCollector,
) -> Vec<String> {
    if comments.is_empty() {
        return vec![];
    }
    let outer_content = panel_content_width(inner_width);
    let card_width = outer_content.saturating_sub(1);
    let mut nested: Vec<String> = Vec::new();
    let mut first = true;
    for comment in comments {
        if !first {
            nested.push(String::new());
        }
        first = false;

        let author = extract_comment_author(comment);
        let when = fmt_ts(comment.get("created_on").unwrap_or(&Value::Null));
        let raw_body = extract_comment_body(comment);
        let body = replace_urls_with_labels(&raw_body, collector);

        for line in comment_box(&author, &when, &body, card_width) {
            nested.push(format!(" {}", line));
        }
    }

    let label = format!("{} ({})", t("Comments"), comments.len());
    panel_box(&label, &nested, inner_width)
}

/// Parity: Python tui.py build_detail_lines.
///
/// Composes the full detail content at `inner_width`:
/// (1) Details panel, (2) blank, (3) Description panel,
/// (4) blank + Comments panel when comments non-empty.
/// The task name is promoted to the frame border title in the TUI layer.
/// No line exceeds `inner_width` chars. Pure: no I/O, no async.
///
/// Backward-compatible wrapper over `build_detail_content` that returns only
/// the rendered lines; existing test callers stay green with no code changes.
#[allow(dead_code)]
pub fn build_detail_lines(
    task: &Value,
    comments: &[Value],
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> Vec<String> {
    build_detail_content(task, comments, user_map, inner_width).lines
}

/// Build the full detail content, replacing every URL in the description and comment
/// bodies with a sequential "↗ Link N" label and collecting the original URLs.
///
/// URL numbering is global: description URLs are assigned indices first, then
/// comment bodies in order. Each label maps 1:1 to the URL at `links[N-1]`.
/// Pure: no I/O, no async, no time access.
pub fn build_detail_content(
    task: &Value,
    comments: &[Value],
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> DetailContent {
    let mut collector = LinkCollector::new();
    let mut lines = vec![];
    lines.extend(build_header_lines(task, user_map, inner_width));
    lines.push(String::new());
    lines.extend(build_body_lines_with_collector(
        task,
        inner_width,
        &mut collector,
    ));
    if !comments.is_empty() {
        lines.push(String::new());
        lines.extend(build_comment_lines_with_collector(
            comments,
            inner_width,
            &mut collector,
        ));
    }
    DetailContent {
        lines,
        links: collector.urls,
    }
}

/// Builds the 2-column aligned meta table rows for the Details panel.
///
/// Pairs are (translated_label, value). The label column width is the max char count
/// among the present translated labels. Each row is formatted as
/// `"{label:<width$}  {value}"` then truncated to fit inside the panel inner area.
fn build_meta_table_rows(
    task: &Value,
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> Vec<String> {
    let project_id = task.get("project_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let task_id = task.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let project_name = task
        .get("project_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
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

    let task_label = t("Task");
    let project_label = t("Project");
    let status_label = t("Status");
    let assignee_label = t("Assignee");
    let estimate_label = t("Estimate");
    let logged_label = t("Logged");

    let start = fmt_date(task.get("start_on").unwrap_or(&Value::Null));
    let due = fmt_date(task.get("due_on").unwrap_or(&Value::Null));

    let start_label = t("Start");
    let due_label = t("Due");

    // Compute max label width among all present labels
    let mut label_col = [
        task_label.chars().count(),
        project_label.chars().count(),
        status_label.chars().count(),
        assignee_label.chars().count(),
        estimate_label.chars().count(),
        logged_label.chars().count(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);

    if !start.is_empty() {
        label_col = label_col.max(start_label.chars().count());
    }
    if !due.is_empty() {
        label_col = label_col.max(due_label.chars().count());
    }

    let mut pairs: Vec<(String, String)> = vec![
        (task_label, format!("{}-{}", project_id, task_id)),
        (project_label, project_name),
        (status_label, status),
        (assignee_label, assignee),
    ];

    if !start.is_empty() {
        pairs.push((start_label, start));
    }
    if !due.is_empty() {
        pairs.push((due_label, due));
    }

    pairs.push((
        estimate_label,
        format!(
            "{}h",
            fmt_hours(task.get("estimate").unwrap_or(&Value::Null))
        ),
    ));
    pairs.push((
        logged_label,
        format!(
            "{}h",
            fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null))
        ),
    ));

    let content_width = panel_content_width(inner_width);
    pairs
        .into_iter()
        .map(|(lbl, val)| {
            let row = format!("{:<width$}  {}", lbl, val, width = label_col);
            truncate_cell(&row, content_width)
        })
        .collect()
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
