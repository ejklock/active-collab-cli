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

/// Produced by `build_detail_content`: the rendered lines and style runs.
///
/// `lines` holds the full detail layout with real URLs inline as `text [url]`.
/// `line_styles` is a parallel channel: `line_styles[i]` holds the `StyleRun`s for
/// `lines[i]`. Both vecs are always the same length.
/// `affordances` records all clickable spans (edit, delete, confirm, cancel) for
/// hit-testing; each entry carries its `AffordanceKind` so the caller dispatches
/// without maintaining parallel collections.
/// `comment_spans` maps comment index → `(start_line, line_count)` in global `lines`.
/// Empty when there are no comments.
pub struct DetailContent {
    pub lines: Vec<String>,
    pub line_styles: Vec<Vec<StyleRun>>,
    pub affordances: Vec<LocalAffordance>,
    pub comment_spans: Vec<(usize, usize)>,
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

/// Prepend `mailto:` when `token` is a bare email (contains `@`, no scheme).
///
/// Single home for link normalization at the body-link emit site; replaces the
/// version that was in `model.rs`.
fn normalize_link_url(token: &str) -> String {
    if token.contains('@') && !token.contains("://") && !token.starts_with("mailto:") {
        format!("mailto:{token}")
    } else {
        token.to_string()
    }
}

/// Flatten a `RichLine` to its plain text.
fn rich_line_plain(line: &crate::richtext::RichLine) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}

