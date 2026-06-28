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
        Regex::new(r"https?://[^\s\]]+|www\.[^\s\]]+").expect("body_url_re is a valid pattern")
    })
}

/// Retained for callers that thread it through the richtext parser.
///
/// The `urls` vec is no longer populated (inline rendering makes the collector
/// unnecessary for body links). It exists so call sites do not require a
/// signature change in this slice.
#[allow(dead_code)]
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

/// An emphasis-style run on a single display-column slice of a rendered line.
///
/// `start` and `len` are DISPLAY COLUMNS (not byte offsets) in the final boxed
/// line string, offset to include the left chrome (border + padding). Only
/// emphasis kinds (Bold/Italic/Code) are tracked here; link color is handled
/// separately by the `link_segments` path.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRun {
    pub start: usize,
    pub len: usize,
    pub style: crate::richtext::RichStyle,
}

/// Produced by `build_detail_content`: the rendered lines and style runs.
///
/// `lines` holds the full detail layout with real URLs inline as `text [url]`.
/// `line_styles` is a parallel channel: `line_styles[i]` holds the `StyleRun`s for
/// `lines[i]`. Both vecs are always the same length.
pub struct DetailContent {
    pub lines: Vec<String>,
    pub line_styles: Vec<Vec<StyleRun>>,
}

/// Strip the panel-box left border and HPAD from a boxed content line.
///
/// A box content line has the form `│ {content} │` where `│` is U+2502 (3 bytes)
/// and the surrounding space is PANEL_HPAD (1 byte each side). Returns the inner
/// content slice `{content}` (including any trailing-space padding added by
/// `fit_to_display_width`), or `None` when the string is not a box content line
/// (e.g. a rounded-corner border row starting with `╭` or `╰`).
fn box_inner_content(s: &str) -> Option<&str> {
    const PREFIX_BYTES: usize = 4; // U+2502 = 3 UTF-8 bytes, then one HPAD space
    const SUFFIX_BYTES: usize = 4; // one HPAD space, then U+2502 = 3 UTF-8 bytes
    if !s.starts_with('\u{2502}') {
        return None;
    }
    let len = s.len();
    if len < PREFIX_BYTES + SUFFIX_BYTES {
        return None;
    }
    Some(&s[PREFIX_BYTES..len - SUFFIX_BYTES])
}

/// Public accessor for `box_inner_content` used by callers outside this module.
///
/// Returns the inner content of a box line (stripping `│ ` prefix and ` │` suffix),
/// or `None` when `s` is not a box content line.
pub fn box_inner_content_pub(s: &str) -> Option<&str> {
    box_inner_content(s)
}

/// Display columns consumed by the panel-box left chrome: the `│` border (1 col) plus
/// `PANEL_HPAD` (1 space), giving 2 total.
///
/// This is the panel-chrome term only. The full absolute-frame → inner-content left
/// offset also includes the ratatui `Block::borders(ALL)` left border (1 col) drawn by
/// `render_content`; that additional column is `DETAIL_CONTENT_BLOCK_BORDER_COLS` in
/// `model.rs`. Add both terms when converting an absolute frame column to an
/// inner-content column.
pub const BODY_LEFT_CHROME_COLS: usize = 1 + PANEL_HPAD;

/// Map a clicked (line_idx, char_col) to the start of its logical wrap group and the
/// column within the joined group content.
///
/// When `wrap_rich` hard-splits a URL token across multiple content lines, every line
/// except the last fills the entire `content_width` with non-space characters (no
/// trailing-space padding). This function walks backward from `line_idx` to find the
/// first line of such a group, then computes the display-column offset within the
/// concatenated inner content.
///
/// `char_col` is the display-column index into the full boxed string (0 = the `│`
/// border character). Content starts at display-column 2 (border + HPAD).
///
/// Returns `(group_start_idx, logical_col)` where `group_start_idx` is the index into
/// `lines` of the first line of the group and `logical_col` is the column within the
/// concatenated inner content (trailing-space padding stripped from each fragment).
/// Returns `None` when `line_idx` is out of range or not a box content line.
pub fn logical_position_in_wrap_group(
    lines: &[String],
    line_idx: usize,
    char_col: usize,
    content_width: usize,
) -> Option<(usize, usize)> {
    box_inner_content(lines.get(line_idx)?)?;

    let mut group_start = line_idx;
    while group_start > 0 {
        let prev_inner = match box_inner_content(&lines[group_start - 1]) {
            Some(c) => c,
            None => break,
        };
        let trimmed_dw = display_width(prev_inner.trim_end_matches(' '));
        if trimmed_dw < content_width {
            break;
        }
        group_start -= 1;
    }

    let content_col = char_col.saturating_sub(2);
    let mut logical_col = 0usize;
    for line in lines.iter().take(line_idx).skip(group_start) {
        let inner = box_inner_content(line)?;
        logical_col += display_width(inner.trim_end_matches(' '));
    }
    logical_col += content_col;

    Some((group_start, logical_col))
}

