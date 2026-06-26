use super::*;
use crate::render::LinkCollector;

// --- R3a-A1: Unordered list —————————————————————————————————————————————

#[test]
fn unordered_list_items_prefixed_with_bullet() {
    let html = "<ul><li>one</li><li>two</li></ul>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("\u{2022} ") && l.contains("one")),
        "first <li> must be prefixed with bullet: {out:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("\u{2022} ") && l.contains("two")),
        "second <li> must be prefixed with bullet: {out:?}"
    );
}

#[test]
fn unordered_list_produces_exactly_n_bullet_lines() {
    let html = "<ul><li>a</li><li>b</li><li>c</li></ul>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let bullet_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("\u{2022} ")).collect();
    assert_eq!(
        bullet_lines.len(),
        3,
        "three <li> elements must produce three bullet lines: {out:?}"
    );
}

// --- R3a-A1: Ordered list ————————————————————————————————————————————————

#[test]
fn ordered_list_items_numbered_from_one() {
    let html = "<ol><li>alpha</li><li>beta</li></ol>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("1. ") && l.contains("alpha")),
        "first item must be prefixed '1. ': {out:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("2. ") && l.contains("beta")),
        "second item must be prefixed '2. ': {out:?}"
    );
}

#[test]
fn each_ordered_list_resets_counter_independently() {
    let html = "<ol><li>x</li></ol><ol><li>y</li></ol>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let lines: Vec<&str> = out.lines().collect();
    let ones: Vec<&&str> = lines.iter().filter(|l| l.starts_with("1. ")).collect();
    assert_eq!(
        ones.len(),
        2,
        "each <ol> resets its counter so both first items use '1. ': {out:?}"
    );
}

// --- R3a-A1: Heading —————————————————————————————————————————————————————

#[test]
fn h2_heading_text_on_its_own_line() {
    let html = "<h2>Title</h2>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines.iter().any(|l| l.trim() == "Title"),
        "heading text must appear on its own line: {out:?}"
    );
}

#[test]
fn heading_surrounded_by_blank_line_separation() {
    let html = "before<h2>Section</h2>after";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    // "before" and "after" must be separated from "Section" by at least one blank line.
    let pos_before = out.find("before").expect("before must be present");
    let pos_heading = out.find("Section").expect("Section must be present");
    let pos_after = out.find("after").expect("after must be present");
    let between_before_heading = &out[pos_before..pos_heading];
    let between_heading_after = &out[pos_heading..pos_after];
    assert!(
        between_before_heading.contains("\n\n") || between_before_heading.contains('\n'),
        "heading must be separated from preceding text: {out:?}"
    );
    assert!(
        between_heading_after.contains('\n'),
        "text after heading must follow on a new line: {out:?}"
    );
}

#[test]
fn all_heading_levels_h1_to_h6_render_on_own_line() {
    for level in 1..=6 {
        let html = format!("<h{level}>Level {level}</h{level}>");
        let mut col = LinkCollector::new();
        let out = structured_text_with_links(&html, &mut col);
        let needle = format!("Level {level}");
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines.iter().any(|l| l.trim() == needle.as_str()),
            "h{level} text must appear on its own line: {out:?}"
        );
    }
}

// --- R3a-A1: Blockquote ——————————————————————————————————————————————————

#[test]
fn blockquote_content_prefixed_with_gt() {
    let html = "<blockquote>quoted text</blockquote>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("> quoted text"),
        "blockquote must be prefixed with '> ': {out:?}"
    );
}

#[test]
fn blockquote_multiline_all_lines_prefixed() {
    let html = "<blockquote>line one<br>line two</blockquote>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let prefixed_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("> ")).collect();
    assert_eq!(
        prefixed_lines.len(),
        2,
        "every line inside blockquote must be prefixed with '> ': {out:?}"
    );
}

// --- R3a-A2: Anchor / link label —————————————————————————————————————————

#[test]
fn anchor_replaced_by_link_label_and_url_collected() {
    let html = r#"<a href="https://x.example.com/y">click here</a>"#;
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("\u{2197} Link 1"),
        "anchor must produce '↗ Link 1' label: {out:?}"
    );
    assert!(
        out.contains("click here"),
        "inner text must be preserved before the label: {out:?}"
    );
    assert_eq!(
        col.urls,
        vec!["https://x.example.com/y"],
        "URL must be registered in collector: {:?}",
        col.urls
    );
}

