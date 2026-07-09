use regex::Regex;
use std::sync::OnceLock;

/// Emphasis kind for a text span in the rich-line representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RichStyle {
    Plain,
    Bold,
    Italic,
    Code,
    Strike,
    Underline,
    /// Layout-emitted link affordance: muted green + underlined.
    ///
    /// Used when linkness is a structural layout fact (e.g. asset rows), not
    /// inferred from visible URL text. Body links use the `link_segments` path.
    Link,
    /// Layout-emitted edit affordance: soft cyan + underlined.
    ///
    /// Applied structurally to the `[editar]` token span on own-comment card
    /// headers — same coordinates as the hit-test span (single source of truth,
    /// ADR 0032 / ADR 0041).
    EditAffordance,
    /// Layout-emitted delete affordance: destructive red + underlined.
    ///
    /// Applied structurally to the `[excluir]` token span on own-comment card
    /// headers — same coordinates as the hit-test span (single source of truth,
    /// ADR 0032 / ADR 0041).
    DeleteAffordance,
    /// Layout-emitted panel title: accent + bold.
    ///
    /// Applied structurally over a panel's top-border label span (the Details,
    /// Description and Comments section panels), never produced by the HTML
    /// parser (ADR 0063).
    PanelTitle,
}

/// A single styled text fragment within a rich line.
#[derive(Debug, Clone, PartialEq)]
pub struct RichSpan {
    pub text: String,
    pub style: RichStyle,
}

/// A logical line composed of styled spans.
pub type RichLine = Vec<RichSpan>;

fn any_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("any_tag_re is a valid pattern"))
}

/// Returns the `href` attribute value from an opening `<a …>` tag string, if present.
fn extract_href(tag: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?i)\bhref=["']([^"']+)["']"#).expect("href attr re is a valid pattern")
    });
    re.captures(tag).map(|c| c[1].to_string())
}

