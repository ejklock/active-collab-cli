use crate::i18n::t;
use crate::store::secs_to_utc_parts;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;

mod cli_render;
mod detail_render;
mod text_measure;
mod wrap;

pub use text_measure::{box_inner_content, slice_by_display_cols, BODY_LEFT_CHROME_COLS};
pub(crate) use text_measure::{
    display_width, truncate_to_display_width, BOX_BL, BOX_BR, BOX_H, BOX_TL, BOX_TR, BOX_V,
    PANEL_HPAD,
};
use text_measure::{fit_to_display_width, panel_content_width};
pub use wrap::{wrap_rich, wrap_text};

pub use cli_render::{html_to_text, render_task_to_str};
// Only exercised by tests/unit/render.rs through this module's `use super::*` — no
// non-test caller names these paths, so keep the re-export test-only to satisfy
// `-D warnings` on the plain bin target.
#[cfg(test)]
use cli_render::{render_comments_to_str, render_meta_to_str};

pub use detail_render::build_detail_content;
// build_header_lines/comment_box/panel_box/DetailContent have no non-test callers today
// but are referenced by absolute path (`crate::render::…`) from tests/unit/tui_render.rs,
// a sibling test file outside this module's descendant tree — pub(crate) so it resolves
// there too, not just through this module's own `use super::*` test include.
use detail_render::format_asset_row;
pub(crate) use detail_render::PANEL_VPAD;
#[cfg(test)]
use detail_render::{build_body_lines, build_comment_lines};
#[cfg(test)]
pub(crate) use detail_render::{build_header_lines, comment_box, panel_box, DetailContent};

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

/// What kind of action a `LocalAffordance` triggers on Ctrl/Cmd+click.
///
/// `Edit(comment_id)` and `Delete(comment_id)` carry the id of the targeted comment.
/// `Edit` opens the compose area pre-filled with the comment body; `Delete` requests
/// a confirmation via the modal overlay (ADR 0039). The modal's [confirmar]/[cancelar]
/// buttons are a separate registry (`Model.modal_button_targets`), registered by
/// `view::register_confirm_button_targets` from the `Rect` that `render_modal` returns,
/// and resolved by `model::dispatch_confirm_modal_click` on a plain click — single-sourced
/// geometry that does not go through this scroll-aware affordance list.
///
/// `OpenAsset(url)` carries the asset's url; emitted once per asset content row (including
/// every wrapped continuation line) by `build_detail_content` at layout time (ADR 0043 §1).
/// Resolved by `asset_panel_cmd_at` on Ctrl/Cmd+click.
///
/// `OpenUrl(url)` carries an inline body-link url; populated by slice 0046. Defined here
/// to keep the enum stable across the two slices.
#[derive(Debug, Clone, PartialEq)]
pub enum AffordanceKind {
    Edit(i64),
    Delete(i64),
    OpenAsset(String),
    /// Populated at layout time by `build_body_lines` (ADR 0043 §2): one span per
    /// wrapped fragment of an openable inline URL/email token.
    /// Resolved by `body_link_cmd_at` on Ctrl/Cmd+click.
    OpenUrl(String),
}

impl AffordanceKind {
    /// Returns true when the whole row is the click target, not just the link span.
    ///
    /// `OpenAsset` spans cover only the link text column range, but ADR 0029 specifies
    /// that any Ctrl/Cmd+click anywhere on the asset row should open the asset.
    /// All other kinds require the click to fall within `[col_start, col_end)`.
    pub fn is_row_target(&self) -> bool {
        matches!(self, AffordanceKind::OpenAsset(_))
    }
}

/// A clickable affordance span registered during `build_detail_content`.
///
/// Carries the line index (within `DetailContent.lines`), the half-open
/// display-column range `[col_start, col_end)`, and the `AffordanceKind` that
/// describes what action the hit should trigger. Used by the hit-test in
/// `handle_click_detail`.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalAffordance {
    pub line_idx: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub kind: AffordanceKind,
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
///
/// Used by unit tests in `tests/unit/render.rs` to validate inline URL detection.
#[allow(dead_code)]
pub fn url_at(line: &str, target_col: usize) -> Option<String> {
    if let Some(url) = bracketed_url_at(line, target_col) {
        return Some(url);
    }
    raw_url_at(line, target_col)
}

/// Scan `[INNER]` tokens in `line`; return INNER when `target_col` is within the
/// inner span and INNER is a URL or bare e-mail.
#[allow(dead_code)]
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
#[allow(dead_code)]
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

/// Build the visible lines and style runs for the compose overlay area.
///
/// Build the modal body lines for the compose overlay.
///
/// Returns `(lines, line_styles)` — the buffer split on `\n`, with a parallel
/// empty-style-run vec. No label, no status text: the modal box renders the
/// title separately and the caller supplies the in-box hint/status line.
/// Called by `view()` to populate the `ModalContent` body.
pub fn compose_block_lines(cp: &crate::tui::model::Compose) -> (Vec<String>, Vec<Vec<StyleRun>>) {
    let mut lines: Vec<String> = Vec::new();
    let mut styles: Vec<Vec<StyleRun>> = Vec::new();

    for body_line in cp.buffer.split('\n') {
        lines.push(body_line.to_string());
        styles.push(vec![]);
    }

    (lines, styles)
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
#[path = "../../tests/unit/render.rs"]
mod tests;