/// Join the inner content of consecutive hard-split lines starting at `group_start`
/// and run `url_at` on the joined string at `logical_col`.
///
/// A line is considered hard-split (has a continuation on the next line) when its
/// inner content, after trimming trailing spaces, fills the entire `content_width`.
/// Joining stops at the first naturally-ended line or after 20 continuations.
fn url_at_in_wrap_group(
    lines: &[String],
    group_start: usize,
    logical_col: usize,
    content_width: usize,
) -> Option<String> {
    let mut joined = String::new();
    let mut i = group_start;
    loop {
        let inner = box_inner_content(lines.get(i)?)?;
        let trimmed = inner.trim_end_matches(' ');
        let is_hard_split = display_width(trimmed) >= content_width;
        joined.push_str(trimmed);
        if !is_hard_split || i >= lines.len().saturating_sub(1) || i >= group_start + 20 {
            break;
        }
        i += 1;
    }
    url_at(&joined, logical_col)
}

/// Resolve the URL at a click that may land on a wrapped fragment of a `[url]` token.
///
/// When `wrap_rich` hard-splits a long `[url]` token across multiple rendered lines,
/// `url_at` on a single fragment returns an incomplete URL or `None`. This function
/// detects the wrap group (consecutive full-width lines), joins their inner content,
/// and runs `url_at` on the reconstructed logical line so any fragment of a split URL
/// resolves to the complete URL.
///
/// For single-line (unwrapped) tokens the function falls through to a direct `url_at`
/// call on the clicked boxed string, preserving the V5 behavior.
///
/// `char_col` is the display-column index into the full boxed string (0 = the `│`
/// border character). `content_width` must equal `panel_content_width(inner_width)`
/// — the same value used when wrapping the body text.
pub fn resolve_wrapped_url(
    lines: &[String],
    line_idx: usize,
    char_col: usize,
    content_width: usize,
) -> Option<String> {
    let (group_start, logical_col) =
        logical_position_in_wrap_group(lines, line_idx, char_col, content_width)?;

    let clicked_inner = box_inner_content(&lines[line_idx])?;
    let clicked_trimmed_dw = display_width(clicked_inner.trim_end_matches(' '));
    let is_continuation = group_start < line_idx;
    let is_hard_split_source = clicked_trimmed_dw >= content_width;

    if is_continuation || is_hard_split_source {
        return url_at_in_wrap_group(lines, group_start, logical_col, content_width);
    }

    url_at(&lines[line_idx], char_col)
}

/// Resolve the URL at `target_col` (display-column) within a rendered body line.
///
/// Algorithm (in priority order):
/// 1. Scan for `[INNER]` bracketed tokens. When `target_col` falls within the INNER
///    span and INNER validates as a URL or bare email, return INNER (without brackets).
///    A `[plain note]` that is not a URL/email is never returned.
/// 2. Scan for raw URLs via `body_url_re`. When `target_col` is within a match,
///    return the matched URL string.
/// 3. Otherwise return `None`.
///
/// The returned string never contains the surrounding `[` and `]` brackets.
/// The function is char-boundary-safe and pure (no I/O, no async, no time access).
pub fn url_at(line: &str, target_col: usize) -> Option<String> {
    if let Some(url) = bracketed_url_at(line, target_col) {
        return Some(url);
    }
    raw_url_at(line, target_col)
}

/// Scan `[INNER]` tokens in `line`; return INNER when `target_col` is within the
/// inner span and INNER is a URL or bare e-mail.
fn bracketed_url_at(line: &str, target_col: usize) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\[([^\[\]]+)\]").expect("bracketed token re is a valid pattern")
    });
    for cap in re.captures_iter(line) {
        let full = cap.get(0)?;
        let inner = cap.get(1)?;
        let inner_start_col = display_col_of_byte(line, inner.start());
        let inner_end_col = inner_start_col + display_width(inner.as_str());
        if target_col < inner_start_col || target_col >= inner_end_col {
            continue;
        }
        let inner_str = inner.as_str();
        if is_url_or_email(inner_str) {
            return Some(inner_str.to_string());
        }
        let _ = full;
    }
    None
}

/// Return true when `s` is a URL (http/https/www) or a bare e-mail address.
fn is_url_or_email(s: &str) -> bool {
    body_url_re().is_match(s) || is_bare_email(s)
}