/// Prefixes every non-empty line of `text` with `> `.
fn prefix_blockquote_lines(text: &str) -> String {
    text.lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Emits an inline link token for `<a href=URL>text</a>`.
///
/// Follows the ActiveCollab `toPlainText` convention:
/// - non-empty text that differs from the URL → `text [display_url]`
/// - empty text or text equal to URL → `[display_url]` (no duplication)
///
/// For `mailto:` hrefs, the scheme is stripped from the bracketed display so the
/// reader sees the bare address (e.g. `[a@b.com]`). The scheme is re-added by the
/// click path when building the open command.
fn emit_anchor_label(inner_text: &str, url: &str) -> String {
    let display_url = strip_mailto_scheme(url);
    let trimmed = inner_text.trim();
    if trimmed.is_empty() || trimmed == url || trimmed == display_url {
        format!("[{display_url}]")
    } else {
        format!("{trimmed} [{display_url}]")
    }
}

/// Returns `url` with a leading `mailto:` prefix removed, or the original string.
fn strip_mailto_scheme(url: &str) -> &str {
    url.strip_prefix("mailto:").unwrap_or(url)
}

/// Returns the bullet/number prefix for the next `<li>` inside `list_stack`.
///
/// Mutates the counter of an `Ordered` entry in place so successive items
/// increment naturally. Falls back to `• ` when the stack is empty (malformed).
fn next_li_prefix(list_stack: &mut [ListKind]) -> String {
    match list_stack.last_mut() {
        Some(ListKind::Unordered) => "\u{2022} ".to_string(),
        Some(ListKind::Ordered(n)) => {
            let p = format!("{n}. ");
            *n += 1;
            p
        }
        None => "\u{2022} ".to_string(),
    }
}

/// State that controls how `<li>` items are prefixed.
#[derive(Clone, PartialEq)]
enum ListKind {
    Unordered,
    Ordered(u32),
}

/// Active emphasis modifier — tracks nesting of bold/italic/code/strike/underline tags.
#[derive(Clone, Copy, PartialEq)]
enum EmphasisKind {
    Bold,
    Italic,
    Code,
    Strike,
    Underline,
}

/// Routing target for text tokens in the presence of nested contexts.
#[derive(Clone, Copy)]
enum Context {
    Main,
    Anchor,
    Blockquote,
    Heading,
    Pre,
    Table,
}

/// Returns the highest-priority active context from an ordered list of `(flag, ctx)` pairs.
fn resolve_context(pairs: &[(bool, Context)]) -> Context {
    pairs
        .iter()
        .find(|(flag, _)| *flag)
        .map_or(Context::Main, |(_, ctx)| *ctx)
}

/// One cell in a table row, recording whether the tag was `<th>`.
struct TableCell {
    text: String,
    is_header: bool,
}

/// One row of collected cells inside a `<table>` context.
struct TableRow {
    cells: Vec<TableCell>,
}

/// Mutable parse state for the rich parser.
struct RichParseState {
    /// Completed rich lines (split on '\n').
    lines: Vec<RichLine>,
    /// The spans currently being built for the active line.
    current_line: RichLine,
    list_stack: Vec<ListKind>,
    emphasis_stack: Vec<EmphasisKind>,
    in_anchor: bool,
    anchor_href: Option<String>,
    /// Rich spans accumulated inside an `<a>` tag.
    anchor_spans: Vec<RichSpan>,
    in_blockquote: bool,
    blockquote_inner: String,
    in_heading: bool,
    heading_inner: String,
    in_pre: bool,
    /// Completed lines accumulated inside a `<pre>` block.
    pre_lines: Vec<RichLine>,
    /// The line currently being built inside `<pre>`.
    pre_current_line: RichLine,
    in_table: bool,
    /// Accumulated rows while inside `<table>`.
    table_rows: Vec<TableRow>,
    /// Current row being built inside `<table>`.
    table_current_row: Option<TableRow>,
    /// Current cell being built inside `<tr>`.
    table_current_cell: Option<TableCell>,
}

impl RichParseState {
    fn new() -> Self {
        RichParseState {
            lines: Vec::new(),
            current_line: Vec::new(),
            list_stack: Vec::new(),
            emphasis_stack: Vec::new(),
            in_anchor: false,
            anchor_href: None,
            anchor_spans: Vec::new(),
            in_blockquote: false,
            blockquote_inner: String::new(),
            in_heading: false,
            heading_inner: String::new(),
            in_pre: false,
            pre_lines: Vec::new(),
            pre_current_line: Vec::new(),
            in_table: false,
            table_rows: Vec::new(),
            table_current_row: None,
            table_current_cell: None,
        }
    }

    /// Current emphasis derived from the top of the emphasis stack.
    fn current_emphasis(&self) -> RichStyle {
        match self.emphasis_stack.last() {
            Some(EmphasisKind::Bold) => RichStyle::Bold,
            Some(EmphasisKind::Italic) => RichStyle::Italic,
            Some(EmphasisKind::Code) => RichStyle::Code,
            Some(EmphasisKind::Strike) => RichStyle::Strike,
            Some(EmphasisKind::Underline) => RichStyle::Underline,
            None => RichStyle::Plain,
        }
    }

    fn active_context(&self) -> Context {
        resolve_context(&[
            (self.in_table, Context::Table),
            (self.in_pre, Context::Pre),
            (self.in_anchor, Context::Anchor),
            (self.in_heading, Context::Heading),
            (self.in_blockquote, Context::Blockquote),
        ])
    }

    /// Commit `current_line` and start a new one.
    fn break_line(&mut self) {
        let finished = std::mem::take(&mut self.current_line);
        self.lines.push(finished);
    }

    /// Push a newline into whichever context is active.
    fn push_newline_to_context(&mut self) {
        match self.active_context() {
            Context::Anchor => {
                if let Some(span) = self.anchor_spans.last_mut() {
                    span.text.push('\n');
                }
            }
            Context::Heading => self.heading_inner.push('\n'),
            Context::Blockquote => self.blockquote_inner.push('\n'),
            Context::Pre => self.break_pre_line(),
            Context::Table => {}
            Context::Main => self.break_line(),
        }
    }

    /// Commit the current pre line and start a new one.
    fn break_pre_line(&mut self) {
        let finished = std::mem::take(&mut self.pre_current_line);
        self.pre_lines.push(finished);
    }

    /// Accumulate a decoded text fragment into the correct context.
    fn accumulate_text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        let ctx = self.active_context();
        match ctx {
            Context::Pre => {
                accumulate_pre_text(
                    raw,
                    self.current_emphasis(),
                    &mut self.pre_lines,
                    &mut self.pre_current_line,
                );
            }
            Context::Table => {
                let decoded = html_escape::decode_html_entities(raw).into_owned();
                if let Some(cell) = self.table_current_cell.as_mut() {
                    cell.text.push_str(&decoded);
                }
            }
            _ => {
                let decoded = html_escape::decode_html_entities(raw).into_owned();
                let em = self.current_emphasis();
                match ctx {
                    Context::Anchor => push_to_anchor_spans(&mut self.anchor_spans, &decoded, em),
                    Context::Blockquote => self.blockquote_inner.push_str(&decoded),
                    Context::Heading => self.heading_inner.push_str(&decoded),
                    Context::Main => push_to_current_line(&mut self.current_line, &decoded, em),
                    Context::Pre | Context::Table => unreachable!(),
                }
            }
        }
    }

    /// Append a plain-text span to the current output line.
    fn push_plain(&mut self, text: &str) {
        push_to_current_line(&mut self.current_line, text, RichStyle::Plain);
    }

    /// Finalize and return all rich lines.
    fn finish(mut self) -> Vec<RichLine> {
        if !self.current_line.is_empty() {
            self.lines.push(std::mem::take(&mut self.current_line));
        }
        self.lines
    }
}