/// Return the net bracket depth of `s`: `+1` per `[`, `-1` per `]`.
///
/// A positive result means `s` contains an unmatched `[` (hard-split token start).
fn bracket_depth(s: &str) -> i32 {
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// Return `(body_rich_line_idx, panel_col_start, panel_col_end, normalized_url)` tuples
/// for every openable URL/email token in the wrapped body-rich lines.
///
/// Bracketed `[url]` tokens are handled by bracket-counting: lines with an unmatched
/// `[` are joined with their continuations until the group closes. Raw `https?://…`
/// tokens that reach the right edge of a full-width line are joined with the next line
/// so their complete URL is recovered. Both strategies fix the OBS-35 over-join bug by
/// stopping at a structural boundary (closing `]` or URL terminator) instead of at
/// display-width.
///
/// `panel_col_start`/`panel_col_end` are in the same coordinate space as
/// `LocalAffordance.col_start`/`col_end` (where `│` = col 0; content starts at
/// `BODY_LEFT_CHROME_COLS` = 2). Only tokens that pass `is_openable_url` or are
/// `mailto:` emails are emitted; non-openable `[note]` tokens produce no entry.
fn collect_body_url_affordances(
    body_rich: &[crate::richtext::RichLine],
    content_width: usize,
) -> Vec<(usize, usize, usize, String)> {
    let mut out: Vec<(usize, usize, usize, String)> = Vec::new();
    let mut i = 0;

    while i < body_rich.len() {
        let plain = rich_line_plain(&body_rich[i]);
        let plain_dw = display_width(&plain);
        let is_full_width = plain_dw == content_width && content_width > 0;

        let (joined, frag_widths, consumed) = if bracket_depth(&plain) > 0 && is_full_width {
            join_bracketed_group(plain, plain_dw, i, body_rich, content_width)
        } else if is_full_width && raw_url_reaches_edge(&plain, plain_dw) {
            join_raw_url_group(plain, plain_dw, i, body_rich, content_width)
        } else {
            (plain, vec![plain_dw], 0)
        };

        emit_group_url_affordances(&joined, &frag_widths, i, &mut out);
        i += 1 + consumed;
    }

    out
}

/// Join lines starting at `start_i` into one string while any `[` remains unmatched.
///
/// Returns `(joined_text, frag_widths, extra_lines_consumed)`. The caller advances
/// the outer index by `1 + extra_lines_consumed`.
fn join_bracketed_group(
    first_plain: String,
    first_dw: usize,
    start_i: usize,
    body_rich: &[crate::richtext::RichLine],
    content_width: usize,
) -> (String, Vec<usize>, usize) {
    let mut joined = first_plain;
    let mut frag_widths: Vec<usize> = vec![first_dw];
    let mut consumed = 0usize;

    while bracket_depth(&joined) > 0 && start_i + 1 + consumed < body_rich.len() && consumed < 20 {
        consumed += 1;
        let np = rich_line_plain(&body_rich[start_i + consumed]);
        let ndw = display_width(&np);
        let is_last_frag = ndw < content_width;
        joined.push_str(&np);
        frag_widths.push(ndw);
        if is_last_frag {
            break;
        }
    }

    (joined, frag_widths, consumed)
}

/// Join lines starting at `start_i` while a raw URL ends exactly at the right edge.
///
/// Returns `(joined_text, frag_widths, extra_lines_consumed)`. The caller advances
/// the outer index by `1 + extra_lines_consumed`.
fn join_raw_url_group(
    first_plain: String,
    first_dw: usize,
    start_i: usize,
    body_rich: &[crate::richtext::RichLine],
    content_width: usize,
) -> (String, Vec<usize>, usize) {
    let mut joined = first_plain;
    let mut frag_widths: Vec<usize> = vec![first_dw];
    let mut consumed = 0usize;

    while raw_url_reaches_edge_of(&joined, total_frag_width(&frag_widths))
        && start_i + 1 + consumed < body_rich.len()
        && consumed < 20
    {
        consumed += 1;
        let np = rich_line_plain(&body_rich[start_i + consumed]);
        let ndw = display_width(&np);
        joined.push_str(&np);
        frag_widths.push(ndw);
        if ndw < content_width {
            break;
        }
    }

    (joined, frag_widths, consumed)
}

/// Sum fragment widths to get the total joined-string display width so far.
fn total_frag_width(frag_widths: &[usize]) -> usize {
    frag_widths.iter().sum()
}

/// Return true when a raw URL in `plain` ends exactly at `edge_col` display columns
/// (the right edge of the fragment), suggesting the URL may continue on the next line.
fn raw_url_reaches_edge(plain: &str, edge_col: usize) -> bool {
    raw_url_reaches_edge_of(plain, edge_col)
}

/// Return true when any raw URL match in `joined` ends exactly at `edge_col`.
fn raw_url_reaches_edge_of(joined: &str, edge_col: usize) -> bool {
    for m in body_url_re().find_iter(joined) {
        let col_end = display_col_of_byte(joined, m.end());
        if col_end == edge_col {
            return true;
        }
    }
    false
}

/// For each URL/email span found in `joined_text`, emit one affordance per fragment
/// line into `out`.
///
/// `frag_widths[k]` is the display width of body_rich line `start_idx + k`.
/// `col_start`/`col_end` are emitted in panel display-column space (content at col
/// `BODY_LEFT_CHROME_COLS` = 2).
fn emit_group_url_affordances(
    joined_text: &str,
    frag_widths: &[usize],
    start_idx: usize,
    out: &mut Vec<(usize, usize, usize, String)>,
) {
    let spans = collect_link_spans(joined_text);
    for span in &spans {
        let url_inner = &joined_text[span.start..span.end];
        let normalized = normalize_link_url(url_inner);
        if !is_openable_url(&normalized) && !normalized.starts_with("mailto:") {
            continue;
        }
        let url_col_start = display_col_of_byte(joined_text, span.start);
        let url_col_end = url_col_start + display_width(url_inner);
        emit_url_per_fragment(
            url_col_start,
            url_col_end,
            &normalized,
            frag_widths,
            start_idx,
            out,
        );
    }
}

/// Compute the per-fragment affordance spans for a URL token that spans `[url_col_start,
/// url_col_end)` in the joined-group display columns.
///
/// For each fragment k (at body_rich line `start_idx + k`), the fragment covers joined
/// display columns `[frag_offset, frag_offset + frag_widths[k])`. The intersection of
/// `[url_col_start, url_col_end)` with that range gives the URL sub-span on that
/// fragment, which is then offset by `BODY_LEFT_CHROME_COLS` (= 2) to produce panel
/// column coordinates.
fn emit_url_per_fragment(
    url_col_start: usize,
    url_col_end: usize,
    url: &str,
    frag_widths: &[usize],
    start_idx: usize,
    out: &mut Vec<(usize, usize, usize, String)>,
) {
    let mut frag_offset = 0usize;
    for (k, &fw) in frag_widths.iter().enumerate() {
        let frag_end = frag_offset + fw;
        let overlap_start = url_col_start.max(frag_offset);
        let overlap_end = url_col_end.min(frag_end);
        if overlap_start < overlap_end {
            let rel_start = overlap_start - frag_offset;
            let rel_end = overlap_end - frag_offset;
            out.push((
                start_idx + k,
                BODY_LEFT_CHROME_COLS + rel_start,
                BODY_LEFT_CHROME_COLS + rel_end,
                url.to_string(),
            ));
        }
        frag_offset = frag_end;
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
/// style on every wrapped fragment.  Style is threaded per-character so repeated
/// words with different emphasis and substring words each keep their own style
/// (ADR 0030, BDR 0023).  Empty input yields an empty Vec.
pub fn wrap_rich(line: &crate::richtext::RichLine, width: usize) -> Vec<crate::richtext::RichLine> {
    use crate::richtext::RichLine;
    if line.is_empty() || width == 0 {
        return vec![];
    }
    let plain: String = line.iter().map(|s| s.text.as_str()).collect();
    if plain.is_empty() || plain.chars().all(|c| c.is_ascii_whitespace()) {
        return vec![line.clone()];
    }

    let styled_chars = expand_to_styled_chars(line);
    let mut result: Vec<RichLine> = Vec::new();
    let mut current: RichLine = Vec::new();
    let mut current_dw = 0usize;
    let mut first_segment = true;

    for segment in styled_chars.split(|(ch, _)| *ch == '\n') {
        if !first_segment && !current.is_empty() {
            result.push(std::mem::take(&mut current));
            current_dw = 0;
        }
        first_segment = false;
        wrap_rich_single_line(segment, width, &mut result, &mut current, &mut current_dw);
    }

    if !current.is_empty() {
        result.push(current);
    } else if plain.chars().next().is_none() {
        result.push(vec![]);
    }

    result
}

/// Expand a `RichLine` to an ordered sequence of `(char, RichStyle)` pairs.
///
/// This is the single place where per-character origin is preserved: each span
/// contributes its characters tagged with that span's style.  The wrap operates
/// on this stream instead of re-deriving style by substring matching.
fn expand_to_styled_chars(
    line: &crate::richtext::RichLine,
) -> Vec<(char, crate::richtext::RichStyle)> {
    line.iter()
        .flat_map(|span| span.text.chars().map(move |ch| (ch, span.style)))
        .collect()
}

/// Wrap a single segment of styled characters (between hard newlines) using greedy width.
///
/// Words are maximal non-whitespace runs in the styled-char stream.  Each word's
/// characters carry their per-character style through to the output spans, so a word
/// that straddles an emphasis boundary keeps both styles as adjacent spans.
fn wrap_rich_single_line(
    styled_chars: &[(char, crate::richtext::RichStyle)],
    width: usize,
    result: &mut Vec<crate::richtext::RichLine>,
    current: &mut crate::richtext::RichLine,
    current_dw: &mut usize,
) {
    use crate::richtext::RichStyle;

    let mut i = 0;
    while i < styled_chars.len() {
        // Skip whitespace (but not newlines — those are already split out).
        if styled_chars[i].0.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Collect one word: a maximal run of non-whitespace styled chars.
        let word_start = i;
        while i < styled_chars.len() && !styled_chars[i].0.is_ascii_whitespace() {
            i += 1;
        }
        let word_chars = &styled_chars[word_start..i];
        let word_dw = word_display_width(word_chars);

        if *current_dw == 0 {
            append_rich_word(word_chars, word_dw, width, current, current_dw, result);
            continue;
        }

        if *current_dw + 1 + word_dw <= width {
            push_rich_span(current, " ", RichStyle::Plain);
            emit_styled_chars(current, word_chars);
            *current_dw += 1 + word_dw;
            continue;
        }

        result.push(std::mem::take(current));
        *current_dw = 0;
        append_rich_word(word_chars, word_dw, width, current, current_dw, result);
    }
}

/// Compute the display width of a slice of styled characters.
fn word_display_width(chars: &[(char, crate::richtext::RichStyle)]) -> usize {
    chars
        .iter()
        .map(|(ch, _)| unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0))
        .sum()
}

/// Push each styled character onto the current line via `push_rich_span`, coalescing runs.
fn emit_styled_chars(
    current: &mut crate::richtext::RichLine,
    chars: &[(char, crate::richtext::RichStyle)],
) {
    for (ch, style) in chars {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        push_rich_span(current, s, *style);
    }
}

/// Append a word (or hard-split it) to the current rich line being built.
fn append_rich_word(
    word_chars: &[(char, crate::richtext::RichStyle)],
    word_dw: usize,
    width: usize,
    current: &mut crate::richtext::RichLine,
    current_dw: &mut usize,
    result: &mut Vec<crate::richtext::RichLine>,
) {
    if word_dw <= width {
        emit_styled_chars(current, word_chars);
        *current_dw = word_dw;
    } else {
        hard_split_rich_word(word_chars, width, current, current_dw, result);
    }
}

/// Hard-split a word wider than `width` columns, flushing full chunks as separate lines.
///
/// Each character retains its per-character style when placed into a chunk.
fn hard_split_rich_word(
    word_chars: &[(char, crate::richtext::RichStyle)],
    width: usize,
    current: &mut crate::richtext::RichLine,
    current_dw: &mut usize,
    result: &mut Vec<crate::richtext::RichLine>,
) {
    let mut acc = 0usize;
    for (ch, style) in word_chars {
        let cw = unicode_width::UnicodeWidthChar::width(*ch).unwrap_or(0);
        if acc + cw > width {
            result.push(std::mem::take(current));
            acc = 0;
        }
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        push_rich_span(current, s, *style);
        acc += cw;
    }
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

fn build_body_lines(
    task: &Value,
    inner_width: usize,
) -> (Vec<String>, Vec<Vec<StyleRun>>, Vec<LocalAffordance>) {
    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let rich_lines = crate::richtext::structured_rich_with_links(body_html);
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

    let raw_affs = collect_body_url_affordances(&body_rich, content_width);
    let boxed = panel_box_rich(&t("Description"), &body_rich, inner_width);
    let (lines, styles) = unzip_boxed(boxed);

    // Content rows start after the top border + PANEL_VPAD blank rows.
    let content_row_offset = 1 + PANEL_VPAD;
    let affordances = raw_affs
        .into_iter()
        .map(|(k, col_start, col_end, url)| LocalAffordance {
            line_idx: content_row_offset + k,
            col_start,
            col_end,
            kind: AffordanceKind::OpenUrl(url),
        })
        .collect();

    (lines, styles, affordances)
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

/// Output of `build_comment_card`: the rendered text lines, their style runs, and
/// optional affordance spans within the top-border line.
///
/// `edit_span` is the half-open display-column span of the `[editar]` token.
/// `delete_span` is the half-open display-column span of the `[excluir]` token.
/// Both are `None` when `is_own` is false.
#[derive(Debug)]
struct CommentCard {
    lines: Vec<String>,
    line_styles: Vec<Vec<StyleRun>>,
    edit_span: Option<(usize, usize)>,
    delete_span: Option<(usize, usize)>,
}

/// Affordance span relative to the `nested_lines` vec produced during comment rendering.
///
/// These local offsets are translated to global `DetailContent.lines` indices by
/// `build_comment_lines`.
struct CardAffordance {
    line_idx: usize,
    col_start: usize,
    col_end: usize,
    kind: AffordanceKind,
}

type CommentLinesOutput = (
    Vec<String>,
    Vec<Vec<StyleRun>>,
    Vec<CardAffordance>,
    Vec<(usize, usize)>,
);

/// Append a built comment card's lines, styles, and affordances into the nested
/// accumulators. Returns `(card_start_idx, card_end_idx)` so the caller can
/// record the span for scroll-into-view.
fn append_card_to_nested(
    card: CommentCard,
    comment_id: i64,
    nested_lines: &mut Vec<String>,
    nested_styles: &mut Vec<Vec<StyleRun>>,
    card_affordances: &mut Vec<CardAffordance>,
) -> (usize, usize) {
    let card_start_idx = nested_lines.len();
    for (line, runs) in card.lines.into_iter().zip(card.line_styles) {
        nested_lines.push(format!(" {}", line));
        nested_styles.push(indent_style_runs(runs, 1));
    }
    if let Some((s, e)) = card.edit_span {
        card_affordances.push(CardAffordance {
            line_idx: card_start_idx,
            col_start: s + 1,
            col_end: e + 1,
            kind: AffordanceKind::Edit(comment_id),
        });
    }
    if let Some((s, e)) = card.delete_span {
        card_affordances.push(CardAffordance {
            line_idx: card_start_idx,
            col_start: s + 1,
            col_end: e + 1,
            kind: AffordanceKind::Delete(comment_id),
        });
    }
    let card_end_idx = nested_lines.len();
    (card_start_idx, card_end_idx)
}

/// Merge per-card nested styles with the outer box styles.
///
/// Lines in the content region use the nested style runs (offset by `chrome`);
/// border and padding lines use the outer box styles.
fn merge_nested_styles_into_outer(
    outer_lines: &[String],
    outer_box_styles: &[Vec<StyleRun>],
    nested_styles: &[Vec<StyleRun>],
    content_start: usize,
    content_end: usize,
    chrome: usize,
) -> Vec<Vec<StyleRun>> {
    outer_lines
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
        .collect()
}

fn build_comment_lines(
    comments: &[Value],
    inner_width: usize,
    current_user_id: Option<i64>,
) -> CommentLinesOutput {
    if comments.is_empty() {
        return (vec![], vec![], vec![], vec![]);
    }
    let outer_content = panel_content_width(inner_width);
    let card_width = outer_content.saturating_sub(1);
    let mut nested_lines: Vec<String> = Vec::new();
    let mut nested_styles: Vec<Vec<StyleRun>> = Vec::new();
    let mut card_affordances: Vec<CardAffordance> = Vec::new();
    let mut nested_card_ranges: Vec<(usize, usize)> = Vec::new();
    let mut first = true;

    for comment in comments {
        if !first {
            nested_lines.push(String::new());
            nested_styles.push(vec![]);
        }
        first = false;

        let comment_id = comment.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let created_by_id = comment.get("created_by_id").and_then(|v| v.as_i64());
        let is_own = current_user_id
            .zip(created_by_id)
            .map(|(uid, cid)| uid == cid)
            .unwrap_or(false);

        let author = extract_comment_author(comment);
        let when = fmt_ts(comment.get("created_on").unwrap_or(&Value::Null));
        let ctx = CommentCardCtx {
            comment,
            author: &author,
            when: &when,
            card_width,
            is_own,
        };
        let card = build_comment_card(ctx);

        let (card_start_idx, card_end_idx) = append_card_to_nested(
            card,
            comment_id,
            &mut nested_lines,
            &mut nested_styles,
            &mut card_affordances,
        );
        nested_card_ranges.push((card_start_idx, card_end_idx - card_start_idx));
    }

    let label = format!("{} ({})", t("Comments"), comments.len());
    let outer_rich = plain_lines_to_rich(nested_lines.clone());
    let boxed = panel_box_rich(&label, &outer_rich, inner_width);
    let (outer_lines, outer_box_styles) = unzip_boxed(boxed);

    let chrome = 1 + PANEL_HPAD;
    let content_start = 1 + PANEL_VPAD;
    let content_end = outer_lines.len().saturating_sub(1 + PANEL_VPAD);

    let merged_styles = merge_nested_styles_into_outer(
        &outer_lines,
        &outer_box_styles,
        &nested_styles,
        content_start,
        content_end,
        chrome,
    );

    let translated_affordances: Vec<CardAffordance> = card_affordances
        .into_iter()
        .map(|a| CardAffordance {
            line_idx: content_start + a.line_idx,
            col_start: a.col_start + chrome,
            col_end: a.col_end + chrome,
            kind: a.kind,
        })
        .collect();

    let outer_card_ranges: Vec<(usize, usize)> = nested_card_ranges
        .into_iter()
        .map(|(start, count)| (content_start + start, count))
        .collect();

    (
        outer_lines,
        merged_styles,
        translated_affordances,
        outer_card_ranges,
    )
}

/// Parameters for building a single comment card.
///
/// Groups the arguments previously passed positionally to `build_comment_card`
/// into a single named context struct so the call site is self-documenting and
/// clippy::too_many_arguments does not apply.
struct CommentCardCtx<'a> {
    comment: &'a Value,
    author: &'a str,
    when: &'a str,
    card_width: usize,
    is_own: bool,
}

/// Build a single comment card.
///
/// The body is parsed through the rich path so inline emphasis carries through
/// into the card's content rows. When `ctx.is_own` is true, `[editar]` and `[excluir]`
/// tokens are appended to the label and the returned spans hold their display-column
/// ranges within the top-border line. All spans are `None` when `ctx.is_own` is false.
fn build_comment_card(ctx: CommentCardCtx<'_>) -> CommentCard {
    let CommentCardCtx {
        comment,
        author,
        when,
        card_width,
        is_own,
    } = ctx;

    let rich_body = extract_comment_body_rich(comment);
    let content_width = panel_content_width(card_width);
    let body_rich = if rich_body.is_empty() {
        plain_lines_to_rich(vec![String::new()])
    } else {
        build_rich_body_rows(rich_body, content_width)
    };

    let edit_token = format!("[{}]", t("edit"));
    let delete_token = format!("[{}]", t("excluir"));
    let label = if is_own {
        format!(
            "{} {} {} {} {}",
            author, MIDDOT, when, edit_token, delete_token
        )
    } else {
        format!("{} {} {}", author, MIDDOT, when)
    };

    let (lines, line_styles): (Vec<_>, Vec<_>) = panel_box_rich(&label, &body_rich, card_width)
        .into_iter()
        .unzip();

    let (edit_span, delete_span) = if is_own && !lines.is_empty() {
        let es = find_token_span(&lines[0], &edit_token);
        let ds = find_token_span(&lines[0], &delete_token);
        (es, ds)
    } else {
        (None, None)
    };

    let mut line_styles = line_styles;
    push_affordance_style_runs(&mut line_styles, edit_span, delete_span);

    CommentCard {
        lines,
        line_styles,
        edit_span,
        delete_span,
    }
}

/// Push `StyleRun`s for the edit and delete affordance spans onto `line_styles[0]`.
///
/// Reuses the exact (start, len) coordinates already computed for the hit-test
/// (single source of truth, ADR 0032 / ADR 0041). No-op when a span is `None`.
fn push_affordance_style_runs(
    line_styles: &mut [Vec<StyleRun>],
    edit_span: Option<(usize, usize)>,
    delete_span: Option<(usize, usize)>,
) {
    use crate::richtext::RichStyle;
    if edit_span.is_none() && delete_span.is_none() {
        return;
    }
    let header_runs = match line_styles.first_mut() {
        Some(r) => r,
        None => return,
    };
    if let Some((start, end)) = edit_span {
        header_runs.push(StyleRun {
            start,
            len: end - start,
            style: RichStyle::EditAffordance,
        });
    }
    if let Some((start, end)) = delete_span {
        header_runs.push(StyleRun {
            start,
            len: end - start,
            style: RichStyle::DeleteAffordance,
        });
    }
}

/// Locate a token in a line and return its half-open display-column span.
///
/// Scans from the end of the line to prefer the rightmost occurrence (which is
/// correct for the affordance tokens appended after the label text).
/// Returns `None` when the token is absent or the line is shorter than the token.
fn find_token_span(line: &str, token: &str) -> Option<(usize, usize)> {
    let token_bytes = token.as_bytes();
    let line_bytes = line.as_bytes();
    let token_len = token_bytes.len();
    if token_len == 0 || line.len() < token_len {
        return None;
    }
    let search_end = line.len() - token_len;
    for byte_pos in (0..=search_end).rev() {
        if !line.is_char_boundary(byte_pos) {
            continue;
        }
        if &line_bytes[byte_pos..byte_pos + token_len] == token_bytes {
            let col_start = display_col_of_byte(line, byte_pos);
            let col_end = col_start + display_width(token);
            return Some((col_start, col_end));
        }
    }
    None
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
fn extract_comment_body_rich(comment: &Value) -> Vec<crate::richtext::RichLine> {
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
            crate::richtext::structured_rich_with_links(html)
        }
    }
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

/// Build the full detail content for the TUI detail view.
///
/// URLs in the description and comment bodies are rendered inline as `text [url]`
/// (no separate URL list). Assets are rendered inline at the end of the content
/// via `asset_panel::section_lines`, so every attachment is reachable by scrolling.
/// `line_styles` is a parallel channel index-aligned with `lines`; both vecs are
/// always the same length.
/// When `current_user_id` is `Some(id)`, comments authored by `id` receive
/// `[editar]` and `[excluir]` affordance tokens in their header; spans are recorded
/// in `affordances` (kind `Edit`/`Delete`) for hit-testing.
/// The delete-confirm UI is rendered by the modal overlay (ADR 0039 §4), not here.
/// Pure: no I/O, no async, no time access.
pub fn build_detail_content(
    task: &Value,
    comments: &[Value],
    user_map: &HashMap<i64, String>,
    inner_width: usize,
    current_user_id: Option<i64>,
) -> DetailContent {
    let mut lines: Vec<String> = vec![];
    let mut line_styles: Vec<Vec<StyleRun>> = vec![];
    let mut affordances: Vec<LocalAffordance> = vec![];

    let header_lines = build_header_lines(task, user_map, inner_width);
    let header_count = header_lines.len();
    lines.extend(header_lines);
    line_styles.extend(std::iter::repeat_n(vec![], header_count));

    lines.push(String::new());
    line_styles.push(vec![]);

    let body_panel_start = lines.len();
    let (body_lines, body_styles, body_url_affs) = build_body_lines(task, inner_width);
    for aff in body_url_affs {
        affordances.push(LocalAffordance {
            line_idx: body_panel_start + aff.line_idx,
            col_start: aff.col_start,
            col_end: aff.col_end,
            kind: aff.kind,
        });
    }
    lines.extend(body_lines);
    line_styles.extend(body_styles);

    let mut comment_spans: Vec<(usize, usize)> = Vec::new();

    if !comments.is_empty() {
        lines.push(String::new());
        line_styles.push(vec![]);

        let comment_start_idx = lines.len();
        let (comment_lines, comment_styles, card_affs, card_ranges) =
            build_comment_lines(comments, inner_width, current_user_id);
        for a in card_affs {
            affordances.push(LocalAffordance {
                line_idx: comment_start_idx + a.line_idx,
                col_start: a.col_start,
                col_end: a.col_end,
                kind: a.kind,
            });
        }
        for (start, count) in card_ranges {
            comment_spans.push((comment_start_idx + start, count));
        }
        lines.extend(comment_lines);
        line_styles.extend(comment_styles);
    }

    let assets = crate::controller::extract_assets(task, comments);
    splice_asset_section(
        &assets,
        inner_width,
        &mut lines,
        &mut line_styles,
        &mut affordances,
    );

    debug_assert_eq!(
        lines.len(),
        line_styles.len(),
        "lines and line_styles must remain index-aligned"
    );

    DetailContent {
        lines,
        line_styles,
        affordances,
        comment_spans,
    }
}

/// Appends the asset panel section (blank separator, rendered rows, and
/// `OpenAsset` affordances) to the running `lines`/`line_styles`/`affordances`
/// vecs kept by `build_detail_content`.
///
/// Called only when the asset list is non-empty; the blank separator line is
/// included here so the caller's index alignment invariant is preserved.
fn splice_asset_section(
    assets: &[Asset],
    inner_width: usize,
    lines: &mut Vec<String>,
    line_styles: &mut Vec<Vec<StyleRun>>,
    affordances: &mut Vec<LocalAffordance>,
) {
    use crate::tui::screens::asset_panel;

    if assets.is_empty() {
        return;
    }

    lines.push(String::new());
    line_styles.push(vec![]);

    let content_width = asset_panel::inline_content_width(inner_width);
    let section_base_idx = lines.len();
    for (section_idx, (text, runs)) in asset_panel::section_lines(assets, content_width)
        .into_iter()
        .enumerate()
    {
        if let Some(asset_idx) =
            asset_panel::asset_index_for_section_row(assets, content_width, section_idx)
        {
            let link_span = runs
                .iter()
                .find(|r| r.style == crate::richtext::RichStyle::Link);
            if let Some(span) = link_span {
                affordances.push(LocalAffordance {
                    line_idx: section_base_idx + section_idx,
                    col_start: span.start,
                    col_end: span.start + span.len,
                    kind: AffordanceKind::OpenAsset(assets[asset_idx].url.clone()),
                });
            }
        }
        lines.push(text);
        line_styles.push(runs);
    }
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
