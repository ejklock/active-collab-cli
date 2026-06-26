use regex::Regex;
use std::sync::OnceLock;

/// Emphasis kind for a text span in the rich-line representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RichStyle {
    Plain,
    Bold,
    Italic,
    Code,
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

/// Emits an inline link label and registers the URL in `collector`.
///
/// When `inner_text` is empty or all-whitespace, the label is just `↗ Link N`.
/// Otherwise `inner_text ↗ Link N`.
fn emit_anchor_label(
    inner_text: &str,
    url: &str,
    collector: &mut crate::render::LinkCollector,
) -> String {
    let n = collector.next_index;
    collector.urls.push(url.to_string());
    collector.next_index += 1;
    let trimmed = inner_text.trim();
    if trimmed.is_empty() {
        format!("\u{2197} Link {n}")
    } else {
        format!("{trimmed} \u{2197} Link {n}")
    }
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

/// Active emphasis modifier — tracks nesting of bold/italic/code tags.
#[derive(Clone, Copy, PartialEq)]
enum EmphasisKind {
    Bold,
    Italic,
    Code,
}

/// Routing target for text tokens in the presence of nested contexts.
enum Context {
    Main,
    Anchor,
    Blockquote,
    Heading,
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
        }
    }

    /// Current emphasis derived from the top of the emphasis stack.
    fn current_emphasis(&self) -> RichStyle {
        match self.emphasis_stack.last() {
            Some(EmphasisKind::Bold) => RichStyle::Bold,
            Some(EmphasisKind::Italic) => RichStyle::Italic,
            Some(EmphasisKind::Code) => RichStyle::Code,
            None => RichStyle::Plain,
        }
    }

    fn active_context(&self) -> Context {
        if self.in_anchor {
            Context::Anchor
        } else if self.in_heading {
            Context::Heading
        } else if self.in_blockquote {
            Context::Blockquote
        } else {
            Context::Main
        }
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
            Context::Main => self.break_line(),
        }
    }

    /// Accumulate a decoded text fragment into the correct context.
    fn accumulate_text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        let decoded = html_escape::decode_html_entities(raw).into_owned();
        let ctx = self.active_context();
        let em = self.current_emphasis();
        match ctx {
            Context::Anchor => push_to_anchor_spans(&mut self.anchor_spans, &decoded, em),
            Context::Blockquote => self.blockquote_inner.push_str(&decoded),
            Context::Heading => self.heading_inner.push_str(&decoded),
            Context::Main => push_to_current_line(&mut self.current_line, &decoded, em),
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
fn flush_open_contexts_rich(
    state: &mut RichParseState,
    collector: &mut crate::render::LinkCollector,
) {
    if state.in_anchor {
        let inner_text = spans_to_text(&state.anchor_spans);
        let label = match state.anchor_href.take() {
            Some(url) => emit_anchor_label(&inner_text, &url, collector),
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
}

/// Thin tag dispatcher for the rich parser.
fn process_tag_rich(
    raw_tag: &str,
    state: &mut RichParseState,
    collector: &mut crate::render::LinkCollector,
) {
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
        "br" | "p" | "div" | "tr" => state.push_newline_to_context(),
        "ul" => handle_list_tag_rich(ListKind::Unordered, is_closing, state),
        "ol" => handle_list_tag_rich(ListKind::Ordered(1), is_closing, state),
        "li" if !is_closing => handle_list_item_open_rich(state),
        "li" => {}
        "blockquote" => handle_blockquote_tag_rich(is_closing, state),
        "a" => handle_anchor_tag_rich(is_closing, raw_tag, state, collector),
        "strong" | "b" => handle_emphasis_tag_rich(EmphasisKind::Bold, is_closing, state),
        "em" | "i" => handle_emphasis_tag_rich(EmphasisKind::Italic, is_closing, state),
        "code" => handle_emphasis_tag_rich(EmphasisKind::Code, is_closing, state),
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

fn handle_anchor_tag_rich(
    is_closing: bool,
    raw_tag: &str,
    state: &mut RichParseState,
    collector: &mut crate::render::LinkCollector,
) {
    if !is_closing {
        if let Some(url) = extract_href(raw_tag) {
            state.in_anchor = true;
            state.anchor_href = Some(url);
            state.anchor_spans.clear();
        }
    } else if state.in_anchor {
        close_anchor_rich(state, collector);
    }
}

fn close_anchor_rich(state: &mut RichParseState, collector: &mut crate::render::LinkCollector) {
    state.in_anchor = false;
    let inner_text = spans_to_text(&state.anchor_spans);
    let label = match state.anchor_href.take() {
        Some(url) => emit_anchor_label(&inner_text, &url, collector),
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
pub fn structured_rich_with_links(
    html: &str,
    collector: &mut crate::render::LinkCollector,
) -> Vec<RichLine> {
    if html.is_empty() {
        return vec![];
    }

    let mut state = RichParseState::new();
    let tag_re = any_tag_re();
    let mut last_byte = 0usize;

    for m in tag_re.find_iter(html) {
        state.accumulate_text(&html[last_byte..m.start()]);
        last_byte = m.end();
        process_tag_rich(m.as_str(), &mut state, collector);
    }

    state.accumulate_text(&html[last_byte..]);
    flush_open_contexts_rich(&mut state, collector);

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

/// Flatten `Vec<RichLine>` to a plain `String`, joining lines with `'\n'`.
///
/// Mirrors the output of the former `structured_text_with_links` function so
/// that all R3a tests remain green without code changes in the test files.
/// The CLI path is unchanged — this is TUI-only.
#[allow(dead_code)]
pub fn structured_text_with_links(
    html: &str,
    collector: &mut crate::render::LinkCollector,
) -> String {
    if html.is_empty() {
        return String::new();
    }
    let rich = structured_rich_with_links(html, collector);
    let joined = rich
        .iter()
        .map(|line| line.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    joined
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .to_owned()
}

#[cfg(test)]
#[path = "../tests/unit/richtext.rs"]
mod tests;