/// Append `text` to `spans`, merging into the last span when the style matches.
fn push_to_current_line(spans: &mut RichLine, text: &str, style: RichStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.style == style {
            last.text.push_str(text);
            return;
        }
    }
    spans.push(RichSpan {
        text: text.to_string(),
        style,
    });
}

/// Append text to the anchor's internal span buffer, merging same-style runs.
fn push_to_anchor_spans(spans: &mut Vec<RichSpan>, text: &str, style: RichStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.style == style {
            last.text.push_str(text);
            return;
        }
    }
    spans.push(RichSpan {
        text: text.to_string(),
        style,
    });
}

/// Flatten a `Vec<RichSpan>` to a plain string.
fn spans_to_text(spans: &[RichSpan]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

/// Flush a blockquote inner buffer as prefixed plain lines appended to `lines`.
fn flush_blockquote_rich(inner: &str, lines: &mut Vec<RichLine>) {
    let trimmed = inner.trim_matches('\n');
    if trimmed.is_empty() {
        return;
    }
    let prefixed = prefix_blockquote_lines(trimmed);
    lines.push(vec![]);
    for l in prefixed.split('\n') {
        lines.push(vec![RichSpan {
            text: l.to_string(),
            style: RichStyle::Plain,
        }]);
    }
    lines.push(vec![]);
}

/// Flush a heading inner buffer as a bold line appended to `lines`.
fn flush_heading_rich(inner: &str, lines: &mut Vec<RichLine>) {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return;
    }
    lines.push(vec![]);
    lines.push(vec![RichSpan {
        text: trimmed.to_string(),
        style: RichStyle::Bold,
    }]);
    lines.push(vec![]);
}

/// Flush any unclosed contexts remaining after the final tag.
fn flush_open_contexts_rich(state: &mut RichParseState) {
    if state.in_anchor {
        let inner_text = spans_to_text(&state.anchor_spans);
        let label = match state.anchor_href.take() {
            Some(url) => emit_anchor_label(&inner_text, &url),
            None => inner_text,
        };
        push_to_current_line(&mut state.current_line, &label, RichStyle::Plain);
    }
    if state.in_blockquote && !state.blockquote_inner.is_empty() {
        let inner = state.blockquote_inner.clone();
        state.break_line();
        flush_blockquote_rich(&inner, &mut state.lines);
    }
    if state.in_heading && !state.heading_inner.is_empty() {
        let inner = state.heading_inner.clone();
        state.break_line();
        flush_heading_rich(&inner, &mut state.lines);
    }
    if state.in_pre && (!state.pre_lines.is_empty() || !state.pre_current_line.is_empty()) {
        let partial = std::mem::take(&mut state.pre_current_line);
        state.pre_lines.push(partial);
        let pre_lines = std::mem::take(&mut state.pre_lines);
        state.break_line();
        flush_pre_rich_lines(pre_lines, &mut state.lines);
    }
    if state.in_table {
        close_current_table_cell(state);
        close_current_table_row(state);
        let rows = std::mem::take(&mut state.table_rows);
        state.break_line();
        flush_table_rich(rows, &mut state.lines);
    }
}