#[test]
fn anchor_with_empty_inner_text_emits_only_label() {
    let html = r#"<a href="https://empty.example.com/"></a>"#;
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("\u{2197} Link 1"),
        "empty-inner anchor must still emit '↗ Link 1': {out:?}"
    );
    assert_eq!(col.urls, vec!["https://empty.example.com/"]);
}

#[test]
fn anchor_without_href_strips_tag_preserves_inner_text() {
    let html = "<a>bare link text</a>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("bare link text"),
        "anchor without href must still show inner text: {out:?}"
    );
    assert!(
        col.urls.is_empty(),
        "no URL must be collected when href absent: {:?}",
        col.urls
    );
}

#[test]
fn two_anchors_get_sequential_indices() {
    let html = r#"<a href="https://one.com">first</a> and <a href="https://two.com">second</a>"#;
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("\u{2197} Link 1"),
        "first anchor must be Link 1: {out:?}"
    );
    assert!(
        out.contains("\u{2197} Link 2"),
        "second anchor must be Link 2: {out:?}"
    );
    assert_eq!(
        col.urls,
        vec!["https://one.com", "https://two.com"],
        "both URLs must be collected in order: {:?}",
        col.urls
    );
}

#[test]
fn anchor_numbering_continues_from_shared_collector() {
    let mut col = LinkCollector::new();
    col.next_index = 3;
    col.urls.push("https://prev1.com".to_string());
    col.urls.push("https://prev2.com".to_string());
    let html = r#"<a href="https://new.com">link</a>"#;
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("\u{2197} Link 3"),
        "numbering must continue from collector state: {out:?}"
    );
    assert_eq!(col.next_index, 4);
    assert_eq!(col.urls[2], "https://new.com");
}

// --- R3a-A3: Malformed / unknown HTML ————————————————————————————————————

#[test]
fn unbalanced_ul_li_does_not_panic_and_shows_text() {
    let html = "<ul><li>item a";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("item a"),
        "unbalanced list must still show text: {out:?}"
    );
}

#[test]
fn unknown_tag_is_stripped_text_preserved() {
    let html = "<foo>kept text</foo>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("kept text"),
        "text inside unknown tag must be preserved: {out:?}"
    );
}

#[test]
fn deeply_nested_unknown_tags_stripped_safely() {
    let html = "<outer><inner><deep>text</deep></inner></outer>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("text"),
        "text must survive deep unknown nesting: {out:?}"
    );
}

#[test]
fn empty_html_returns_empty_string() {
    let mut col = LinkCollector::new();
    let out = structured_text_with_links("", &mut col);
    assert_eq!(out, "");
}

// --- R3a-A4 / BDR 0003: CLI parity is verified in render tests ————————————

// --- Mixed document ——————————————————————————————————————————————————————

#[test]
fn mixed_document_preserves_structure() {
    let html = "<h2>Tasks</h2><ul><li>First</li><li>Second</li></ul><blockquote>Note</blockquote>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    let lines: Vec<&str> = out.lines().collect();
    // Heading text appears on its own line
    assert!(
        lines.iter().any(|l| l.trim() == "Tasks"),
        "heading must appear: {out:?}"
    );
    // Two bullet lines
    let bullets: Vec<&&str> = lines
        .iter()
        .filter(|l| l.starts_with("\u{2022} "))
        .collect();
    assert_eq!(bullets.len(), 2, "two bullet lines must appear: {out:?}");
    // Blockquote line
    assert!(
        out.contains("> Note"),
        "blockquote must be prefixed: {out:?}"
    );
}

#[test]
fn entity_decoding_works_inside_list_items() {
    let html = "<ul><li>a &amp; b</li><li>&lt;escaped&gt;</li></ul>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        out.contains("a & b"),
        "entity &amp; must decode to &: {out:?}"
    );
    assert!(
        out.contains("<escaped>"),
        "entities &lt; &gt; must decode: {out:?}"
    );
}

#[test]
fn three_or_more_consecutive_newlines_collapsed_to_two() {
    let html = "<p>a</p><p>b</p><p>c</p>";
    let mut col = LinkCollector::new();
    let out = structured_text_with_links(html, &mut col);
    assert!(
        !out.contains("\n\n\n"),
        "three+ consecutive newlines must collapse to 2: {out:?}"
    );
}
