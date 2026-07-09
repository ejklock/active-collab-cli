//! TUI detail-content output adapter (ADR 0049).
//!
//! Owns `build_detail_content` and its builders: the header/meta panel, the
//! wrapped description body (with inline URL affordances, ADR 0043 §2), the
//! comment cards, and the shared rounded-panel primitives (`panel_box`,
//! `panel_box_rich`, `comment_box`). The `DetailContent` type and the
//! affordance emission contract are unchanged by this module split.

use super::{
    AffordanceKind, Asset, LocalAffordance, StyleRun, BODY_LEFT_CHROME_COLS, BOX_BL, BOX_BR, BOX_H,
    BOX_TL, BOX_TR, BOX_V, PANEL_HPAD,
};
use crate::i18n::t;
use serde_json::Value;
use std::collections::HashMap;

/// Clip marker used when a panel header does not fit the available width.
pub(crate) const ELLIPSIS: &str = "\u{2026}";

/// Separator glyph used between a comment card's author and timestamp.
pub(crate) const MIDDOT: &str = "\u{00B7}";

pub(crate) const PANEL_VPAD: usize = 1;

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
        let plain_dw = super::display_width(&plain);
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
        let ndw = super::display_width(&np);
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
        let ndw = super::display_width(&np);
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
    for m in super::body_url_re().find_iter(joined) {
        let col_end = super::display_col_of_byte(joined, m.end());
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
    let spans = super::collect_link_spans(joined_text);
    for span in &spans {
        let url_inner = &joined_text[span.start..span.end];
        let normalized = normalize_link_url(url_inner);
        if !super::is_openable_url(&normalized) && !normalized.starts_with("mailto:") {
            continue;
        }
        let url_col_start = super::display_col_of_byte(joined_text, span.start);
        let url_col_end = url_col_start + super::display_width(url_inner);
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

/// Format a prefixed asset row with a hanging indent on continuation lines.
///
/// `prefix` is the fixed part (e.g. `"[1] ↗ "`), `label` is the potentially
/// long text that follows.  The first output line is `"{prefix}{label_start}"`;
/// continuation lines are indented by `display_width(prefix)` spaces so the
/// label text aligns under itself.  When everything fits in `width`, exactly
/// one line is returned.  `width == 0` returns `[prefix.to_string()]`.
pub(crate) fn format_asset_row(prefix: &str, label: &str, width: usize) -> Vec<String> {
    let prefix_dw = super::display_width(prefix);
    if width == 0 || prefix_dw >= width {
        return vec![format!("{prefix}{label}")];
    }
    let label_width = width.saturating_sub(prefix_dw);
    let label_lines = super::wrap_text(label, label_width);
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
        let w = super::display_width(&span.text);
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

/// Compute the fitted (clipped-if-needed) top-border label and its display width
/// for a panel of the given full box `width`.
///
/// Shared by `panel_box` (which draws the fitted label into the border) and
/// `panel_title_run` (which styles that same span) so the two can never drift
/// apart (ADR 0063).
pub(crate) fn fit_panel_header(label: &str, width: usize) -> (String, usize) {
    let inner = width.saturating_sub(2);
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
    let fitted_dw = super::display_width(&header_fitted);
    (header_fitted, fitted_dw)
}

/// Layout-emitted `StyleRun` covering a panel's fitted top-border label span.
///
/// `start` is always 2 (past `BOX_TL` + one `BOX_H` cell) — matching the
/// coordinates `panel_box` actually draws. Returns `None` when the panel is too
/// narrow to draw (`width < 4`, the same guard `panel_box` uses) or the fitted
/// label is empty.
pub(crate) fn panel_title_run(label: &str, width: usize) -> Option<StyleRun> {
    use crate::richtext::RichStyle;
    if width < 4 {
        return None;
    }
    let (_, fitted_dw) = fit_panel_header(label, width);
    if fitted_dw == 0 {
        return None;
    }
    Some(StyleRun {
        start: 2,
        len: fitted_dw,
        style: RichStyle::PanelTitle,
    })
}

/// Push a `PanelTitle` run onto `line_styles[0]` (the panel's top-border row).
///
/// No-op when `line_styles` is empty or `panel_title_run` returns `None`.
fn seed_panel_title_run(line_styles: &mut [Vec<StyleRun>], label: &str, width: usize) {
    let Some(run) = panel_title_run(label, width) else {
        return;
    };
    if let Some(first) = line_styles.first_mut() {
        first.push(run);
    }
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
    let content_width = super::panel_content_width(width);
    let hpad = " ".repeat(PANEL_HPAD);

    let (header_fitted, fitted_dw) = fit_panel_header(label, width);
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
        let fitted = super::fit_to_display_width(line, content_width);
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
        let wrapped = super::wrap_text(body, super::panel_content_width(width));
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

pub(crate) fn build_body_lines(
    task: &Value,
    inner_width: usize,
) -> (Vec<String>, Vec<Vec<StyleRun>>, Vec<LocalAffordance>) {
    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let rich_lines = crate::richtext::structured_rich_with_links(body_html);
    let content_width = super::panel_content_width(inner_width);
    let body_rich = if rich_lines.is_empty() {
        let fallback_wrapped = super::wrap_text(&t("(no description)"), content_width);
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
    let (lines, mut styles) = unzip_boxed(boxed);
    seed_panel_title_run(&mut styles, &t("Description"), inner_width);

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
        let fragments = super::wrap_rich(&line, content_width);
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
pub(crate) struct CardAffordance {
    line_idx: usize,
    col_start: usize,
    col_end: usize,
    kind: AffordanceKind,
}

pub(crate) type CommentLinesOutput = (
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

pub(crate) fn build_comment_lines(
    comments: &[Value],
    inner_width: usize,
    current_user_id: Option<i64>,
) -> CommentLinesOutput {
    if comments.is_empty() {
        return (vec![], vec![], vec![], vec![]);
    }
    let outer_content = super::panel_content_width(inner_width);
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
        let when = super::fmt_ts(comment.get("created_on").unwrap_or(&Value::Null));
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

    let mut merged_styles = merge_nested_styles_into_outer(
        &outer_lines,
        &outer_box_styles,
        &nested_styles,
        content_start,
        content_end,
        chrome,
    );
    seed_panel_title_run(&mut merged_styles, &label, inner_width);

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
    let content_width = super::panel_content_width(card_width);
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
            let col_start = super::display_col_of_byte(line, byte_pos);
            let col_end = col_start + super::display_width(token);
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
    let mut header_styles: Vec<Vec<StyleRun>> = vec![vec![]; header_count];
    seed_panel_title_run(&mut header_styles, &t("Details"), inner_width);
    line_styles.extend(header_styles);

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
    let content_width = super::panel_content_width(inner_width);
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
    let start = super::fmt_date(task.get("start_on").unwrap_or(&Value::Null));
    let due = super::fmt_date(task.get("due_on").unwrap_or(&Value::Null));
    let estimate = format!(
        "{}h",
        super::fmt_hours(task.get("estimate").unwrap_or(&Value::Null))
    );
    let logged = format!(
        "{}h",
        super::fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null))
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
    let value_lines = super::wrap_text(value, value_width);
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