/// Thin tag dispatcher for the rich parser.
fn process_tag_rich(raw_tag: &str, state: &mut RichParseState) {
    let tag_lower = raw_tag.to_ascii_lowercase();
    let tag_body = tag_lower
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    let is_closing = tag_body.starts_with('/');
    let element = if is_closing {
        tag_body.trim_start_matches('/').trim()
    } else {
        tag_body.split_whitespace().next().unwrap_or("")
    };

    match element {
        "br" | "p" | "div" => state.push_newline_to_context(),
        "ul" => handle_list_tag_rich(ListKind::Unordered, is_closing, state),
        "ol" => handle_list_tag_rich(ListKind::Ordered(1), is_closing, state),
        "li" if !is_closing => handle_list_item_open_rich(state),
        "li" => {}
        "blockquote" => handle_blockquote_tag_rich(is_closing, state),
        "a" => handle_anchor_tag_rich(is_closing, raw_tag, state),
        "strong" | "b" => handle_emphasis_tag_rich(EmphasisKind::Bold, is_closing, state),
        "em" | "i" => handle_emphasis_tag_rich(EmphasisKind::Italic, is_closing, state),
        "code" => handle_emphasis_tag_rich(EmphasisKind::Code, is_closing, state),
        "strike" | "del" => handle_emphasis_tag_rich(EmphasisKind::Strike, is_closing, state),
        "u" => handle_emphasis_tag_rich(EmphasisKind::Underline, is_closing, state),
        "pre" => handle_pre_tag_rich(is_closing, state),
        "table" => handle_table_tag_rich(is_closing, state),
        "thead" | "tbody" | "tfoot" => {}
        "tr" => handle_tr_tag_rich(is_closing, state),
        "td" => handle_td_th_tag_rich(is_closing, false, state),
        "th" => handle_td_th_tag_rich(is_closing, true, state),
        h if is_heading_tag(h) => handle_heading_tag_rich(is_closing, state),
        _ => {}
    }
}

fn handle_list_tag_rich(kind: ListKind, is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        state.list_stack.pop();
    } else {
        state.list_stack.push(kind);
    }
}

fn handle_list_item_open_rich(state: &mut RichParseState) {
    let prefix = next_li_prefix(&mut state.list_stack);
    if state.in_blockquote {
        state.blockquote_inner.push('\n');
        state.blockquote_inner.push_str(&prefix);
    } else {
        state.break_line();
        state.push_plain(&prefix);
    }
}

fn handle_blockquote_tag_rich(is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        state.in_blockquote = false;
        let inner = state.blockquote_inner.clone();
        state.break_line();
        flush_blockquote_rich(&inner, &mut state.lines);
        state.blockquote_inner.clear();
    } else {
        state.in_blockquote = true;
        state.blockquote_inner.clear();
    }
}

fn handle_heading_tag_rich(is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        state.in_heading = false;
        let inner = state.heading_inner.clone();
        state.break_line();
        flush_heading_rich(&inner, &mut state.lines);
        state.heading_inner.clear();
    } else {
        state.in_heading = true;
        state.heading_inner.clear();
    }
}

fn handle_anchor_tag_rich(is_closing: bool, raw_tag: &str, state: &mut RichParseState) {
    if !is_closing {
        if let Some(url) = extract_href(raw_tag) {
            state.in_anchor = true;
            state.anchor_href = Some(url);
            state.anchor_spans.clear();
        }
    } else if state.in_anchor {
        close_anchor_rich(state);
    }
}

fn close_anchor_rich(state: &mut RichParseState) {
    state.in_anchor = false;
    let inner_text = spans_to_text(&state.anchor_spans);
    let label = match state.anchor_href.take() {
        Some(url) => emit_anchor_label(&inner_text, &url),
        None => inner_text,
    };
    push_to_current_line(&mut state.current_line, &label, RichStyle::Plain);
    state.anchor_spans.clear();
}