/// Return true when `s` looks like a bare e-mail address (contains `@`, no scheme).
fn is_bare_email(s: &str) -> bool {
    s.contains('@') && !s.contains("://")
}

/// Scan raw URL matches in `line`; return the URL when `target_col` is within a match.
fn raw_url_at(line: &str, target_col: usize) -> Option<String> {
    for m in body_url_re().find_iter(line) {
        let col_start = display_col_of_byte(line, m.start());
        let col_end = col_start + display_width(m.as_str());
        if target_col >= col_start && target_col < col_end {
            return Some(m.as_str().to_string());
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

/// Split a single rendered panel body line into ordered `LinkSegment`s.
///
/// For `[INNER]` bracketed tokens where INNER is a URL or bare email, the INNER
/// span is tagged `is_link: true` while the surrounding `[` and `]` remain
/// `is_link: false`. Raw URL substrings (`https?://…` or `www.…`) are also tagged
/// `is_link: true`. Everything else — border chars, padding, plain text, and
/// non-URL `[note]` tokens — is `is_link: false`. The function never includes a
/// trailing `]` inside a link segment. Slicing is char-boundary-safe for UTF-8.
pub fn link_segments(line: &str) -> Vec<LinkSegment> {
    let link_spans = collect_link_spans(line);
    build_segments_from_spans(line, &link_spans)
}

/// A byte-range `[start, end)` within a string that should be tagged `is_link`.
struct LinkSpan {
    start: usize,
    end: usize,
}

/// Collect all link byte-spans in `line` in sorted order.
///
/// For `[INNER]` tokens whose INNER is a URL/email, the span covers INNER only
/// (not the brackets). For raw URLs, the span covers the full match.
fn collect_link_spans(line: &str) -> Vec<LinkSpan> {
    static BRACKET_RE: OnceLock<Regex> = OnceLock::new();
    let bracket_re = BRACKET_RE
        .get_or_init(|| Regex::new(r"\[([^\[\]]+)\]").expect("bracket_re is a valid pattern"));

    let mut spans: Vec<LinkSpan> = Vec::new();
    let mut bracket_ranges: Vec<(usize, usize)> = Vec::new();

    for cap in bracket_re.captures_iter(line) {
        if let (Some(full), Some(inner)) = (cap.get(0), cap.get(1)) {
            if is_url_or_email(inner.as_str()) {
                spans.push(LinkSpan {
                    start: inner.start(),
                    end: inner.end(),
                });
                bracket_ranges.push((full.start(), full.end()));
            }
        }
    }

    for m in body_url_re().find_iter(line) {
        let covered = bracket_ranges
            .iter()
            .any(|(bs, be)| m.start() >= *bs && m.end() <= *be);
        if !covered {
            spans.push(LinkSpan {
                start: m.start(),
                end: m.end(),
            });
        }
    }

    spans.sort_by_key(|s| s.start);
    spans
}

/// Build a `Vec<LinkSegment>` by slicing `line` around the given sorted `spans`.
fn build_segments_from_spans(line: &str, spans: &[LinkSpan]) -> Vec<LinkSegment> {
    let mut segments: Vec<LinkSegment> = Vec::new();
    let mut last_byte = 0usize;

    for span in spans {
        if span.start < last_byte {
            continue;
        }
        if span.start > last_byte {
            segments.push(LinkSegment {
                text: line[last_byte..span.start].to_string(),
                is_link: false,
            });
        }
        segments.push(LinkSegment {
            text: line[span.start..span.end].to_string(),
            is_link: true,
        });
        last_byte = span.end;
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

/// Returns true when `name` looks like a real downloadable filename.
///
/// Requirements: non-empty, at most 48 chars, no `?`/`=`/`&` anywhere (rejects query
/// tails), and ends with a dot followed by 2–6 ASCII-alphabetic characters (rejects
/// purely-numeric extensions like `.0` or `.123` and mixed alphanumeric like `.tar123`).
pub(crate) fn looks_like_filename(name: &str) -> bool {
    if name.is_empty() || name.chars().count() > 48 {
        return false;
    }
    if name.contains('?') || name.contains('=') || name.contains('&') {
        return false;
    }
    let last_dot = match name.rfind('.') {
        Some(pos) => pos,
        None => return false,
    };
    let ext = &name[last_dot + 1..];
    ext.len() >= 2 && ext.len() <= 6 && ext.chars().all(|c| c.is_ascii_alphabetic())
}

/// Returns the display line for a single asset entry in the Artifacts panel.
///
/// Format: `"[{index}] \u{2197} {label}"` where `label` is `asset.name` when
/// non-empty (already fully derived by the controller), otherwise the locale-aware
/// "Open link" fallback.
pub fn asset_link_line(index: usize, asset: &Asset) -> String {
    let label = if asset.name.is_empty() {
        t("Open link")
    } else {
        asset.name.clone()
    };
    format!("[{}] \u{2197} {}", index, label)
}

/// Render an asset row as one or more wrapped lines for the Artifacts panel.
///
/// The prefix `"[{index}] ↗ "` is fixed; the label wraps to `width` display
/// columns with a hanging indent under the label start so continuation lines
/// align with the label text.  Single-line rows (when the full row fits) return
/// exactly one element equal to `asset_link_line(index, asset)`.
pub(crate) fn asset_row_lines(index: usize, asset: &Asset, width: usize) -> Vec<String> {
    let full = asset_link_line(index, asset);
    if width == 0 || display_width(&full) <= width {
        return vec![full];
    }
    let prefix = format!("[{}] \u{2197} ", index);
    let label = &full[prefix.len()..];
    format_asset_row(&prefix, label, width)
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MineTableRow {
    pub instance: String,
    pub project_id: i64,
    pub task_number: i64,
    pub task_id: i64,
    pub name: String,
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
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

/// Normalize a raw API `due_on` value to a `YYYY-MM-DD` string.
///
/// Accepts the same shapes as `fmt_date` (unix-timestamp number or ISO string).
/// Returns `None` when the value is null, missing, or unparseable.
pub fn normalize_due_to_ymd(value: &Value) -> Option<String> {
    let s = fmt_date(value);
    if s.is_empty() {
        None
    } else {
        Some(s)
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
pub(crate) const PANEL_HPAD: usize = 1;
pub(crate) const PANEL_VPAD: usize = 1;

pub(crate) fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Return the substring of `s` that occupies the half-open DISPLAY-column window
/// `[start_col, end_col)`.
///
/// Double-width chars (emoji, CJK) are never split: a char whose first display
/// column falls within the window is included in full. `end_col` is clamped to
/// `display_width(s)` so a window past the end simply returns the tail of the
/// string. An empty window (`start_col >= end_col`) returns an empty string.
///
/// This is the single source of truth for mapping display columns to char
/// boundaries. It must be used wherever a display-column range is converted to a
/// text slice — never treat a display column as a char index.
pub fn slice_by_display_cols(s: &str, start_col: usize, end_col: usize) -> String {
    if start_col >= end_col {
        return String::new();
    }
    let end_col = end_col.min(display_width(s));
    if start_col >= end_col {
        return String::new();
    }
    let mut result = String::new();
    let mut acc = 0usize;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc >= end_col {
            break;
        }
        if acc >= start_col {
            result.push(ch);
        }
        acc += cw;
    }
    result
}

/// Format a prefixed asset row with a hanging indent on continuation lines.
///
/// `prefix` is the fixed part (e.g. `"[1] ↗ "`), `label` is the potentially
/// long text that follows.  The first output line is `"{prefix}{label_start}"`;
/// continuation lines are indented by `display_width(prefix)` spaces so the
/// label text aligns under itself.  When everything fits in `width`, exactly
/// one line is returned.  `width == 0` returns `[prefix.to_string()]`.
pub(crate) fn format_asset_row(prefix: &str, label: &str, width: usize) -> Vec<String> {
    let prefix_dw = display_width(prefix);
    if width == 0 || prefix_dw >= width {
        return vec![format!("{prefix}{label}")];
    }
    let label_width = width.saturating_sub(prefix_dw);
    let label_lines = wrap_text(label, label_width);
    let pad = " ".repeat(prefix_dw);
    let mut result: Vec<String> = Vec::new();
    match label_lines.as_slice() {
        [] => result.push(prefix.to_string()),
        [first] => result.push(format!("{prefix}{first}")),
        [first, rest @ ..] => {
            result.push(format!("{prefix}{first}"));
            for cont in rest {
                result.push(format!("{pad}{cont}"));
            }
        }
    }
    result
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

/// Public accessor for `panel_content_width` used by callers outside this module.
///
/// Returns the number of display columns available for content inside a panel box
/// of `outer_width` columns (removes 2 border columns and 2×PANEL_HPAD padding).
pub fn panel_content_width_pub(outer_width: usize) -> usize {
    panel_content_width(outer_width)
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

/// Greedy word-wrap for a `RichLine`, preserving span emphasis across breaks.
///
/// Mirrors `wrap_text`'s greedy display-width algorithm but operates on
/// `Vec<RichSpan>` so that a span whose style is Bold/Italic/Code carries that
/// style on every wrapped fragment (BDR S9).  Empty input yields an empty Vec.
pub fn wrap_rich(line: &crate::richtext::RichLine, width: usize) -> Vec<crate::richtext::RichLine> {
    use crate::richtext::RichLine;
    if line.is_empty() || width == 0 {
        return vec![];
    }
    let plain: String = line.iter().map(|s| s.text.as_str()).collect();
    if plain.is_empty() || plain.chars().all(|c| c.is_ascii_whitespace()) {
        return vec![line.clone()];
    }

    let mut result: Vec<RichLine> = Vec::new();
    let mut current: RichLine = Vec::new();
    let mut current_dw = 0usize;

    for input_line in plain.split('\n') {
        if !current.is_empty() {
            result.push(std::mem::take(&mut current));
            current_dw = 0;
        }
        wrap_rich_single_line(
            input_line,
            line,
            width,
            &mut result,
            &mut current,
            &mut current_dw,
        );
    }

    if !current.is_empty() {
        result.push(current);
    } else if plain.chars().next().is_none() {
        result.push(vec![]);
    }

    result
}

/// Wrap a single plain-text input line, preserving span styles from the original `RichLine`.
///
/// Because the plain text was derived from the rich spans in order, we re-derive
/// the style for each word by scanning the span sequence proportionally to byte position.
fn wrap_rich_single_line(
    plain_line: &str,
    rich_source: &crate::richtext::RichLine,
    width: usize,
    result: &mut Vec<crate::richtext::RichLine>,
    current: &mut crate::richtext::RichLine,
    current_dw: &mut usize,
) {
    use crate::richtext::RichStyle;

    let style_for_word =
        |word: &str| -> RichStyle { style_of_word_in_rich_line(word, rich_source) };

    for word in plain_line.split_whitespace() {
        let word_dw = display_width(word);
        let word_style = style_for_word(word);

        if *current_dw == 0 {
            append_rich_word(
                word, word_dw, word_style, width, current, current_dw, result,
            );
            continue;
        }

        if *current_dw + 1 + word_dw <= width {
            push_rich_span(current, " ", RichStyle::Plain);
            push_rich_span(current, word, word_style);
            *current_dw += 1 + word_dw;
            continue;
        }

        result.push(std::mem::take(current));
        *current_dw = 0;
        append_rich_word(
            word, word_dw, word_style, width, current, current_dw, result,
        );
    }
}

/// Find the dominant style for `word` by scanning `rich_source` for an exact text match.
///
/// Falls back to Plain when the word is not found (e.g. after normalization).
fn style_of_word_in_rich_line(
    word: &str,
    rich_source: &crate::richtext::RichLine,
) -> crate::richtext::RichStyle {
    use crate::richtext::RichStyle;
    for span in rich_source {
        if span.text.contains(word) {
            return span.style;
        }
    }
    RichStyle::Plain
}

/// Append a word (or hard-split it) to the current rich line being built.
fn append_rich_word(
    word: &str,
    word_dw: usize,
    style: crate::richtext::RichStyle,
    width: usize,
    current: &mut crate::richtext::RichLine,
    current_dw: &mut usize,
    result: &mut Vec<crate::richtext::RichLine>,
) {
    if word_dw <= width {
        push_rich_span(current, word, style);
        *current_dw = word_dw;
    } else {
        hard_split_rich_word(word, style, width, current, current_dw, result);
    }
}

/// Hard-split a word wider than `width` columns, flushing full chunks as separate lines.
fn hard_split_rich_word(
    word: &str,
    style: crate::richtext::RichStyle,
    width: usize,
    current: &mut crate::richtext::RichLine,
    current_dw: &mut usize,
    result: &mut Vec<crate::richtext::RichLine>,
) {
    let mut acc = 0usize;
    let mut chunk = String::new();
    for ch in word.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if acc + cw > width {
            push_rich_span(current, &chunk, style);
            result.push(std::mem::take(current));
            chunk.clear();
            acc = 0;
        }
        chunk.push(ch);
        acc += cw;
    }
    push_rich_span(current, &chunk, style);
    *current_dw = acc;
}

/// Push a text fragment onto a `RichLine`, merging adjacent same-style spans.
fn push_rich_span(
    line: &mut crate::richtext::RichLine,
    text: &str,
    style: crate::richtext::RichStyle,
) {
    use crate::richtext::RichSpan;
    if text.is_empty() {
        return;
    }
    if let Some(last) = line.last_mut() {
        if last.style == style {
            last.text.push_str(text);
            return;
        }
    }
    line.push(RichSpan {
        text: text.to_string(),
        style,
    });
}

/// Compute the `StyleRun`s for a content row given its rich spans and the left chrome offset.
///
/// Each span's text is measured in display columns. Runs that carry non-Plain styles
/// are emitted with `start` already offset by `chrome_cols` so callers see final
/// display-column positions. Plain spans produce no run.
fn rich_line_to_style_runs(spans: &crate::richtext::RichLine, chrome_cols: usize) -> Vec<StyleRun> {
    use crate::richtext::RichStyle;
    let mut runs = Vec::new();
    let mut col = chrome_cols;
    for span in spans {
        let w = display_width(&span.text);
        if span.style != RichStyle::Plain {
            runs.push(StyleRun {
                start: col,
                len: w,
                style: span.style,
            });
        }
        col += w;
    }
    runs
}

/// Rounded-box identical to `panel_box` but returns `(plain_string, Vec<StyleRun>)` per row.
///
/// Border rows, blank pad rows, and the label row emit empty run vectors.
/// Content rows emit runs offset by the left chrome (1 border + PANEL_HPAD cols).
/// Returns empty when `width < 4` (same guard as `panel_box`).
pub fn panel_box_rich(
    label: &str,
    inner: &[crate::richtext::RichLine],
    width: usize,
) -> Vec<(String, Vec<StyleRun>)> {
    if width < 4 {
        return vec![];
    }
    let plain_lines: Vec<String> = inner
        .iter()
        .map(|rl| rl.iter().map(|s| s.text.as_str()).collect())
        .collect();
    let boxed = panel_box(label, &plain_lines, width);
    let chrome_cols = 1 + PANEL_HPAD;
    let content_rows_start = 1 + PANEL_VPAD;
    let content_rows_end = boxed.len().saturating_sub(1 + PANEL_VPAD);

    boxed
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let runs = if i >= content_rows_start && i < content_rows_end {
                let rich_idx = i - content_rows_start;
                inner
                    .get(rich_idx)
                    .map(|rl| rich_line_to_style_runs(rl, chrome_cols))
                    .unwrap_or_default()
            } else {
                vec![]
            };
            (line, runs)
        })
        .collect()
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
#[allow(dead_code)]
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

fn build_body_lines_with_collector(
    task: &Value,
    inner_width: usize,
    collector: &mut LinkCollector,
) -> (Vec<String>, Vec<Vec<StyleRun>>) {
    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let rich_lines = crate::richtext::structured_rich_with_links(body_html, collector);
    let content_width = panel_content_width(inner_width);
    let body_rich = if rich_lines.is_empty() {
        let fallback_wrapped = wrap_text(&t("(no description)"), content_width);
        plain_lines_to_rich(if fallback_wrapped.is_empty() {
            vec![t("(no description)")]
        } else {
            fallback_wrapped
        })
    } else {
        build_rich_body_rows(rich_lines, content_width)
    };
    let boxed = panel_box_rich(&t("Description"), &body_rich, inner_width);
    unzip_boxed(boxed)
}

/// Wrap a set of rich lines to `content_width` and return the wrapped rich lines.
fn build_rich_body_rows(
    rich_lines: Vec<crate::richtext::RichLine>,
    content_width: usize,
) -> Vec<crate::richtext::RichLine> {
    use crate::richtext::RichLine;
    let mut wrapped: Vec<RichLine> = Vec::new();
    for line in rich_lines {
        let fragments = wrap_rich(&line, content_width);
        if fragments.is_empty() {
            wrapped.push(vec![]);
        } else {
            wrapped.extend(fragments);
        }
    }
    if wrapped.is_empty() {
        plain_lines_to_rich(vec![t("(no description)")])
    } else {
        wrapped
    }
}

/// Convert a `Vec<String>` to a `Vec<RichLine>` where every span is Plain.
fn plain_lines_to_rich(lines: Vec<String>) -> Vec<crate::richtext::RichLine> {
    use crate::richtext::{RichSpan, RichStyle};
    lines
        .into_iter()
        .map(|s| {
            vec![RichSpan {
                text: s,
                style: RichStyle::Plain,
            }]
        })
        .collect()
}

/// Unzip `Vec<(String, Vec<StyleRun>)>` into parallel vecs.
fn unzip_boxed(boxed: Vec<(String, Vec<StyleRun>)>) -> (Vec<String>, Vec<Vec<StyleRun>>) {
    boxed.into_iter().unzip()
}

fn build_comment_lines_with_collector(
    comments: &[Value],
    inner_width: usize,
    collector: &mut LinkCollector,
) -> (Vec<String>, Vec<Vec<StyleRun>>) {
    if comments.is_empty() {
        return (vec![], vec![]);
    }
    let outer_content = panel_content_width(inner_width);
    let card_width = outer_content.saturating_sub(1);
    let mut nested_lines: Vec<String> = Vec::new();
    let mut nested_styles: Vec<Vec<StyleRun>> = Vec::new();
    let mut first = true;
    for comment in comments {
        if !first {
            nested_lines.push(String::new());
            nested_styles.push(vec![]);
        }
        first = false;

        let author = extract_comment_author(comment);
        let when = fmt_ts(comment.get("created_on").unwrap_or(&Value::Null));
        let (card_lines, card_styles) =
            build_comment_card(comment, &author, &when, card_width, collector);
        for (line, runs) in card_lines.into_iter().zip(card_styles) {
            nested_lines.push(format!(" {}", line));
            let indented_runs = indent_style_runs(runs, 1);
            nested_styles.push(indented_runs);
        }
    }

    let label = format!("{} ({})", t("Comments"), comments.len());
    let outer_rich = plain_lines_to_rich(nested_lines.clone());
    let boxed = panel_box_rich(&label, &outer_rich, inner_width);
    let (outer_lines, outer_box_styles) = unzip_boxed(boxed);

    let chrome = 1 + PANEL_HPAD;
    let content_start = 1 + PANEL_VPAD;
    let content_end = outer_lines.len().saturating_sub(1 + PANEL_VPAD);

    let merged_styles: Vec<Vec<StyleRun>> = outer_lines
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if i >= content_start && i < content_end {
                let nested_idx = i - content_start;
                nested_styles
                    .get(nested_idx)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| StyleRun {
                        start: r.start + chrome,
                        len: r.len,
                        style: r.style,
                    })
                    .collect()
            } else {
                outer_box_styles.get(i).cloned().unwrap_or_default()
            }
        })
        .collect();

    (outer_lines, merged_styles)
}

/// Build a single comment card as a plain `comment_box`, returning (lines, style_runs).
///
/// The body is parsed through the rich path so inline emphasis carries through
/// into the card's content rows.
fn build_comment_card(
    comment: &Value,
    author: &str,
    when: &str,
    card_width: usize,
    collector: &mut LinkCollector,
) -> (Vec<String>, Vec<Vec<StyleRun>>) {
    let rich_body = extract_comment_body_rich(comment, collector);
    let content_width = panel_content_width(card_width);
    let body_rich = if rich_body.is_empty() {
        plain_lines_to_rich(vec![String::new()])
    } else {
        build_rich_body_rows(rich_body, content_width)
    };
    let label = format!("{} {} {}", author, MIDDOT, when);
    panel_box_rich(&label, &body_rich, card_width)
        .into_iter()
        .unzip()
}

/// Shift every `StyleRun`'s start column right by `cols`.
fn indent_style_runs(runs: Vec<StyleRun>, cols: usize) -> Vec<StyleRun> {
    runs.into_iter()
        .map(|r| StyleRun {
            start: r.start + cols,
            len: r.len,
            style: r.style,
        })
        .collect()
}

/// Extract comment body as rich lines for the TUI path.
fn extract_comment_body_rich(
    comment: &Value,
    collector: &mut LinkCollector,
) -> Vec<crate::richtext::RichLine> {
    use crate::richtext::{RichSpan, RichStyle};
    let plain = comment
        .get("body_plain_text")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    match plain {
        Some(s) => vec![vec![RichSpan {
            text: s.to_string(),
            style: RichStyle::Plain,
        }]],
        None => {
            let html = comment.get("body").and_then(|v| v.as_str()).unwrap_or("");
            crate::richtext::structured_rich_with_links(html, collector)
        }
    }
}

/// Build the full detail content for the TUI detail view.
///
/// URLs in the description and comment bodies are rendered inline as `text [url]`
/// (no separate URL list). `line_styles` is a parallel channel index-aligned with
/// `lines`; both vecs are always the same length.
/// Pure: no I/O, no async, no time access.
pub fn build_detail_content(
    task: &Value,
    comments: &[Value],
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> DetailContent {
    let mut collector = LinkCollector::new();
    let mut lines: Vec<String> = vec![];
    let mut line_styles: Vec<Vec<StyleRun>> = vec![];

    let header_lines = build_header_lines(task, user_map, inner_width);
    let header_count = header_lines.len();
    lines.extend(header_lines);
    line_styles.extend(std::iter::repeat_n(vec![], header_count));

    lines.push(String::new());
    line_styles.push(vec![]);

    let (body_lines, body_styles) =
        build_body_lines_with_collector(task, inner_width, &mut collector);
    lines.extend(body_lines);
    line_styles.extend(body_styles);

    if !comments.is_empty() {
        lines.push(String::new());
        line_styles.push(vec![]);

        let (comment_lines, comment_styles) =
            build_comment_lines_with_collector(comments, inner_width, &mut collector);
        lines.extend(comment_lines);
        line_styles.extend(comment_styles);
    }

    debug_assert_eq!(
        lines.len(),
        line_styles.len(),
        "lines and line_styles must remain index-aligned"
    );

    DetailContent { lines, line_styles }
}

/// Builds the 2-column aligned meta table rows for the Details panel.
///
/// Delegates field extraction to `meta_field_pairs`, computes the shared label
/// column width, then formats each pair via `format_meta_row`.
fn build_meta_table_rows(
    task: &Value,
    user_map: &HashMap<i64, String>,
    inner_width: usize,
) -> Vec<String> {
    let pairs = meta_field_pairs(task, user_map);
    let label_col = meta_label_col_width(&pairs);
    let content_width = panel_content_width(inner_width);
    pairs
        .into_iter()
        .flat_map(|(lbl, val)| format_meta_row(&lbl, &val, label_col, content_width))
        .collect()
}

/// Returns the ordered (translated_label, value) pairs for the Details meta table.
///
/// Always includes Task, Title, Project, Status, Assignee, Estimate, Logged.
/// Start and Due are included only when non-empty (i.e. the task has those dates set).
fn meta_field_pairs(task: &Value, user_map: &HashMap<i64, String>) -> Vec<(String, String)> {
    let project_id = task.get("project_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let task_id = task.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let title = task
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let project_name = task
        .get("project_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| t("(unknown)"));
    let status = meta_status_value(task);
    let assignee = meta_assignee_value(task, user_map);
    let start = fmt_date(task.get("start_on").unwrap_or(&Value::Null));
    let due = fmt_date(task.get("due_on").unwrap_or(&Value::Null));
    let estimate = format!(
        "{}h",
        fmt_hours(task.get("estimate").unwrap_or(&Value::Null))
    );
    let logged = format!(
        "{}h",
        fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null))
    );

    let mut pairs = vec![
        (t("Task"), format!("{}-{}", project_id, task_id)),
        (t("Title"), title),
        (t("Project"), project_name),
        (t("Status"), status),
        (t("Assignee"), assignee),
    ];
    if !start.is_empty() {
        pairs.push((t("Start"), start));
    }
    if !due.is_empty() {
        pairs.push((t("Due"), due));
    }
    pairs.push((t("Estimate"), estimate));
    pairs.push((t("Logged"), logged));
    pairs
}

/// Returns the translated completion status string for `task`.
fn meta_status_value(task: &Value) -> String {
    if task
        .get("is_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        t("Completed")
    } else {
        t("Open")
    }
}

/// Returns the resolved assignee display string for `task`.
///
/// Falls back to `(unassigned)` when no assignee_id is present, and to `(id)`
/// when the id is not found in `user_map`.
fn meta_assignee_value(task: &Value, user_map: &HashMap<i64, String>) -> String {
    match task.get("assignee_id").and_then(|v| v.as_i64()) {
        None => t("(unassigned)"),
        Some(id) => match user_map.get(&id) {
            Some(name) => format!("{name} ({id})"),
            None => format!("({id})"),
        },
    }
}

/// Returns the maximum char count across all label strings in `pairs`.
///
/// This is the shared left-column width used to align the value column.
fn meta_label_col_width(pairs: &[(String, String)]) -> usize {
    pairs
        .iter()
        .map(|(lbl, _)| lbl.chars().count())
        .max()
        .unwrap_or(0)
}

/// Formats a single (label, value) pair as one or more wrapped meta-row lines.
///
/// The first line is `"{label:<label_col$}  {value_fragment}"`. When the value is
/// wider than the remaining columns, it wraps to continuation lines indented by
/// `label_col + 2` spaces so the value text aligns under itself. No ellipsis is
/// ever inserted.
fn format_meta_row(
    label: &str,
    value: &str,
    label_col: usize,
    content_width: usize,
) -> Vec<String> {
    let prefix = format!("{:<width$}  ", label, width = label_col);
    let prefix_len = prefix.chars().count();
    let value_width = content_width.saturating_sub(prefix_len);
    if value_width == 0 {
        return vec![prefix];
    }
    let value_lines = wrap_text(value, value_width);
    let indent = " ".repeat(prefix_len);
    match value_lines.as_slice() {
        [] => vec![prefix],
        [first] => vec![format!("{prefix}{first}")],
        [first, rest @ ..] => {
            let mut rows = vec![format!("{prefix}{first}")];
            for cont in rest {
                rows.push(format!("{indent}{cont}"));
            }
            rows
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

/// Returns `s` truncated to exactly `max_width` chars.
///
/// When `s` fits within `max_width`, it is returned unchanged. When it is longer,
/// the result is the first `max_width - 1` chars followed by "\u{2026}" so the
/// caller's column stays at exactly `max_width` chars. Edge cases: `max_width == 0`
/// returns an empty string; `max_width == 1` returns "\u{2026}".
#[allow(dead_code)]
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