fn handle_emphasis_tag_rich(kind: EmphasisKind, is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        state.emphasis_stack.retain(|k| k != &kind);
    } else {
        state.emphasis_stack.push(kind);
    }
}

fn handle_pre_tag_rich(is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        state.in_pre = false;
        let partial = std::mem::take(&mut state.pre_current_line);
        state.pre_lines.push(partial);
        let pre_lines = std::mem::take(&mut state.pre_lines);
        state.break_line();
        flush_pre_rich_lines(pre_lines, &mut state.lines);
    } else {
        state.in_pre = true;
        state.pre_lines.clear();
        state.pre_current_line.clear();
    }
}

fn handle_table_tag_rich(is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        close_current_table_cell(state);
        close_current_table_row(state);
        let rows = std::mem::take(&mut state.table_rows);
        state.in_table = false;
        state.break_line();
        flush_table_rich(rows, &mut state.lines);
    } else {
        state.in_table = true;
        state.table_rows.clear();
        state.table_current_row = None;
        state.table_current_cell = None;
    }
}

fn handle_tr_tag_rich(is_closing: bool, state: &mut RichParseState) {
    if is_closing {
        close_current_table_cell(state);
        close_current_table_row(state);
    } else {
        close_current_table_cell(state);
        close_current_table_row(state);
        state.table_current_row = Some(TableRow { cells: Vec::new() });
    }
}

fn handle_td_th_tag_rich(is_closing: bool, is_header: bool, state: &mut RichParseState) {
    if is_closing {
        close_current_table_cell(state);
    } else {
        close_current_table_cell(state);
        state.table_current_cell = Some(TableCell {
            text: String::new(),
            is_header,
        });
    }
}

fn close_current_table_cell(state: &mut RichParseState) {
    if let Some(cell) = state.table_current_cell.take() {
        if let Some(row) = state.table_current_row.as_mut() {
            row.cells.push(cell);
        }
    }
}

fn close_current_table_row(state: &mut RichParseState) {
    if let Some(row) = state.table_current_row.take() {
        state.table_rows.push(row);
    }
}

/// Split `raw` on newlines and push styled spans into the `<pre>` line buffer.
///
/// The style defaults to `Code` when no emphasis tag is active; emphasis tags
/// inside `<pre>` (e.g. `<b>`) override with their own style. Whitespace is
/// preserved verbatim — no collapsing, no entity-driven run-merging. HTML
/// entities are decoded at this point so `&amp;` etc. render correctly.
fn accumulate_pre_text(
    raw: &str,
    emphasis: RichStyle,
    pre_lines: &mut Vec<RichLine>,
    pre_current_line: &mut RichLine,
) {
    let style = if emphasis == RichStyle::Plain {
        RichStyle::Code
    } else {
        emphasis
    };
    let decoded = html_escape::decode_html_entities(raw).into_owned();
    let mut parts = decoded.split('\n');
    if let Some(first) = parts.next() {
        if !first.is_empty() {
            push_to_current_line(pre_current_line, first, style);
        }
    }
    for part in parts {
        let finished = std::mem::take(pre_current_line);
        pre_lines.push(finished);
        if !part.is_empty() {
            push_to_current_line(pre_current_line, part, style);
        }
    }
}

/// Flush accumulated `<pre>` line buffers, framing them with blank lines.
///
/// Leading and trailing blank lines inside the block are stripped so that
/// `<pre>\ncode\n</pre>` does not add spurious empty lines inside the frame.
fn flush_pre_rich_lines(pre_lines: Vec<RichLine>, lines: &mut Vec<RichLine>) {
    let all_blank = |l: &RichLine| l.is_empty() || l.iter().all(|s| s.text.is_empty());
    let start = pre_lines.iter().position(|l| !all_blank(l));
    let Some(start) = start else { return };
    let end = pre_lines
        .iter()
        .rposition(|l| !all_blank(l))
        .unwrap_or(start);
    lines.push(vec![]);
    for line in &pre_lines[start..=end] {
        lines.push(line.clone());
    }
    lines.push(vec![]);
}

/// Compute the column widths needed to align `rows` of cells.
fn compute_column_widths(rows: &[TableRow]) -> Vec<usize> {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        for (col_idx, cell) in row.cells.iter().enumerate() {
            let w = crate::render::display_width(&cell.text);
            if col_idx < widths.len() {
                if w > widths[col_idx] {
                    widths[col_idx] = w;
                }
            } else {
                widths.push(w);
            }
        }
    }
    widths
}

/// Render one table row into a `RichLine` using precomputed column widths.
fn render_table_row(row: &TableRow, col_widths: &[usize]) -> RichLine {
    let mut line: RichLine = Vec::new();
    for (col_idx, &width) in col_widths.iter().enumerate() {
        if col_idx > 0 {
            push_to_current_line(&mut line, "  ", RichStyle::Plain);
        }
        let (text, is_header) = row
            .cells
            .get(col_idx)
            .map(|c| (c.text.as_str(), c.is_header))
            .unwrap_or(("", false));
        let padded = pad_to_display_width(text, width);
        let style = if is_header {
            RichStyle::Bold
        } else {
            RichStyle::Plain
        };
        push_to_current_line(&mut line, &padded, style);
    }
    line
}

/// Pad `text` to exactly `width` display columns, truncating if wider.
fn pad_to_display_width(text: &str, width: usize) -> String {
    let w = crate::render::display_width(text);
    if w >= width {
        return text.to_string();
    }
    let mut s = text.to_string();
    for _ in 0..(width - w) {
        s.push(' ');
    }
    s
}

/// Flush a collected table as column-aligned rich lines.
fn flush_table_rich(rows: Vec<TableRow>, lines: &mut Vec<RichLine>) {
    if rows.is_empty() {
        return;
    }
    let col_widths = compute_column_widths(&rows);
    if col_widths.is_empty() {
        return;
    }
    lines.push(vec![]);
    for row in &rows {
        let rich_line = render_table_row(row, &col_widths);
        lines.push(rich_line);
    }
    lines.push(vec![]);
}

fn is_heading_tag(element: &str) -> bool {
    matches!(element, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

/// Pure HTML→`Vec<RichLine>` mapper with inline emphasis for the TUI detail view (R3b).
///
/// Extends R3a structural mapping with emphasis tracking: `<strong>`/`<b>` → Bold,
/// `<em>`/`<i>` → Italic, `<code>` → Code, `<h1>`-`<h6>` lines → whole-line Bold.
/// Anchor labels are Plain (link color is applied later by the rendering layer).
///
/// The function never panics on malformed HTML — it degrades gracefully.
/// It is pure: no I/O, no async, no time access.
pub fn structured_rich_with_links(html: &str) -> Vec<RichLine> {
    if html.is_empty() {
        return vec![];
    }

    let mut state = RichParseState::new();
    let tag_re = any_tag_re();
    let mut last_byte = 0usize;

    for m in tag_re.find_iter(html) {
        state.accumulate_text(&html[last_byte..m.start()]);
        last_byte = m.end();
        process_tag_rich(m.as_str(), &mut state);
    }

    state.accumulate_text(&html[last_byte..]);
    flush_open_contexts_rich(&mut state);

    let raw_lines = state.finish();
    normalize_rich_lines(raw_lines)
}

/// Collapse 3+ consecutive blank lines to 2, and trim leading/trailing blank lines.
fn normalize_rich_lines(lines: Vec<RichLine>) -> Vec<RichLine> {
    let mut result: Vec<RichLine> = Vec::new();
    let mut blank_run = 0usize;

    for line in lines {
        let is_blank = line.is_empty() || line.iter().all(|s| s.text.trim().is_empty());
        if is_blank {
            blank_run += 1;
            if blank_run <= 2 {
                result.push(line);
            }
        } else {
            blank_run = 0;
            result.push(line);
        }
    }

    while result.first().map(|l| l.is_empty()).unwrap_or(false) {
        result.remove(0);
    }
    while result.last().map(|l| l.is_empty()).unwrap_or(false) {
        result.pop();
    }

    result
}

#[cfg(test)]
#[path = "../tests/unit/richtext.rs"]
mod tests;
