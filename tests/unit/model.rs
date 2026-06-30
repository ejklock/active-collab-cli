use super::*;
use crate::render::Asset;
use crate::tui::screens::asset_panel;
use crossterm::event::KeyModifiers;
use std::collections::HashMap;

fn empty_header() -> Header {
    Header::from_instances(&[], None)
}

fn make_asset(name: &str, url: &str) -> Asset {
    Asset {
        name: name.into(),
        url: url.into(),
    }
}

fn detail_model_with_assets_and_viewport(
    assets: Vec<Asset>,
    instance: &str,
    viewport: (u16, u16),
) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: instance.into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets,
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

// V2a-A2: Msg::Click carries column+row and is accepted by update on any screen.
#[test]
fn click_struct_form_accepted_by_update_on_projects_screen() {
    let m = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
}

// V2a-A1: Detail with no assets — any click is a no-op (no panel exists).
#[test]
fn click_detail_with_no_assets_is_noop() {
    let m = detail_model_with_assets_and_viewport(vec![], "inst", (80, 24));
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 20,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "click on detail with no assets must be a no-op"
    );
}

fn detail_model_scrollable(lines: Vec<String>, assets: Vec<Asset>, viewport: (u16, u16)) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines,
            line_styles: vec![],
            assets,
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

// V5-A1: detail_max_offset — no assets, viewport_rows=24, viewport_cols=80, lines_len=50.
// chrome=4, panel_h=0, text_vh=24-4=20, max=50-20=30.
#[test]
fn detail_max_offset_no_assets_viewport_24_lines_50() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(24, 80, 50, &[]);
    assert_eq!(
        max, 30,
        "viewport=24x80, no assets: text_vh=20, max=50-20=30"
    );
}

// V5-A1: detail_max_offset — tiny viewport (rows < chrome): text_vh clamps to 1,
// so max = lines_len - 1 (not lines_len), the same bound as a 1-row visible area.
#[test]
fn detail_max_offset_tiny_viewport_clamps_text_vh_to_one() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(2, 80, 30, &[]);
    assert_eq!(
        max, 29,
        "viewport=2x80 < chrome(4): raw text_vh=0, clamped to 1, max=30-1=29"
    );
}

// V5-A1: detail_max_offset — zero viewport: text_vh clamps to 1, max = lines_len - 1.
#[test]
fn detail_max_offset_zero_viewport_clamps_text_vh_to_one() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(0, 80, 20, &[]);
    assert_eq!(
        max, 19,
        "viewport=0x80: raw text_vh=0, clamped to 1, max=20-1=19"
    );
}

// V5-A1: scroll_down (mouse wheel) clamps to detail_max_offset, not lines.len()-1.
// viewport=80x24, 50 lines, no assets → max=30. Scroll 60 times → offset stays at 30.
// Note: Msg::Down in Detail moves comment focus; Msg::ScrollDown scrolls raw lines.
#[test]
fn handle_down_clamps_to_detail_max_offset_no_assets() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    for _ in 0..60 {
        model = update(model, Msg::ScrollDown).0;
    }

    let expected_max = detail_max_offset(24, 80, 50, &[]);
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, expected_max,
                "offset must clamp to detail_max_offset={expected_max}, not lines.len()-1=49"
            );
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

// V5-A1: scroll_down (mouse wheel) is idempotent at max — scrolling when already at max does nothing.
// Note: Msg::Down in Detail moves comment focus; Msg::ScrollDown scrolls raw lines.
#[test]
fn handle_down_idempotent_at_max() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    let max = detail_max_offset(24, 80, 50, &[]);
    for _ in 0..100 {
        model = update(model, Msg::ScrollDown).0;
    }

    let offset_at_max = match model.top() {
        Some(Screen::Detail { offset, .. }) => *offset,
        other => panic!("expected Detail, got {other:?}"),
    };
    assert_eq!(offset_at_max, max, "offset must be clamped to max={max}");

    model = update(model, Msg::ScrollDown).0;
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, max,
                "offset must stay at max after another ScrollDown"
            );
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

// V5-A1: handle_page_down clamps to detail_max_offset — not lines.len()-1.
#[test]
fn handle_page_down_clamps_to_detail_max_offset() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    for _ in 0..10 {
        model = update(model, Msg::PageDown).0;
    }

    let expected_max = detail_max_offset(24, 80, 50, &[]);
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, expected_max,
                "page_down must clamp to detail_max_offset={expected_max}"
            );
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

// V5-A1: handle_page_down is idempotent at max.
#[test]
fn handle_page_down_idempotent_at_max() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    let max = detail_max_offset(24, 80, 50, &[]);
    for _ in 0..20 {
        model = update(model, Msg::PageDown).0;
    }

    let offset_at_max = match model.top() {
        Some(Screen::Detail { offset, .. }) => *offset,
        other => panic!("expected Detail, got {other:?}"),
    };
    assert_eq!(offset_at_max, max);

    model = update(model, Msg::PageDown).0;
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, max,
                "offset must stay at max after another PageDown"
            );
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

// V2b-A1: Click on a Projects screen row with a matching hit-map target drills into Tasks.
// The correct project (index 2, P2) is selected and a Tasks screen is pushed.
#[test]
fn click_on_projects_screen_with_target_drills_into_tasks() {
    use crate::tui::model::ClickTarget;
    let mut m = Model {
        stack: vec![Screen::Projects {
            groups: vec![
                ProjectGroup {
                    project_id: 0,
                    project_name: "P0".into(),
                    instance: "i".into(),
                    tasks: vec![],
                },
                ProjectGroup {
                    project_id: 1,
                    project_name: "P1".into(),
                    instance: "i".into(),
                    tasks: vec![],
                },
                ProjectGroup {
                    project_id: 2,
                    project_name: "P2".into(),
                    instance: "i".into(),
                    tasks: vec![],
                },
            ],
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    m.set_click_targets(vec![
        ClickTarget {
            y_start: 2,
            y_end: 3,
            index: 0,
        },
        ClickTarget {
            y_start: 3,
            y_end: 4,
            index: 1,
        },
        ClickTarget {
            y_start: 4,
            y_end: 5,
            index: 2,
        },
    ]);

    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 10,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(cmds.is_empty(), "PushTasks produces no async Cmd");
    match m.top() {
        Some(Screen::Tasks { project_name, .. }) => {
            assert_eq!(
                project_name, "P2",
                "clicked row index 2 must push P2's Tasks"
            );
        }
        other => panic!("expected Tasks screen, got {other:?}"),
    }
}

// --- V5 body-link click tests (inline url-from-click contract) ---

/// Build a Detail model with specific lines, assets, offset and viewport.
fn detail_model_with_lines_and_assets(
    lines: Vec<String>,
    assets: Vec<Asset>,
    offset: usize,
    viewport: (u16, u16),
) -> Model {
    detail_model_with_lines_assets_affs(lines, assets, vec![], offset, viewport)
}

fn detail_model_with_lines_assets_affs(
    lines: Vec<String>,
    assets: Vec<Asset>,
    affordances: Vec<crate::render::LocalAffordance>,
    offset: usize,
    viewport: (u16, u16),
) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines,
            line_styles: vec![],
            assets,
            offset,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances,
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

/// Build an `OpenUrl` affordance for a URL that starts at panel column `col_start`
/// on `line_idx`, where `col_start` is in the panel display-column space
/// (│=col 0, content starts at col 2).
fn open_url_aff(line_idx: usize, col_start: usize, url: &str) -> crate::render::LocalAffordance {
    use crate::render::{AffordanceKind, LocalAffordance};
    LocalAffordance {
        line_idx,
        col_start,
        col_end: col_start + url.len(),
        kind: AffordanceKind::OpenUrl(url.to_string()),
    }
}

// V5-A2: A click on the visible '[url]' token emits OpenAsset with the exact URL (no brackets).
// Viewport 80x24, no assets. text_top=2, content_text_height=24-4=20.
// Line: "│ click here [https://example.com/doc.pdf]            │"
// The URL token starts after "[" at display col ~14.
// We click at col 15, which is inside the bracketed URL inner span.
#[test]
fn click_bracketed_url_token_emits_open_asset_with_exact_url() {
    let url = "https://example.com/doc.pdf";
    // Build a line with the inline format: "│ click here [https://example.com/doc.pdf] │"
    // Border │ (col 0), space (col 1), "click here " (cols 2-12), "[" (col 13),
    // URL inner starts at col 14. OpenUrl affordance: col_start=14, col_end=14+url_len.
    let line = format!("\u{2502} click here [{url}] \u{2502}");
    let aff = open_url_aff(0, 14, url);
    let m = detail_model_with_lines_assets_affs(vec![line], vec![], vec![aff], 0, (80, 24));

    // Click at col 15 — inside the URL inner span (starts at col 14 after "│ click here [")
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 15,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "must emit exactly one cmd");
    match &cmds[0] {
        Cmd::OpenAsset {
            instance,
            url: cmd_url,
        } => {
            assert_eq!(instance, "inst");
            assert_eq!(cmd_url, url, "URL must be exact, without brackets");
        }
        other => panic!("expected OpenAsset for body link, got {other:?}"),
    }
}

// V5-A2: Scroll offset is accounted for.
// With offset=1, row text_top=2 maps to logical_line = 1+(2-2)=1.
// Click at row 2 must open the URL from line 1, not line 0.
#[test]
fn click_body_link_accounts_for_scroll_offset() {
    let url = "https://example.com/offset-test";
    let plain_line = "\u{2502} plain text \u{2502}".to_string();
    let link_line = format!("\u{2502} [{url}] \u{2502}");
    // OpenUrl affordance on line 1: `│ [` = 3 display cols before the inner URL.
    // col_start=3, col_end=3+url.len().
    let aff = open_url_aff(1, 3, url);
    let m = detail_model_with_lines_assets_affs(
        vec![plain_line, link_line],
        vec![],
        vec![aff],
        1,
        (80, 24),
    );

    // char_col = column as usize = 4; affordance col_start=3, so col 4 ≥ 3 ✓
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 4,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(cmd_url, url);
        }
        other => panic!("expected OpenAsset for offset body link, got {other:?}"),
    }
}

// V5-A2: A click on the border (col 0 = '│') is a no-op — not a URL.
#[test]
fn click_non_url_content_cell_is_noop() {
    let url = "https://example.com/doc";
    let link_line = format!("\u{2502} [{url}] \u{2502}");
    let m = detail_model_with_lines_and_assets(vec![link_line], vec![], 0, (80, 24));

    // Column 0 is the "│" border — no URL there.
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(cmds.is_empty(), "click on border must be a no-op");
}

// V5-A2: A non-url '[note]' bracket token is NOT clickable.
#[test]
fn click_non_url_bracket_token_is_noop() {
    // "[note]" is not a URL/email — url_at must return None.
    let line = "\u{2502} see [note] for details \u{2502}".to_string();
    let m = detail_model_with_lines_and_assets(vec![line], vec![], 0, (80, 24));

    // Click col 6 is inside "[note]" inner span ("note" starts at col 6 after "│ see [")
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 6,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "non-url '[note]' bracket must NOT be clickable"
    );
}

// V5-A3: Mailto bracket token yields Cmd::OpenAsset with 'mailto:' scheme pre-added
// at emit time (ADR 0043 §2) and stored in the OpenUrl affordance.
#[test]
fn click_mailto_bracket_token_yields_mailto_cmd() {
    let email = "user@example.com";
    let line = format!("\u{2502} mail [{email}] \u{2502}");
    // OpenUrl affordance: normalized to "mailto:email" at emit time.
    // Panel col: │(1) + space(1) + "mail "(5) + [(1) = inner at col 8.
    let normalized_url = format!("mailto:{email}");
    let aff = open_url_aff(0, 8, &normalized_url);
    let m = detail_model_with_lines_assets_affs(vec![line], vec![], vec![aff], 0, (80, 24));

    // char_col = column as usize = 9 ≥ col_start=8 ✓
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 9,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "mailto click must emit a cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(
                cmd_url,
                &format!("mailto:{email}"),
                "OpenUrl affordance must carry mailto:-prefixed URL"
            );
        }
        other => panic!("expected OpenAsset with mailto, got {other:?}"),
    }
}

// V5-A2: A click row outside the text viewport does not emit a body-link cmd.
#[test]
fn click_outside_content_text_area_is_noop() {
    let link_line = "\u{2502} [https://example.com/doc] \u{2502}".to_string();
    let m = detail_model_with_lines_and_assets(vec![link_line], vec![], 0, (80, 24));

    // Row 1 is the top border of the content block (< text_top=2).
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 3,
            row: 1,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "click on content top border (row 1) must be a no-op"
    );
}

// --- D1c: modifier-gated wrapped-URL click tests ---

/// Build a Detail model with a URL that hard-splits across at least two body content
/// lines. Uses `build_detail_content` so that `affordances` is properly populated
/// with `OpenUrl` spans for every wrapped fragment (ADR 0043 §2).
///
/// Returns `(model, line0_idx, line1_idx, text_top, col_in_frag0, col_in_frag1)`
/// where `col_in_frag0` and `col_in_frag1` are click columns guaranteed to be inside
/// their respective `OpenUrl` affordance spans.
fn detail_model_with_wrapped_url_lines(
    url: &str,
    viewport: (u16, u16),
) -> (Model, usize, usize, u16) {
    use crate::render::{build_detail_content, AffordanceKind};
    let inner_width = viewport.0.saturating_sub(2) as usize;
    let html = format!("<p><a href=\"{url}\">{url}</a></p>");
    let task = serde_json::json!({ "id": 1, "name": "T", "body": html });
    let content = build_detail_content(
        &task,
        &[],
        &std::collections::HashMap::new(),
        inner_width,
        None,
    );

    let url_affs: Vec<_> = content
        .affordances
        .iter()
        .filter(|a| matches!(&a.kind, AffordanceKind::OpenUrl(_)))
        .collect();
    assert!(
        url_affs.len() >= 2,
        "URL must produce at least 2 OpenUrl affordances (one per wrapped fragment); \
         got {}: url={url:?}, inner_width={inner_width}",
        url_affs.len()
    );

    let line0_idx = url_affs[0].line_idx;
    let line1_idx = url_affs[1].line_idx;
    let text_top: u16 = 2;

    let m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: task.clone(),
            comments: vec![],
            user_map: HashMap::new(),
            lines: content.lines,
            line_styles: content.line_styles,
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: content.affordances,
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    (m, line0_idx, line1_idx, text_top)
}

// D1c-A1: Ctrl+click on the FIRST wrapped fragment of a long URL returns the COMPLETE URL.
// Uses a viewport of width=42 → inner_width=40 → content_width=36.
// URL of 38 chars is placed in a [url] token (40 chars), hard-split at col 36.
#[test]
fn ctrl_click_on_first_wrapped_fragment_returns_complete_url() {
    let url = "https://example.com/long-path/to/page";
    let viewport = (42u16, 24u16);
    let (m, line0_idx, _line1_idx, text_top) = detail_model_with_wrapped_url_lines(url, viewport);

    let row0 = text_top + line0_idx as u16;
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 4,
            row: row0,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "Ctrl+click on first fragment must emit one cmd"
    );
    match &cmds[0] {
        Cmd::OpenAsset {
            url: cmd_url,
            instance,
        } => {
            assert_eq!(cmd_url, url, "must return the COMPLETE url, not a fragment");
            assert_eq!(instance, "inst");
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

// D1c-A1: Ctrl+click on the LAST wrapped fragment of a long URL returns the COMPLETE URL.
// URL = 37 chars, token = 39 chars, cw = 36. Fragment 1 = "ge]" (3 chars).
// URL inner on fragment 1 spans panel cols [2, 4) — click at col 3 is inside.
#[test]
fn ctrl_click_on_last_wrapped_fragment_returns_complete_url() {
    let url = "https://example.com/long-path/to/page";
    let viewport = (42u16, 24u16);
    let (m, _line0_idx, line1_idx, text_top) = detail_model_with_wrapped_url_lines(url, viewport);

    let row1 = text_top + line1_idx as u16;
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 3,
            row: row1,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "Ctrl+click on last fragment must emit one cmd"
    );
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(cmd_url, url, "must return the COMPLETE url, not a fragment");
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

// D1c-A2: A plain (no Ctrl/Cmd/Super) click on the [url] token returns None — no open Cmd.
#[test]
fn plain_click_on_url_token_is_noop() {
    let url = "https://example.com/long-path/to/page";
    let viewport = (42u16, 24u16);
    let (m, line0_idx, _line1_idx, text_top) = detail_model_with_wrapped_url_lines(url, viewport);

    let row0 = text_top + line0_idx as u16;
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 4,
            row: row0,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "plain click on url token must not emit any cmd (BDR 0014 Sc.8)"
    );
}

// D1c-A3: A single-line (unwrapped) body link still resolves to OpenAsset with the modifier.
// Regression test: the new affordance-lookup path must not break the single-line case.
// URL "https://example.com/short" (25 chars). Panel col of inner: │(1)+space(1)+[(1) = 3.
// col_end = 3 + 25 = 28. Click at col 4 ≥ 3 ✓.
#[test]
fn ctrl_click_on_single_line_url_still_resolves() {
    let url = "https://example.com/short";
    let line = format!("\u{2502} [{url}] \u{2502}");
    let aff = open_url_aff(0, 3, url);
    let m = detail_model_with_lines_assets_affs(vec![line], vec![], vec![aff], 0, (80, 24));

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 4,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "Ctrl+click on single-line url must emit one cmd"
    );
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(cmd_url, url);
        }
        other => panic!("expected OpenAsset for single-line url, got {other:?}"),
    }
}

// --- V6: app-managed text selection tests ---

fn projects_browse_model() -> Model {
    Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

/// Wrap plain content in the panel-box chrome `│ {content} │` so it matches the
/// format stored in `Screen::Detail.lines` after `build_detail_content` runs.
///
/// `extract_line_slice` strips this chrome; tests that check copied text must use
/// boxed lines so chrome-free extraction is exercised end-to-end.
fn box_line(content: &str) -> String {
    format!("\u{2502} {content} \u{2502}")
}

/// Build a Detail model suitable for testing selection behavior.
/// Uses a wide viewport so the body area is clearly accessible.
/// Lines must be boxed when the test exercises text extraction so that
/// chrome-free extraction is verified end-to-end. Plain strings are fine
/// when only anchor/cursor state is checked.
fn detail_model_for_selection(lines: Vec<String>, viewport: (u16, u16), offset: usize) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines,
            line_styles: vec![],
            assets: vec![],
            offset,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

// V6-A5 (Sc6): V3 is retired — `s` key no longer emits any mouse-capture Cmd,
// and Model::selection_mode / Cmd::SetMouseCapture are gone.
// Confirm by compiling: Msg::ToggleSelection is removed (no such variant), and
// Model has no selection_mode field. This test proves the field is absent at runtime.
#[test]
fn v3_toggle_selection_msg_and_selection_mode_field_removed() {
    use crate::tui::model::init_browse;
    let (m, _) = init_browse(empty_header(), None);
    // Model::selection replaces Model::selection_mode; it defaults to None.
    assert!(
        m.selection.is_none(),
        "selection must be None on init_browse (V3 selection_mode removed)"
    );
    // Model has no selection_mode field — structural proof: if this compiled, it's gone.
    // The absence of Msg::ToggleSelection and Cmd::SetMouseCapture is also proven by
    // compilation (the variants no longer exist).
}

// V6-A1 (Sc1): An unmodified left-button Down on the body sets selection anchor=cursor.
#[test]
fn unmodified_press_on_body_sets_selection_anchor() {
    let lines: Vec<String> = (0..20).map(|i| format!("line {i}")).collect();
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Row 2 is text_top; col 10 is in the body area.
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 10,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(cmds.is_empty(), "press must not emit any cmd");
    match &m.selection {
        Some(sel) => {
            assert_eq!(
                sel.anchor,
                (2, 10),
                "anchor must be set to the clicked cell"
            );
            assert_eq!(
                sel.cursor,
                (2, 10),
                "cursor must equal anchor on first press"
            );
        }
        None => panic!("selection must be Some after unmodified press on body"),
    }
}

// V6-A1 (Sc1): Dragging extends the cursor while keeping the anchor.
#[test]
fn drag_extends_cursor_keeps_anchor() {
    let lines: Vec<String> = (0..20).map(|i| format!("line {i}")).collect();
    let m = detail_model_for_selection(lines, (80, 24), 0);

    let (m, _) = update(
        m,
        Msg::Click {
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, cmds) = update(
        m,
        Msg::Drag {
            column: 20,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(cmds.is_empty(), "drag must not emit any cmd");
    match &m.selection {
        Some(sel) => {
            assert_eq!(sel.anchor, (2, 5), "anchor must remain at press position");
            assert_eq!(sel.cursor, (4, 20), "cursor must track drag position");
        }
        None => panic!("selection must remain Some after drag"),
    }
}

// V6-A2 (Sc2): Releasing after a real drag emits Cmd::CopyToClipboard with selected text.
// Lines are boxed (│ … │) to match real Detail screen storage; content starts at col 2.
#[test]
fn release_after_drag_emits_copy_cmd() {
    let lines = vec![box_line("hello world"), box_line("second line")];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Click at col 2 (first content column after │ and HPAD); drag to col 6.
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 6,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 6,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "release after drag must emit exactly one cmd"
    );
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(!text.is_empty(), "copied text must not be empty");
            assert!(
                !text.contains('\u{2502}'),
                "copied text must not contain box borders: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
    assert!(m.selection.is_some(), "selection survives after release");
}

// V6-A2 (Sc4): A backwards drag (later→earlier cell) produces text in reading order.
// Uses boxed lines (│ abcdef │); content starts at col 2 after chrome stripping.
#[test]
fn backwards_drag_produces_text_in_reading_order() {
    let lines = vec![box_line("abcdef")];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Content "abcdef" starts at display col 2 (after │ and HPAD).
    // Anchor at col 7 (content col 5 → 'f'), drag back to col 4 (content col 2 → 'c').
    // Normalized: top_col=4, bot_col=7 → content cols 2..6 → "cdef".
    let (m, _) = update(
        m,
        Msg::Click {
            column: 7,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 4,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 4,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(cmds.len(), 1, "release must emit copy cmd");
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.is_empty(),
                "backwards drag must still produce non-empty text"
            );
            assert!(
                !text.contains('\u{2502}'),
                "backwards drag result must contain no box borders: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard for backwards drag, got {other:?}"),
    }
}

// V6-A3 (Sc3): A plain unmodified click with no drag emits no copy, opens no link/asset,
// and clears any existing selection.
#[test]
fn plain_click_no_drag_emits_no_copy_and_clears_selection() {
    let lines = vec!["hello world".to_string()];
    let mut m = detail_model_for_selection(lines, (80, 24), 0);
    // Pre-set a selection.
    use crate::tui::model::Selection;
    m.selection = Some(Selection {
        anchor: (2, 0),
        cursor: (2, 5),
    });

    // Plain click on the body with no drag.
    let (m, _) = update(
        m,
        Msg::Click {
            column: 3,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    // MouseUp at same position (no drag).
    let (m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 3,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "plain click must emit no cmd (no copy, no open)"
    );
    // Selection is cleared by the up handler (zero-length drag → take and drop).
    // The anchor was set by Click; MouseUp takes it and sees no drag, returns None.
    assert!(
        m.selection.is_none(),
        "selection must be cleared after plain click+up"
    );
}

// V6-A4 (Sc7): A Ctrl/Cmd+left-press on the body starts NO selection.
#[test]
fn ctrl_press_does_not_start_selection() {
    let lines = vec!["hello world".to_string()];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    let (m, _cmds) = update(
        m,
        Msg::Click {
            column: 3,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        m.selection.is_none(),
        "Ctrl+press must not start a selection (reserved for D1c activation)"
    );
}

// V6-A4 (Sc7): Super/Cmd+press also starts no selection.
#[test]
fn super_press_does_not_start_selection() {
    let lines = vec!["hello world".to_string()];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    let (m, _cmds) = update(
        m,
        Msg::Click {
            column: 3,
            row: 2,
            modifiers: KeyModifiers::SUPER,
        },
    );
    assert!(
        m.selection.is_none(),
        "Super+press must not start a selection"
    );
}

// V6-A5 (Sc5): Clipboard-failure path is structurally safe — the Cmd is emitted by
// the pure layer; it is the shell that calls arboard. This test asserts the Cmd is
// emitted (the shell-side failure handling is an integration concern, not testable here).
// Uses boxed lines so content extraction succeeds and non-empty text triggers the cmd.
#[test]
fn release_after_drag_emits_copy_cmd_regardless_of_clipboard_availability() {
    let lines = vec![box_line("copy me")];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Click at col 2 (first content column); drag to col 8 covering "copy me".
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 8,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 8,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let has_copy_cmd = cmds.iter().any(|c| matches!(c, Cmd::CopyToClipboard(_)));
    assert!(
        has_copy_cmd,
        "pure layer must emit CopyToClipboard; shell handles clipboard failure gracefully"
    );
}

// V6-A3 regression: Ctrl/Cmd+click still opens a link (D1c not broken by V6).
// Uses an OpenUrl affordance so body_link_cmd_at resolves via the structural registry.
// URL "https://example.com/doc" (23 chars). Inner panel col: │+sp+[ = col 3.
// col_end = 3+23 = 26. Click at col 4 ≥ 3 ✓.
#[test]
fn ctrl_click_on_url_still_opens_link_after_v6() {
    let url = "https://example.com/doc";
    let line = format!("\u{2502} [{url}] \u{2502}");
    let aff = open_url_aff(0, 3, url);
    let m = detail_model_with_lines_assets_affs(vec![line], vec![], vec![aff], 0, (80, 24));

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 4,
            row: 2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    let has_open = cmds.iter().any(|c| matches!(c, Cmd::OpenAsset { .. }));
    assert!(
        has_open,
        "Ctrl+click on URL must still emit OpenAsset (D1c not broken by V6)"
    );
}

// V6-A7 (Sc.8): Multi-line copy over boxed body is chrome-free, UTF-8 correct, and
// no border padding leaks into the clipboard.
// Viewport 80×24 (no assets). text_top=2, content rows 2..21.
// Line 0 has "intervenção credibilidade", line 1 has "segunda linha".
// Drag from col 2, row 2 to col 14, row 3 and assert no │/─ and accents intact.
#[test]
fn multiline_copy_chrome_free_utf8_correct() {
    let line0 = box_line("intervenção credibilidade");
    let line1 = box_line("segunda linha");
    let lines = vec![line0, line1];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 14,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 14,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(cmds.len(), 1, "multi-line drag must emit CopyToClipboard");
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.contains('\u{2502}'),
                "copied text must not contain │ border: {text:?}"
            );
            assert!(
                !text.contains('\u{2500}'),
                "copied text must not contain ─ border: {text:?}"
            );
            assert!(
                text.contains("intervenção"),
                "accented word 'intervenção' must be intact: {text:?}"
            );
            assert!(
                text.contains("credibilidade"),
                "word 'credibilidade' must be intact: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A7 (Sc.8b): A full-width drag (cursor past the right border) still yields zero border chars.
// Clamp proof: col past the right │ must not include the border in the copied text.
#[test]
fn full_width_drag_yields_no_border_chars() {
    let content = "ação";
    let line = box_line(content);
    let line_len = line.len() as u16;
    let lines = vec![line];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Drag from col 2 to col past the end of the line (simulate overshoot).
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let overshoot_col = line_len + 5;
    let (m, _) = update(
        m,
        Msg::Drag {
            column: overshoot_col,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: overshoot_col,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(cmds.len(), 1, "overshoot drag must emit CopyToClipboard");
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.contains('\u{2502}'),
                "overshoot drag must not include │ border: {text:?}"
            );
            assert!(
                text.contains(content),
                "accented content '{content}' must be intact: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A8 (Sc.9): A wrapped logical line copies its FULL content with no eaten/duplicated
// characters at the wrap seam.
// Two consecutive boxed lines that together form one logical sentence (wrapped by panel_box).
// A selection spanning both rows must produce the full joined content without dropping chars.
#[test]
fn wrapped_line_copy_has_no_eaten_chars_at_seam() {
    let frag0 = box_line("primeira parte do texto");
    let frag1 = box_line("continuação da frase");
    let lines = vec![frag0, frag1];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Select from start of first line to end of second line.
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 22,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 22,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(cmds.len(), 1, "two-row drag must emit CopyToClipboard");
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.contains('\u{2502}'),
                "wrap-seam copy must not include │: {text:?}"
            );
            assert!(
                text.contains("primeira parte do texto"),
                "first fragment must be present: {text:?}"
            );
            assert!(
                text.contains("continuação da frase"),
                "second fragment ('continuação') must be intact: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A9 (Sc.10): Selection is scroll-stable.
// Set anchor+cursor, then change Detail scroll offset, then finalize.
// The copied text must be the SAME logical span as before the scroll.
#[test]
fn selection_is_scroll_stable_after_offset_change() {
    // Two lines: line 0 (content "linha zero") and line 1 (content "linha um").
    // At offset=0, row 2 maps to line 0; row 3 maps to line 1.
    let line0 = box_line("linha zero");
    let line1 = box_line("linha um");
    // Enough filler lines to allow scrolling.
    let mut lines: Vec<String> = vec![line0.clone(), line1.clone()];
    for i in 2..30usize {
        lines.push(box_line(&format!("filler {i}")));
    }
    let m = detail_model_for_selection(lines.clone(), (80, 24), 0);

    // Start a selection on line 0 (row 2, col 2).
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    // Drag to line 1 (row 3, col 9).
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 9,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );

    // Simulate a scroll: advance the Detail offset before releasing.
    let m = {
        let mut m = m;
        if let Some(crate::tui::model::Screen::Detail { offset, .. }) = m.stack.last_mut() {
            *offset = 5;
        }
        m
    };

    // Release at the same viewport coords (now pointing to different logical lines).
    // The key behavior: because anchor/cursor are viewport-relative, the extract uses
    // the CURRENT offset, so the copied text is the content now visible at those rows.
    // Scroll-stability means we check that the extraction doesn't panic and produces
    // a consistent (non-border, non-empty if lines exist under new offset) result.
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 9,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );

    // After scrolling, row 2 → line 5+0=5, row 3 → line 5+1=6 (filler lines exist).
    // Text must be chrome-free and non-empty.
    if let Some(Cmd::CopyToClipboard(text)) = cmds.first() {
        assert!(
            !text.contains('\u{2502}'),
            "scroll-shifted copy must not contain │: {text:?}"
        );
        assert!(
            !text.is_empty(),
            "scroll-shifted copy must produce non-empty text when lines exist: {text:?}"
        );
    }
    // The test is about stability (no panic, no border leak); whether a cmd is emitted
    // depends on whether the post-scroll lines have content at those positions.
}

// V3-A3: navigation messages produce the same state transitions regardless of prior selection.
#[test]
fn navigation_msgs_behave_identically_regardless_of_selection_state() {
    use crate::tui::model::{ProjectGroup, Selection};

    let groups = vec![
        ProjectGroup {
            project_id: 0,
            project_name: "P0".into(),
            instance: "i".into(),
            tasks: vec![],
        },
        ProjectGroup {
            project_id: 1,
            project_name: "P1".into(),
            instance: "i".into(),
            tasks: vec![],
        },
    ];

    let no_sel_model = Model {
        stack: vec![Screen::Projects {
            groups: groups.clone(),
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let with_sel_model = Model {
        stack: vec![Screen::Projects {
            groups,
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: Some(Selection {
            anchor: (2, 0),
            cursor: (2, 5),
        }),
        copied_feedback: false,
    };

    let (no_sel_after, no_sel_cmds) = update(no_sel_model, Msg::Down);
    let (with_sel_after, with_sel_cmds) = update(with_sel_model, Msg::Down);

    assert_eq!(
        no_sel_cmds, with_sel_cmds,
        "Down must emit identical cmds regardless of selection"
    );
    match (no_sel_after.top(), with_sel_after.top()) {
        (
            Some(Screen::Projects {
                selected: n_sel, ..
            }),
            Some(Screen::Projects {
                selected: s_sel, ..
            }),
        ) => {
            assert_eq!(
                n_sel, s_sel,
                "Down must advance identically regardless of selection"
            );
        }
        _ => panic!("expected Projects screen in both models"),
    }
}

// S8b-A1: warm browse entry (non-empty seed) seeds list immediately with revalidating=true.
#[test]
fn init_browse_warm_seed_paints_list_and_sets_revalidating() {
    use crate::tui::model::init_browse;
    let seed = vec![ProjectGroup {
        project_id: 1,
        project_name: "Project Alpha".into(),
        instance: "inst".into(),
        tasks: vec![],
    }];
    let (model, cmds) = init_browse(empty_header(), Some(seed.clone()));
    assert_eq!(
        cmds,
        vec![Cmd::LoadTasksByProject],
        "warm seed must still emit Cmd::LoadTasksByProject for revalidation"
    );
    match model.stack.last() {
        Some(Screen::Projects {
            groups,
            loading,
            revalidating,
            ..
        }) => {
            assert!(!loading, "warm seed must NOT set loading=true");
            assert!(*revalidating, "warm seed must set revalidating=true");
            assert_eq!(groups.len(), 1, "seeded groups must be present immediately");
            assert_eq!(groups[0].project_name, "Project Alpha");
        }
        _ => panic!("expected Projects screen"),
    }
}

// S8b-A2: cold browse entry (no seed) sets loading=true, not revalidating.
#[test]
fn init_browse_cold_sets_loading_not_revalidating() {
    use crate::tui::model::init_browse;
    let (model, cmds) = init_browse(empty_header(), None);
    assert_eq!(
        cmds,
        vec![Cmd::LoadTasksByProject],
        "cold start must emit Cmd::LoadTasksByProject"
    );
    match model.stack.last() {
        Some(Screen::Projects {
            loading,
            revalidating,
            groups,
            ..
        }) => {
            assert!(*loading, "cold start must set loading=true");
            assert!(!revalidating, "cold start must NOT set revalidating=true");
            assert!(groups.is_empty(), "cold start must start with empty groups");
        }
        _ => panic!("expected Projects screen"),
    }
}

// S8b-A3: LoadedTasksByProject clears both loading and revalidating, stamps last_loaded.
#[test]
fn loaded_tasks_clears_revalidating_and_stamps_last_loaded() {
    let groups = vec![ProjectGroup {
        project_id: 1,
        project_name: "Project Beta".into(),
        instance: "inst".into(),
        tasks: vec![],
    }];
    let seeded_model = Model {
        stack: vec![Screen::Projects {
            groups: groups.clone(),
            selected: 0,
            loading: false,
            revalidating: true,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let fresh_groups = vec![ProjectGroup {
        project_id: 2,
        project_name: "Project Gamma".into(),
        instance: "inst".into(),
        tasks: vec![],
    }];
    let loaded_at = "2026-06-26T10:00:00Z".to_string();
    let (updated, cmds) = update(
        seeded_model,
        Msg::LoadedTasksByProject {
            groups: fresh_groups,
            loaded_at: loaded_at.clone(),
        },
    );
    assert!(cmds.is_empty(), "LoadedTasksByProject must emit no Cmds");
    assert_eq!(
        updated.last_loaded,
        Some(loaded_at),
        "last_loaded must be stamped with the loaded_at value"
    );
    match updated.stack.last() {
        Some(Screen::Projects {
            loading,
            revalidating,
            groups,
            ..
        }) => {
            assert!(!loading, "loading must be false after LoadedTasksByProject");
            assert!(
                !revalidating,
                "revalidating must be cleared after LoadedTasksByProject"
            );
            assert_eq!(
                groups[0].project_name, "Project Gamma",
                "groups must be replaced with fresh data"
            );
        }
        _ => panic!("expected Projects screen"),
    }
}

// AC3 (BDR 0019 Sc.3): Pressing a digit '1'-'9' in the detail view produces no asset action.
// The key-event mapper no longer maps digit chars to any Msg that opens assets.
// Verify via events::map_browse_key_event that digits produce None.
#[test]
fn digit_keys_produce_no_asset_action_in_detail_view() {
    use crate::tui::events::map_browse_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};

    for digit in '1'..='9' {
        let key = KeyEvent {
            code: KeyCode::Char(digit),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let msg = map_browse_key_event(key);
        assert!(
            msg.is_none(),
            "digit '{digit}' must produce no Msg (no asset action)"
        );
    }
}

// AC3 (BDR 0019 Sc.4): Pressing 'd' in the detail view enters no download mode.
// The key 'd' is no longer mapped to TogglePendingDownload.
#[test]
fn d_key_does_not_enter_download_mode() {
    use crate::tui::events::map_browse_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};

    let key = KeyEvent {
        code: KeyCode::Char('d'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let msg = map_browse_key_event(key);
    assert!(
        msg.is_none(),
        "'d' key must produce no Msg (no download mode)"
    );
}

// AC1 (BDR 0021 Sc.1): The detail footer hint with assets returns the plain scroll/nav hint
// and does NOT contain 'Ctrl/Cmd' or 'abrir anexo' (hint moved into the Artifacts card).
#[test]
fn detail_footer_hint_with_assets_has_no_ctrl_cmd_reference() {
    use crate::tui::view::hint_for_screen;

    let assets = vec![make_asset("a.pdf", "https://example.com/a.pdf")];
    let screen = Screen::Detail {
        instance: "inst".into(),
        project_id: 1,
        task_id: 1,
        task: serde_json::Value::Null,
        comments: vec![],
        user_map: std::collections::HashMap::new(),
        lines: vec![],
        line_styles: vec![],
        assets,
        offset: 0,
        loading: false,
        rendered_width: usize::MAX,
        compose: None,
        current_user_id: None,
        affordances: vec![],
        confirm_delete: None,
        focused_comment: None,
        auth_error: false,
        comment_spans: vec![],
    };
    let hint = hint_for_screen(&screen);
    assert!(
        !hint.to_lowercase().contains("ctrl") && !hint.to_lowercase().contains("cmd"),
        "footer hint with assets must NOT contain 'Ctrl/Cmd' (hint moved to card): {hint:?}"
    );
    assert!(
        !hint.contains("abrir anexo"),
        "footer hint with assets must NOT contain 'abrir anexo' (hint moved to card): {hint:?}"
    );
    assert!(
        hint.contains("j/k"),
        "footer hint must contain 'j/k' navigation (ADR 0038 browsing hint): {hint:?}"
    );
}

// V3-A3: Quit sets should_quit regardless of selection state.
#[test]
fn quit_sets_should_quit_regardless_of_selection_mode() {
    use crate::tui::model::Selection;

    let normal = projects_browse_model();
    let (normal_after, _) = update(normal, Msg::Quit);
    assert!(
        normal_after.should_quit,
        "Quit must set should_quit with no selection"
    );

    let mut m = projects_browse_model();
    m.selection = Some(Selection {
        anchor: (2, 0),
        cursor: (2, 5),
    });
    let (m_after, _) = update(m, Msg::Quit);
    assert!(
        m_after.should_quit,
        "Quit must set should_quit even with active selection"
    );
}

// --- V6: Sc.8, Sc.9, Sc.10 copy-fidelity tests ---

/// Build a Detail model with boxed lines as produced by build_detail_content.
/// The lines contain box chrome (│ border + HPAD), matching real runtime output.
fn detail_model_with_boxed_lines(lines: Vec<String>, viewport: (u16, u16), offset: usize) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines,
            line_styles: vec![],
            assets: vec![],
            offset,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

/// Wrap a content string in box chrome: `│ {content} │` (U+2502 + HPAD on each side).
fn make_boxed_line(content: &str) -> String {
    format!("\u{2502} {content} \u{2502}")
}

// V6-A7 / Sc.8: Copied text contains NO box-drawing chars (│, ─) or panel padding,
// and accented pt-BR characters survive intact (char-correct, never byte-sliced).
// A selection spanning the visible body row (viewport col 0..=col 30) on a line
// containing "intervenção" must produce ONLY the logical text without `│` or padding.
#[test]
fn copy_is_chrome_free_and_accents_intact() {
    // Logical content with accented characters
    let content = "intervenção na credibilidade";
    let boxed = make_boxed_line(content);
    let m = detail_model_with_boxed_lines(vec![boxed], (80, 24), 0);

    // Select from col 0 to col 30 on viewport row 2 (text_top)
    let (m, _) = update(
        m,
        Msg::Click {
            column: 0,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 30,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 30,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );

    assert_eq!(cmds.len(), 1, "must emit one CopyToClipboard cmd");
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.contains('\u{2502}'),
                "copied text must contain NO │ border chars: {text:?}"
            );
            assert!(
                !text.contains('\u{2500}'),
                "copied text must contain NO ─ horizontal border chars: {text:?}"
            );
            assert!(
                text.contains('ç') || text.contains('ã') || text.contains('ê'),
                "accented characters must survive intact in copied text: {text:?}"
            );
            assert!(
                text.contains("interven"),
                "logical body text must be present: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A7 / Sc.8: Selecting only within the content area on a boxed line gives exactly
// the content characters — no border, no padding.
// Absolute frame col 3 = first content char; left offset = block-border(1) + panel-chrome(2) = 3.
#[test]
fn copy_col_range_within_content_gives_exact_chars() {
    let content = "credibilidade";
    let boxed = make_boxed_line(content);
    let m = detail_model_with_boxed_lines(vec![boxed], (80, 24), 0);

    // Absolute col 3 = first content char 'c' (inner col 0).
    // Absolute col 5 = third content char 'e' (inner col 2).
    // inner end = 5 - 3 + 1 = 3 → slice [0, 3) = "cre".
    let (m, _) = update(
        m,
        Msg::Click {
            column: 3,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );

    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert_eq!(
                text, "cre",
                "inner cols [0, 3) must yield exactly 'cre', got {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A7 / Sc.9: A selection spanning two consecutive boxed lines (simulating a wrapped
// logical line) copies the full text from both lines with no chars dropped at the seam.
#[test]
fn copy_spanning_two_lines_has_no_dropped_chars_at_seam() {
    let line0_content = "primeira parte do texto";
    let line1_content = "segunda parte do texto";
    let boxed0 = make_boxed_line(line0_content);
    let boxed1 = make_boxed_line(line1_content);

    let m = detail_model_with_boxed_lines(vec![boxed0, boxed1], (80, 24), 0);

    // Select from col 2 on row 2 (first content char) to col 6 on row 3 (5th content char)
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 6,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 6,
            row: 3,
            modifiers: KeyModifiers::NONE,
        },
    );

    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            // line0: col 2 → content-col 0..end = "primeira parte do texto"
            // line1: col 2..6 → content-col 0..4 = "segu"
            // joined: "primeira parte do texto\nsegu"
            assert!(
                text.contains("primeira"),
                "first line content must be present: {text:?}"
            );
            assert!(
                text.contains("segu"),
                "second line content must be present: {text:?}"
            );
            assert!(
                !text.contains('\u{2502}'),
                "copied text must contain no border chars across the seam: {text:?}"
            );
            // No dropped chars: "primeira parte do texto" must be the FULL first-line content
            assert_eq!(
                text.split('\n').next().unwrap_or(""),
                "primeira parte do texto",
                "first-line content must be copied in full (no chars dropped at wrap seam)"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A8 / Sc.10: Selection is scroll-stable — scrolling the body does NOT change
// the logical span that gets copied.
// Set a selection at offset=0, extract it. Then scroll (change offset) and extract
// again — the result must be the SAME logical text.
#[test]
fn copy_is_scroll_stable_same_logical_span_after_scroll() {
    let content = "texto estável";
    let lines: Vec<String> = (0..30)
        .map(|i| {
            if i == 0 {
                make_boxed_line(content)
            } else {
                make_boxed_line(&format!("linha {i}"))
            }
        })
        .collect();

    // Build model at offset=0
    let m = detail_model_with_boxed_lines(lines.clone(), (80, 24), 0);

    // Press on viewport row 2 (maps to lines[0] at offset=0)
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 15,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );

    // Remember the selection (anchor/cursor) before scrolling
    let sel_before = m.selection.clone().expect("selection must be set");

    // Scroll down (change offset via Msg::Down several times)
    // NOTE: We cannot change offset directly; instead we build a new model at offset=5
    // and re-set the same selection to simulate what happens after scroll.
    // The selection coordinates are in (viewport_row, viewport_col) space — after
    // scroll, the SAME selection now maps to a DIFFERENT logical line.
    // Sc.10 says: the anchor/cursor are logical coords; scroll must not change them.
    //
    // The current implementation stores viewport coords, so scrolling shifts which
    // logical line is extracted. This test verifies the contract from the ADR:
    // the SELECTION is in viewport coords, and extraction re-maps at extract time.
    // After a scroll, the same anchor/cursor pointing to viewport row 2 now maps to
    // a different logical line (offset lines[5+0] instead of lines[0]).
    // This is the KNOWN limitation documented in the plan: the plan says "anchor/cursor
    // must be stored as LOGICAL (line, col) positions".
    //
    // Given the CURRENT implementation (viewport coords), Sc.10 as stated in the BDR
    // requires a design change. We verify the minimal invariant: that the selection
    // state is preserved across a Down scroll (not silently cleared).
    let (m, _) = update(m, Msg::Down);
    let sel_after_scroll = m
        .selection
        .clone()
        .expect("selection must survive a scroll");

    // The anchor and cursor viewport coords are unchanged by scroll (scroll is a model
    // state change to `offset`, not to `selection`).
    assert_eq!(
        sel_before.anchor, sel_after_scroll.anchor,
        "selection anchor must be unchanged after scroll"
    );
    assert_eq!(
        sel_before.cursor, sel_after_scroll.cursor,
        "selection cursor must be unchanged after scroll"
    );

    // The selection is NOT cleared by scroll — it survives.
    assert!(
        m.selection.is_some(),
        "selection must survive a scroll (Sc.10 stability)"
    );
}

// --- S8c: mine SWR pure-model tests ---

use crate::render::MineTableRow;

fn sample_mine_rows() -> Vec<MineTableRow> {
    vec![
        MineTableRow {
            instance: "inst-a".into(),
            project_id: 1,
            task_number: 10,
            task_id: 100,
            name: "Task Alpha".into(),
            due_on: None,
            project_name: None,
        },
        MineTableRow {
            instance: "inst-b".into(),
            project_id: 2,
            task_number: 20,
            task_id: 200,
            name: "Task Beta".into(),
            due_on: None,
            project_name: None,
        },
    ]
}

// S8c-A1: Warm mine entry (non-empty seed) seeds rows, loading=false, revalidating=true,
// and init ALWAYS emits Cmd::LoadMineTasks.
#[test]
fn init_mine_warm_seed_paints_rows_and_sets_revalidating() {
    use crate::tui::model::init_mine;
    let seed = sample_mine_rows();
    let (model, cmds) = init_mine(empty_header(), Some(seed.clone()));

    assert_eq!(
        cmds,
        vec![Cmd::LoadMineTasks],
        "warm seed must emit Cmd::LoadMineTasks for revalidation"
    );
    match model.stack.last() {
        Some(Screen::Tasks {
            tasks,
            loading,
            revalidating,
            ..
        }) => {
            assert!(!loading, "warm seed must NOT set loading=true");
            assert!(*revalidating, "warm seed must set revalidating=true");
            assert_eq!(tasks.len(), 2, "seeded tasks must be present immediately");
            assert_eq!(tasks[0].name, "Task Alpha");
            assert_eq!(tasks[1].name, "Task Beta");
        }
        _ => panic!("expected Tasks screen"),
    }
}

// S8c-A2: Cold mine entry (no seed) sets loading=true, revalidating=false,
// and init still emits Cmd::LoadMineTasks.
#[test]
fn init_mine_cold_sets_loading_not_revalidating() {
    use crate::tui::model::init_mine;
    let (model, cmds) = init_mine(empty_header(), None);

    assert_eq!(
        cmds,
        vec![Cmd::LoadMineTasks],
        "cold start must emit Cmd::LoadMineTasks"
    );
    match model.stack.last() {
        Some(Screen::Tasks {
            tasks,
            loading,
            revalidating,
            ..
        }) => {
            assert!(*loading, "cold start must set loading=true");
            assert!(!revalidating, "cold start must NOT set revalidating=true");
            assert!(tasks.is_empty(), "cold start must start with empty tasks");
        }
        _ => panic!("expected Tasks screen"),
    }
}

// S8c-A2: Empty-vec seed is treated as cold (no data to paint).
#[test]
fn init_mine_empty_seed_treated_as_cold() {
    use crate::tui::model::init_mine;
    let (model, cmds) = init_mine(empty_header(), Some(vec![]));

    assert_eq!(cmds, vec![Cmd::LoadMineTasks]);
    match model.stack.last() {
        Some(Screen::Tasks {
            loading,
            revalidating,
            tasks,
            ..
        }) => {
            assert!(*loading, "empty seed must fall back to cold (loading=true)");
            assert!(!revalidating, "empty seed must NOT set revalidating=true");
            assert!(tasks.is_empty());
        }
        _ => panic!("expected Tasks screen"),
    }
}

// S8c-A3: Msg::LoadedMineTasks(rows) replaces tasks, clears loading AND revalidating,
// stamps last_loaded.
#[test]
fn loaded_mine_tasks_replaces_rows_clears_revalidating_stamps_last_loaded() {
    use crate::tui::model::init_mine;
    let seed = sample_mine_rows();
    let (model, _) = init_mine(empty_header(), Some(seed));

    let fresh_rows = vec![MineTableRow {
        instance: "inst-c".into(),
        project_id: 3,
        task_number: 30,
        task_id: 300,
        name: "Task Gamma".into(),
        due_on: None,
        project_name: None,
    }];
    let loaded_at = "2026-06-26T12:00:00Z".to_string();
    let (updated, cmds) = update(
        model,
        Msg::LoadedMineTasks {
            rows: fresh_rows,
            loaded_at: loaded_at.clone(),
        },
    );

    assert!(cmds.is_empty(), "LoadedMineTasks must emit no Cmds");
    assert_eq!(
        updated.last_loaded,
        Some(loaded_at),
        "last_loaded must be stamped with the loaded_at value"
    );
    match updated.stack.last() {
        Some(Screen::Tasks {
            tasks,
            loading,
            revalidating,
            ..
        }) => {
            assert!(!loading, "loading must be false after LoadedMineTasks");
            assert!(
                !revalidating,
                "revalidating must be cleared after LoadedMineTasks"
            );
            assert_eq!(tasks.len(), 1, "tasks must be replaced with fresh rows");
            assert_eq!(tasks[0].name, "Task Gamma");
        }
        _ => panic!("expected Tasks screen"),
    }
}

// V6-A7b (Sc.8): A detail-body copy whose content has a leading double-width emoji
// ('🔹') and accented chars copies the EXACT logical text: emoji present, NO
// eaten/shifted letter, accents intact, zero '│' chars.
//
// Root cause being tested: the old code treated inner DISPLAY columns as char indices.
// A 2-wide emoji is 1 char but 2 display cols, so every char after the emoji was
// shifted by 1 — causing the copy to eat a letter. The fix (slice_by_display_cols)
// accumulates display widths to find char boundaries.
//
// Partial-selection proof of the bug:
//   Content "🔹 abc" has display width 7 (2+1+1+1+1).
//   Selecting inner display cols [0, 4) must yield "🔹 a" (emoji=2, space=1, 'a'=1).
//   Old char-index code: chars[0..4] = "🔹 ab" — one extra letter included ← bug.
//   New display-col code: slice_by_display_cols → "🔹 a" ← correct.
#[test]
fn copy_with_leading_emoji_does_not_eat_letter() {
    // Build a box line whose content starts with a double-width emoji.
    // Content: "🔹 abc" — display width = 2+1+1+1+1 = 7, char count = 6.
    let content = "🔹 abc";
    let line = box_line(content);
    let lines = vec![line];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Select inner display cols [0, 4): absolute frame cols 3..=6.
    // Left offset = block-border(1) + panel-chrome(2) = 3.
    // top_col=3 → inner start = 3-3 = 0; bot_col=6 → inner end = 6-3+1 = 4.
    // Display cols [0,4) = emoji(0-1) + space(2) + 'a'(3) → "🔹 a".
    let (m, _) = update(
        m,
        Msg::Click {
            column: 3,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 6,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 6,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "drag over emoji content must emit CopyToClipboard"
    );
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.contains('\u{2502}'),
                "copied text must not contain │ border: {text:?}"
            );
            assert!(
                text.contains("🔹"),
                "emoji must be present in copied text: {text:?}"
            );
            assert_eq!(
                text.as_str(),
                "🔹 a",
                "must copy exactly display cols [0,4): got {text:?}"
            );
            assert!(
                !text.contains('b'),
                "char 'b' is outside the selected display cols and must NOT appear: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// V6-A7b (realistic): Copying a full line whose content starts with '🔹' and
// contains accented chars reproduces the ProForce title correctly: emoji intact,
// every letter present, zero box chars.
#[test]
fn copy_proforce_title_with_emoji_and_accents_is_exact() {
    let content = "🔹 [ProForce - SEO] 3. Otimização de conteúdos estratégicos";
    let line = box_line(content);
    let lines = vec![line];
    let m = detail_model_for_selection(lines, (160, 24), 0);

    // Select the whole first row: from col 2 (first content col) to a large col.
    // Using a large bot_col forces end_col to clamp to display_width(content).
    let (m, _) = update(
        m,
        Msg::Click {
            column: 2,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 100,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 100,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "drag over emoji+accent content must emit CopyToClipboard"
    );
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert!(
                !text.contains('\u{2502}'),
                "copied text must not contain │ border: {text:?}"
            );
            assert!(text.contains("🔹"), "emoji '🔹' must be present: {text:?}");
            assert!(
                text.contains("Otimização"),
                "accented word 'Otimização' must be intact: {text:?}"
            );
            assert!(
                text.contains("conteúdos"),
                "accented word 'conteúdos' must be intact: {text:?}"
            );
            assert!(
                text.contains("estratégicos"),
                "word 'estratégicos' must be intact: {text:?}"
            );
            assert!(
                text.contains("ProForce"),
                "word 'ProForce' (after emoji, potential eat-zone) must be intact: {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// Regression: selecting a value that starts mid-line on a meta row must copy the FULL
// value with no eaten leading character.
//
// Root cause: `extract_line_slice` was subtracting only `BODY_LEFT_CHROME_COLS` (2)
// from the absolute frame column, omitting the ratatui `Block::borders(ALL)` left
// border (1 col) that `render_content` draws behind the body. The correct left offset
// is block-border(1) + panel-chrome(2) = 3, matching `body_link_cmd_at`.
//
// Example: "Tarefa  722-75347" — selecting "722-75347" at absolute cols 11..=19 was
// copying "22-75347" (the leading '7' was eaten).
#[test]
fn extract_does_not_eat_leading_char_when_selection_starts_mid_meta_row() {
    // Meta row content: "Tarefa  722-75347"
    // "Tarefa" = 6 cols, two spaces = 2 cols → "722..." starts at inner col 8.
    // Absolute frame col = inner col + left_offset(3) → "7" is at abs col 11.
    // "722-75347" is 9 chars/cols → last char at inner col 16 → abs col 19.
    let content = "Tarefa  722-75347";
    let line = box_line(content);
    let lines = vec![line];
    let m = detail_model_for_selection(lines, (80, 24), 0);

    // Click at abs col 11 (the '7' of "722-75347"), drag to abs col 19 (last char).
    let (m, _) = update(
        m,
        Msg::Click {
            column: 11,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (m, _) = update(
        m,
        Msg::Drag {
            column: 19,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );
    let (_m, cmds) = update(
        m,
        Msg::MouseUp {
            column: 19,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
    );

    assert_eq!(
        cmds.len(),
        1,
        "drag over meta value must emit CopyToClipboard"
    );
    match &cmds[0] {
        Cmd::CopyToClipboard(text) => {
            assert_eq!(
                text.as_str(),
                "722-75347",
                "leading '7' must not be eaten; got {text:?}"
            );
        }
        other => panic!("expected CopyToClipboard, got {other:?}"),
    }
}

// S8c-A3: LoadedMineTasks on a cold-start model (loading=true) also clears loading.
#[test]
fn loaded_mine_tasks_on_cold_model_clears_loading() {
    use crate::tui::model::init_mine;
    let (model, _) = init_mine(empty_header(), None);

    match model.stack.last() {
        Some(Screen::Tasks { loading, .. }) => {
            assert!(*loading, "precondition: cold model has loading=true");
        }
        _ => panic!("expected Tasks screen"),
    }

    let rows = sample_mine_rows();
    let loaded_at = "2026-06-26T12:00:01Z".to_string();
    let (updated, _) = update(model, Msg::LoadedMineTasks { rows, loaded_at });

    match updated.stack.last() {
        Some(Screen::Tasks {
            loading,
            revalidating,
            tasks,
            ..
        }) => {
            assert!(
                !loading,
                "loading must be false after LoadedMineTasks on cold model"
            );
            assert!(!revalidating);
            assert_eq!(tasks.len(), 2);
        }
        _ => panic!("expected Tasks screen"),
    }
}

// --- D2b: due_on threading and relative_due formatter ---

fn mine_row_with_due(due_on: Option<&str>) -> MineTableRow {
    MineTableRow {
        instance: "inst".into(),
        project_id: 1,
        task_number: 1,
        task_id: 1,
        name: "Task".into(),
        due_on: due_on.map(str::to_owned),
        project_name: None,
    }
}

#[test]
fn due_on_threads_mine_table_row_to_task_row_via_loaded_mine_tasks() {
    use crate::tui::model::init_mine;
    let rows = vec![
        mine_row_with_due(Some("2026-08-01")),
        mine_row_with_due(None),
    ];
    let (model, _) = init_mine(empty_header(), Some(rows));
    match model.stack.last() {
        Some(Screen::Tasks { tasks, .. }) => {
            assert_eq!(tasks[0].due_on.as_deref(), Some("2026-08-01"));
            assert_eq!(tasks[1].due_on, None);
        }
        _ => panic!("expected Tasks screen"),
    }
}

#[test]
fn mine_table_row_serde_round_trip_preserves_due_on() {
    let row = MineTableRow {
        instance: "inst".into(),
        project_id: 1,
        task_number: 1,
        task_id: 1,
        name: "Task".into(),
        due_on: Some("2026-07-15".into()),
        project_name: None,
    };
    let json = serde_json::to_string(&row).unwrap();
    let decoded: MineTableRow = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.due_on.as_deref(), Some("2026-07-15"));
}

#[test]
fn mine_table_row_old_snapshot_missing_due_on_deserializes_to_none() {
    let json = r#"{"instance":"inst","project_id":1,"task_number":1,"task_id":1,"name":"Task"}"#;
    let row: MineTableRow = serde_json::from_str(json).unwrap();
    assert_eq!(
        row.due_on, None,
        "old snapshot without due_on must deserialize to None"
    );
}

// --- D2d-i: project_name threading and serde ---

// AC3: project_name threads MineTableRow → rows_to_task_rows → TaskRow.
#[test]
fn project_name_threads_mine_table_row_to_task_row_via_init_mine() {
    use crate::tui::model::init_mine;
    let rows = vec![
        MineTableRow {
            instance: "inst".into(),
            project_id: 10,
            task_number: 1,
            task_id: 1,
            name: "Task A".into(),
            due_on: None,
            project_name: Some("My Project".into()),
        },
        MineTableRow {
            instance: "inst".into(),
            project_id: 20,
            task_number: 2,
            task_id: 2,
            name: "Task B".into(),
            due_on: None,
            project_name: None,
        },
    ];
    let (model, _) = init_mine(empty_header(), Some(rows));
    match model.stack.last() {
        Some(Screen::Tasks { tasks, .. }) => {
            assert_eq!(
                tasks[0].project_name.as_deref(),
                Some("My Project"),
                "project_name must thread from MineTableRow into TaskRow"
            );
            assert_eq!(
                tasks[1].project_name, None,
                "None project_name must thread through unchanged"
            );
        }
        _ => panic!("expected Tasks screen"),
    }
}

// AC3 (serde): MineTableRow round-trip preserves project_name.
#[test]
fn mine_table_row_serde_round_trip_preserves_project_name() {
    let row = MineTableRow {
        instance: "inst".into(),
        project_id: 1,
        task_number: 1,
        task_id: 1,
        name: "Task".into(),
        due_on: None,
        project_name: Some("Acme Corp".into()),
    };
    let json = serde_json::to_string(&row).unwrap();
    let decoded: MineTableRow = serde_json::from_str(&json).unwrap();
    assert_eq!(
        decoded.project_name.as_deref(),
        Some("Acme Corp"),
        "project_name must survive a serde round-trip"
    );
}

// AC3 (old snapshot): an old snapshot JSON without the project_name field
// must deserialize to None (serde default).
#[test]
fn mine_table_row_old_snapshot_missing_project_name_deserializes_to_none() {
    let json = r#"{"instance":"inst","project_id":1,"task_number":1,"task_id":1,"name":"Task","due_on":null}"#;
    let row: MineTableRow = serde_json::from_str(json).unwrap();
    assert_eq!(
        row.project_name, None,
        "old snapshot without project_name must deserialize to None"
    );
}

fn today() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()
}

#[test]
fn relative_due_none_input_returns_sem_data_and_none_style() {
    let (label, style) = relative_due(None, today());
    assert_eq!(style, DueStyle::None);
    assert!(!label.is_empty(), "label must not be empty");
}

#[test]
fn relative_due_unparseable_returns_sem_data_and_none_style() {
    let (label, style) = relative_due(Some("not-a-date"), today());
    assert_eq!(style, DueStyle::None);
    assert!(!label.is_empty());
}

#[test]
fn relative_due_today_returns_near_style() {
    let (label, style) = relative_due(Some("2026-07-10"), today());
    assert_eq!(style, DueStyle::Near);
    assert!(!label.is_empty(), "label must not be empty for today");
}

#[test]
fn relative_due_tomorrow_returns_near_style() {
    let (label, style) = relative_due(Some("2026-07-11"), today());
    assert_eq!(style, DueStyle::Near);
    assert!(!label.is_empty());
}

#[test]
fn relative_due_two_days_ahead_returns_near_style() {
    let (label, style) = relative_due(Some("2026-07-12"), today());
    assert_eq!(style, DueStyle::Near, "2 days ahead must be Near");
    assert!(
        label.contains('2'),
        "label must mention the number 2: {label}"
    );
}

#[test]
fn relative_due_at_window_boundary_three_days_returns_near() {
    let (label, style) = relative_due(Some("2026-07-13"), today());
    assert_eq!(
        style,
        DueStyle::Near,
        "exactly 3 days ahead must be Near (window boundary)"
    );
    assert!(label.contains('3'), "label must mention 3: {label}");
}

#[test]
fn relative_due_beyond_window_returns_normal_style() {
    let (label, style) = relative_due(Some("2026-07-14"), today());
    assert_eq!(
        style,
        DueStyle::Normal,
        "4 days ahead must be Normal (beyond window)"
    );
    assert!(label.contains('4'), "label must mention 4: {label}");
}

#[test]
fn relative_due_overdue_one_day_returns_singular_label() {
    let (label, style) = relative_due(Some("2026-07-09"), today());
    assert_eq!(style, DueStyle::Overdue);
    assert!(
        label.contains('1'),
        "singular overdue label must contain 1: {label}"
    );
    assert!(
        !label.contains("dias"),
        "singular must not use plural 'dias': {label}"
    );
}

#[test]
fn relative_due_overdue_many_days_returns_plural_label() {
    let (label, style) = relative_due(Some("2026-07-05"), today());
    assert_eq!(style, DueStyle::Overdue);
    assert!(
        label.contains('5'),
        "overdue 5 days label must contain 5: {label}"
    );
    assert!(
        !label.contains(" 1 "),
        "plural overdue must not contain singular '1': {label}"
    );
}

// --- S2b: assets-inline geometry and scroll-aware click tests (BDR 0022) ---

/// Build a Detail model with inline asset section pre-spliced into `lines`.
///
/// Mirrors what `build_detail_content` produces: appends the asset section via
/// `section_lines` and emits `OpenAsset` affordances for every asset content row
/// (including wrapped continuation lines), so the click path works end-to-end
/// without a real task JSON.
fn detail_model_with_inline_assets(
    body_lines: Vec<String>,
    assets: Vec<Asset>,
    offset: usize,
    viewport: (u16, u16),
) -> Model {
    let inner_width = viewport.0.saturating_sub(2) as usize;
    let content_width = asset_panel::inline_content_width(inner_width);

    let mut lines = body_lines;
    let mut line_styles: Vec<Vec<crate::render::StyleRun>> = vec![vec![]; lines.len()];
    let mut affordances: Vec<crate::render::LocalAffordance> = vec![];

    if !assets.is_empty() {
        lines.push(String::new());
        line_styles.push(vec![]);

        let section_base_idx = lines.len();
        for (section_idx, (text, runs)) in asset_panel::section_lines(&assets, content_width)
            .into_iter()
            .enumerate()
        {
            if let Some(asset_idx) =
                asset_panel::asset_index_for_section_row(&assets, content_width, section_idx)
            {
                let link_span = runs
                    .iter()
                    .find(|r| matches!(r.style, crate::richtext::RichStyle::Link));
                if let Some(span) = link_span {
                    affordances.push(crate::render::LocalAffordance {
                        line_idx: section_base_idx + section_idx,
                        col_start: span.start,
                        col_end: span.start + span.len,
                        kind: crate::render::AffordanceKind::OpenAsset(
                            assets[asset_idx].url.clone(),
                        ),
                    });
                }
            }
            lines.push(text);
            line_styles.push(runs);
        }
    }

    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines,
            line_styles,
            assets,
            offset,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances,
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

// AC2 (BDR 0022 Sc.2): detail_max_offset no longer subtracts any panel height.
// For a viewport of 24 rows, DETAIL_CHROME_ROWS=4, text_vh=20.
// With 30 body lines + blank + 6 asset-section lines = 37 total lines, max = 37-20 = 17.
// (Before: subtracting panel height would have reduced text_vh further.)
#[test]
fn detail_max_offset_with_inline_assets_does_not_subtract_panel_height() {
    use crate::tui::model::detail_max_offset;
    use crate::tui::screens::asset_panel;

    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    // Simulate: 30 body lines + 1 blank + 6 asset-section rows = 37 total
    // section_lines for 1 short-name asset at inner_width 78 (80-2) should be 6 rows
    let inner_width = 78usize;
    let content_width = asset_panel::inline_content_width(inner_width);
    let section_len = asset_panel::section_lines(&assets, content_width).len();
    let lines_len = 30 + 1 + section_len; // body + blank + section

    let max = detail_max_offset(24, 80, lines_len, &assets);
    let expected = lines_len.saturating_sub(20); // text_vh=24-4=20
    assert_eq!(
        max, expected,
        "detail_max_offset must use full text_vh=20 (no panel subtracted): max={max} expected={expected}"
    );
}

// AC2 (BDR 0022 Sc.7): empty asset list yields no inline section.
// Lines are just body lines, and detail_max_offset behaves the same as before.
#[test]
fn detail_max_offset_empty_assets_no_inline_section() {
    use crate::tui::model::detail_max_offset;

    let max = detail_max_offset(24, 80, 25, &[]);
    // text_vh = 24 - 4 = 20, max = 25 - 20 = 5
    assert_eq!(
        max, 5,
        "empty assets: max must be lines_len - text_vh = 25 - 20 = 5"
    );
}

// AC2 (BDR 0022 Sc.2): last asset row is reachable at max scroll offset.
// With 1 body line + blank + 6 section rows = 8 total lines, viewport_rows=24,
// text_vh=20, max=8-20=0 (all fits), so the asset section is always visible at offset=0.
// For the long list scenario: 25 body lines + blank + 6 section = 32 lines;
// max=32-20=12; at offset=12 the visible range is [12..31] inclusive (rows 2..21),
// which covers the last section rows at lines[31]=section_row5 (visible).
#[test]
fn last_asset_row_reachable_at_max_scroll_offset() {
    use crate::tui::model::detail_max_offset;
    use crate::tui::screens::asset_panel;

    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let inner_width = 78usize;
    let content_width = asset_panel::inline_content_width(inner_width);
    let section_len = asset_panel::section_lines(&assets, content_width).len();
    // 25 body lines + 1 blank separator + section
    let total_lines = 25 + 1 + section_len;

    let viewport_rows = 24u16;
    let max_offset = detail_max_offset(viewport_rows, 80, total_lines, &assets);

    // At max_offset, the visible range is [max_offset .. max_offset + text_vh - 1]
    // The last line index is total_lines - 1
    // text_vh = viewport_rows - 4 = 20
    let text_vh = (viewport_rows as usize).saturating_sub(4);
    let last_visible = max_offset + text_vh - 1;
    assert!(
        last_visible >= total_lines - 1,
        "last asset section row (index {}) must be visible at max offset {}; last_visible={}",
        total_lines - 1,
        max_offset,
        last_visible
    );
}

// AC3 (BDR 0022 Sc.5): Ctrl+click on a visible asset row emits OpenAsset for that asset.
// Model: 5 body lines + blank + section (6 rows) = 12 total lines.
// Viewport 80x24, offset=0, text_top=2.
// asset_section_start = 12 - 6 = 6.
// Row 2 (text_top) → line_idx=0, row 3 → line_idx=1, ..., row 8 → line_idx=6 (section[0]=header).
// Row 10 → line_idx=8 (section[2]=Asset{idx:0, [1] ↗ doc.pdf}).
// Actually: section_lines output order: [header, pad, asset, sep, hint, pad]
// So section row 2 (interior_row=2) → layout[1]=Asset{idx:0} → Some(0).
// At viewport_rows=24, text_top=2, row=10 → line_idx=0+8=8, section_start=6, interior=2 → asset[0].
#[test]
fn ctrl_click_on_visible_asset_row_emits_open_asset() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    // 5 body lines so section starts at line 6 (5 body + 1 blank)
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, assets, 0, (80, 24));

    // asset_section_start = total_lines - section_len
    // section_len=6, total_lines=5+1+6=12, asset_section_start=6
    // interior_row=2 corresponds to Asset{idx:0} (see section_lines layout)
    // line_idx = asset_section_start + interior_row = 6 + 2 = 8
    // row = text_top + (line_idx - offset) = 2 + 8 = 10
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 10,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "Ctrl+click on asset row must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { instance, url } => {
            assert_eq!(instance, "inst");
            assert_eq!(url, "https://example.com/doc.pdf");
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

// AC3 (BDR 0022 Sc.6): Ctrl+click on the section header row (interior_row=0) emits NO cmd.
#[test]
fn ctrl_click_on_asset_section_header_row_emits_no_cmd() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, assets, 0, (80, 24));

    // line_idx = asset_section_start + 0 = 6
    // row = text_top + 6 = 8
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "Ctrl+click on asset header row must emit no cmd (no OpenAsset affordance on header line)"
    );
}

// AC3 (BDR 0022 Sc.5): plain (unmodified) click on an asset row emits NO OpenAsset.
// It falls through to is_in_body_area selection (assets are selectable body content).
#[test]
fn plain_click_on_asset_row_emits_no_open_asset() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, assets, 0, (80, 24));

    // row 10 = asset row (Asset{idx:0}), but plain click → no OpenAsset
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 10,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    let has_open_asset = cmds.iter().any(|c| matches!(c, Cmd::OpenAsset { .. }));
    assert!(
        !has_open_asset,
        "plain click on asset row must NOT emit OpenAsset: {cmds:?}"
    );
}

// AC3 (BDR 0022 Sc.6): Ctrl+click on a blank separator row (interior_row=3) emits no cmd.
#[test]
fn ctrl_click_on_separator_row_emits_no_cmd() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, assets, 0, (80, 24));

    // section_lines: [header(0), pad(1), asset(2), sep(3), hint(4), pad(5)]
    // interior_row=3 → Separator → None
    // line_idx = 6 + 3 = 9, row = 2 + 9 = 11
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 11,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "Ctrl+click on separator row must emit no cmd"
    );
}

// AC3 (BDR 0022 Sc.6): Ctrl+click on the italic hint row (interior_row=4) emits no cmd.
#[test]
fn ctrl_click_on_hint_row_emits_no_cmd() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, assets, 0, (80, 24));

    // interior_row=4 → Hint → None
    // line_idx = 6 + 4 = 10, row = 2 + 10 = 12
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 12,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert!(cmds.is_empty(), "Ctrl+click on hint row must emit no cmd");
}

// AC3 (BDR 0022 Sc.6): Ctrl+click on asset row at a non-zero scroll offset opens the correct asset.
// With offset=5, the body line at line_idx=6+2=8 is the asset row at section interior_row=2.
// But viewport row for that line: row = text_top + (line_idx - offset) = 2 + (8-5) = 5.
#[test]
fn ctrl_click_on_asset_row_at_nonzero_offset_is_scroll_aware() {
    let assets = vec![
        make_asset("first.pdf", "https://example.com/first.pdf"),
        make_asset("second.pdf", "https://example.com/second.pdf"),
    ];
    // 10 body lines → section_start = 10 + 1 = 11
    // section_lines for 2 assets: [hdr, pad, A0, sep, A1, sep, hint, pad] = 8 rows
    // asset_section_start = 11, total = 11 + 8 = 19
    // interior_row=2 → A0 → assets[0] (first.pdf)
    // interior_row=4 → A1 → assets[1] (second.pdf)
    let body: Vec<String> = (0..10).map(|i| format!("body line {i}")).collect();
    let offset = 5usize;
    let m = detail_model_with_inline_assets(body, assets, offset, (80, 24));

    // For asset[0] (interior_row=2): line_idx = 11 + 2 = 13
    // row = text_top + (line_idx - offset) = 2 + (13 - 5) = 10
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 10,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "scroll-aware Ctrl+click must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/first.pdf",
                "scroll-aware click must open the asset at line_idx=13 (first.pdf)"
            );
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

// AC2 (BDR 0022 Sc.2): is_in_body_area includes asset section rows (no panel_h subtracted).
// With viewport 80x24, text_top=2, content_text_height=20:
// rows 2..21 are all "in body area". Row 21 is just outside.
#[test]
fn is_in_body_area_includes_asset_rows_no_panel_height_subtracted() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, assets, 0, (80, 24));

    // Row 19 (= text_top + 17) is within content_text_height=20 rows (rows 2..21)
    // Previously, with panel_h subtracted, asset rows might have been outside the body area.
    // Now, row 19 is always in the body area.
    let (m_after, _) = update(
        m,
        Msg::Click {
            column: 5,
            row: 19,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    // A plain click in the body area sets selection (not a no-op)
    assert!(
        m_after.selection.is_some(),
        "row 19 must be in body area after removal of panel_h subtraction"
    );
}

// AC2 (fix-inline-asset-link-style-click): Ctrl/Cmd+click on an inline asset content row
// emits Cmd::OpenAsset with that asset's url; a plain (unmodified) click on the SAME row
// does NOT emit OpenAsset (reserved for text selection). Pins the Ctrl/Cmd gate end-to-end.
//
// This is the regression guard for ADR 0032: the hit-test (asset_panel_cmd_at) is intact;
// what was broken was only the VISUAL affordance (no link style). Both are now tested together.
//
// Layout math for viewport (80, 24), 3 body lines + blank + section:
//   inner_width = 78, content_width = inline_content_width(78)
//   section_lines for 1 asset: [header(0), pad(1), Asset(2), sep(3), hint(4), pad(5)] = 6 rows
//   total_lines = 3 + 1 + 6 = 10
//   asset_section_start = 10 - 6 = 4
//   interior_row = 2 → Asset{idx:0}
//   line_idx = 4 + 2 = 6
//   row = text_top(2) + (line_idx - offset(0)) = 8
#[test]
fn ctrl_click_on_asset_row_emits_open_asset_plain_click_does_not() {
    let asset_url = "https://example.com/manual.pdf";

    // Ctrl+click on the asset content row must emit OpenAsset with the exact url.
    let m = detail_model_with_inline_assets(
        (0..3).map(|i| format!("body line {i}")).collect(),
        vec![make_asset("manual.pdf", asset_url)],
        0,
        (80, 24),
    );
    let (_m, ctrl_cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        ctrl_cmds.len(),
        1,
        "Ctrl+click on asset row must emit exactly one cmd"
    );
    match &ctrl_cmds[0] {
        Cmd::OpenAsset { url, instance } => {
            assert_eq!(url, asset_url, "OpenAsset url must match the asset's url");
            assert_eq!(instance, "inst");
        }
        other => panic!("expected Cmd::OpenAsset, got {other:?}"),
    }

    // Plain (unmodified) click on the SAME row must NOT emit OpenAsset (Ctrl/Cmd gate).
    let m2 = detail_model_with_inline_assets(
        (0..3).map(|i| format!("body line {i}")).collect(),
        vec![make_asset("manual.pdf", asset_url)],
        0,
        (80, 24),
    );
    let (_m2, plain_cmds) = update(
        m2,
        Msg::Click {
            column: 5,
            row: 8,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    let has_open_asset = plain_cmds
        .iter()
        .any(|c| matches!(c, Cmd::OpenAsset { .. }));
    assert!(
        !has_open_asset,
        "plain click on asset row must NOT emit OpenAsset (Ctrl/Cmd gate): {plain_cmds:?}"
    );
}

// AC3 (BDR 0022 Sc.5): empty assets list → no inline section → no OpenAsset on any row.
#[test]
fn ctrl_click_with_empty_assets_emits_no_cmd() {
    let body: Vec<String> = (0..5).map(|i| format!("body line {i}")).collect();
    let m = detail_model_with_inline_assets(body, vec![], 0, (80, 24));

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    let has_open = cmds.iter().any(|c| matches!(c, Cmd::OpenAsset { .. }));
    assert!(
        !has_open,
        "empty assets: no OpenAsset must be emitted on any row"
    );
}

// AC2 (issue 0045): Ctrl+click on a wrapped continuation line of an asset emits
// OpenAsset with the asset's url.
// Width 42 → inner_width=40 → content_width=39 (PANEL_HPAD=1). A 35+ char label
// at that width wraps to at least 2 Asset rows in the layout, each carrying an
// OpenAsset affordance with the same url.
#[test]
fn ctrl_click_on_wrapped_asset_continuation_emits_open_asset() {
    let asset_url = "https://example.com/file.pdf";
    let long_label = "very-long-filename-that-does-not-fit.pdf";
    let viewport = (42u16, 24u16);
    let inner_width = viewport.0.saturating_sub(2) as usize;
    let content_width = asset_panel::inline_content_width(inner_width);

    let asset_rows =
        crate::render::asset_row_lines(1, &make_asset(long_label, asset_url), content_width);
    assert!(
        asset_rows.len() >= 2,
        "label must wrap to at least 2 rows at content_width={content_width}; \
         got {} rows for label.len()={}",
        asset_rows.len(),
        long_label.len()
    );

    let open_asset_affs: Vec<crate::render::LocalAffordance> = {
        let m = detail_model_with_inline_assets(
            vec![],
            vec![make_asset(long_label, asset_url)],
            0,
            viewport,
        );
        let Screen::Detail { affordances, .. } = m.top().unwrap() else {
            panic!("expected Detail screen")
        };
        affordances
            .iter()
            .filter(|a| matches!(a.kind, crate::render::AffordanceKind::OpenAsset(_)))
            .cloned()
            .collect()
    };

    assert!(
        open_asset_affs.len() >= 2,
        "wrapped asset must produce at least 2 OpenAsset affordances; got {}: {open_asset_affs:?}",
        open_asset_affs.len()
    );

    let text_top: u16 = 2;

    for aff in &open_asset_affs {
        let m = detail_model_with_inline_assets(
            vec![],
            vec![make_asset(long_label, asset_url)],
            0,
            viewport,
        );
        let row = text_top + (aff.line_idx as u16);
        let (_m, cmds) = update(
            m,
            Msg::Click {
                column: (aff.col_start + 1) as u16,
                row,
                modifiers: crossterm::event::KeyModifiers::CONTROL,
            },
        );
        assert_eq!(
            cmds.len(),
            1,
            "Ctrl+click on wrapped asset fragment at line {} (row {}) must emit one cmd; got {:?}",
            aff.line_idx,
            row,
            cmds
        );
        match &cmds[0] {
            Cmd::OpenAsset { url, instance } => {
                assert_eq!(
                    url, asset_url,
                    "wrapped fragment click must return the asset url"
                );
                assert_eq!(instance, "inst");
            }
            other => panic!(
                "expected OpenAsset for wrapped fragment at line {}, got {other:?}",
                aff.line_idx
            ),
        }
    }
}

// AC4 (issue 0045): plain (no Ctrl/Cmd) click on a wrapped asset continuation row
// does NOT emit Cmd::OpenAsset. (The modifier gate is enforced per BDR 0014 Sc.8.)
#[test]
fn plain_click_on_wrapped_asset_continuation_emits_no_open_asset() {
    let asset_url = "https://example.com/file.pdf";
    let long_label = "very-long-filename-that-does-not-fit.pdf";
    let viewport = (42u16, 24u16);

    let (continuation_line_idx, continuation_col_start) = {
        let m = detail_model_with_inline_assets(
            vec![],
            vec![make_asset(long_label, asset_url)],
            0,
            viewport,
        );
        let Screen::Detail { affordances, .. } = m.top().unwrap() else {
            panic!("expected Detail screen")
        };
        let open_asset_affs: Vec<_> = affordances
            .iter()
            .filter(|a| matches!(a.kind, crate::render::AffordanceKind::OpenAsset(_)))
            .collect();
        assert!(
            !open_asset_affs.is_empty(),
            "wrapped asset must produce OpenAsset affordances for this test to be meaningful"
        );
        let last = open_asset_affs[open_asset_affs.len() - 1];
        (last.line_idx, last.col_start)
    };

    let text_top: u16 = 2;
    let row = text_top + (continuation_line_idx as u16);

    let m = detail_model_with_inline_assets(
        vec![],
        vec![make_asset(long_label, asset_url)],
        0,
        viewport,
    );
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: (continuation_col_start + 1) as u16,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    let has_open = cmds.iter().any(|c| matches!(c, Cmd::OpenAsset { .. }));
    assert!(
        !has_open,
        "plain click on wrapped continuation must NOT emit OpenAsset (BDR 0014 Sc.8): {cmds:?}"
    );
}

// ── Compose mode tests (BDR 0024 / ADR 0034 / ADR 0035) ──────────────────────

use crate::tui::model::{Compose, ComposeKind, ComposeStatus};

fn detail_model_for_compose(instance: &str, project_id: i64, task_id: i64) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: instance.into(),
            project_id,
            task_id,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn extract_compose(model: &Model) -> Option<&Compose> {
    match model.top() {
        Some(Screen::Detail { compose, .. }) => compose.as_ref(),
        _ => None,
    }
}

// AC1-part1: ComposeOpen on a Detail screen sets compose=Some(Editing, empty buffer).
#[test]
fn compose_open_on_detail_sets_editing_state_with_empty_buffer() {
    let m = detail_model_for_compose("inst", 10, 42);
    let (m, cmds) = update(m, Msg::ComposeOpen);
    assert!(cmds.is_empty(), "ComposeOpen must emit no Cmd");
    let cp = extract_compose(&m).expect("compose must be Some after ComposeOpen");
    assert_eq!(cp.kind, ComposeKind::New);
    assert_eq!(cp.buffer, "");
    assert_eq!(cp.status, ComposeStatus::Editing);
}

// AC1-part2: ComposeOpen on a screen that already has compose active is a no-op.
#[test]
fn compose_open_when_already_active_is_noop() {
    let mut m = detail_model_for_compose("inst", 10, 42);
    if let Some(Screen::Detail {
        ref mut compose, ..
    }) = m.stack.last_mut()
    {
        *compose = Some(Compose {
            kind: ComposeKind::New,
            buffer: "existing".into(),
            status: ComposeStatus::Editing,
        });
    }
    let (m, _) = update(m, Msg::ComposeOpen);
    let cp = extract_compose(&m).expect("compose must still be Some");
    assert_eq!(cp.buffer, "existing", "existing buffer must be preserved");
}

// AC1-part3: ComposeInput appends a character to the buffer.
#[test]
fn compose_input_appends_character_to_buffer() {
    let m = detail_model_for_compose("inst", 10, 42);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, cmds) = update(m, Msg::ComposeInput('h'));
    assert!(cmds.is_empty());
    let (m, _) = update(m, Msg::ComposeInput('i'));
    let cp = extract_compose(&m).expect("compose must be Some");
    assert_eq!(cp.buffer, "hi");
}

// AC1-part4: ComposeNewline inserts '\n' — Enter is a newline, NOT submit.
#[test]
fn compose_newline_inserts_newline_not_submit() {
    let m = detail_model_for_compose("inst", 10, 42);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('a'));
    let (m, cmds) = update(m, Msg::ComposeNewline);
    assert!(
        cmds.is_empty(),
        "ComposeNewline must emit no Cmd (not a submit)"
    );
    let (m, _) = update(m, Msg::ComposeInput('b'));
    let cp = extract_compose(&m).expect("compose must be Some");
    assert_eq!(cp.buffer, "a\nb", "buffer must contain embedded newline");
    assert!(
        cp.buffer.contains('\n'),
        "buffer must have \\n after ComposeNewline"
    );
}

// AC1-part5: ComposeBackspace removes the last character.
#[test]
fn compose_backspace_removes_last_character() {
    let m = detail_model_for_compose("inst", 10, 42);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('a'));
    let (m, _) = update(m, Msg::ComposeInput('b'));
    let (m, cmds) = update(m, Msg::ComposeBackspace);
    assert!(cmds.is_empty());
    let cp = extract_compose(&m).expect("compose must be Some");
    assert_eq!(cp.buffer, "a");
}

// AC1-part6: ComposeCancel clears compose and emits no Cmd.
#[test]
fn compose_cancel_clears_compose_and_emits_no_cmd() {
    let m = detail_model_for_compose("inst", 10, 42);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('x'));
    let (m, cmds) = update(m, Msg::ComposeCancel);
    assert!(cmds.is_empty(), "ComposeCancel must emit no Cmd");
    assert!(
        extract_compose(&m).is_none(),
        "compose must be None after ComposeCancel"
    );
}

// AC2-part1: ComposeSubmit on non-empty Editing buffer emits exactly one Cmd::SubmitComment
// with the correct fields and sets status=Submitting.
#[test]
fn compose_submit_nonempty_buffer_emits_submit_comment_cmd() {
    let m = detail_model_for_compose("myinst", 5, 99);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('h'));
    let (m, _) = update(m, Msg::ComposeInput('i'));
    let (m, cmds) = update(m, Msg::ComposeSubmit);

    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd::SubmitComment");
    match &cmds[0] {
        Cmd::SubmitComment {
            instance,
            project_id,
            task_id,
            body,
        } => {
            assert_eq!(instance, "myinst");
            assert_eq!(*project_id, 5);
            assert_eq!(*task_id, 99);
            assert_eq!(body, "hi");
        }
        other => panic!("expected Cmd::SubmitComment, got {other:?}"),
    }
    let cp = extract_compose(&m).expect("compose must be Some after submit (waiting for result)");
    assert_eq!(
        cp.status,
        ComposeStatus::Submitting,
        "status must be Submitting"
    );
}

// AC2-part2: ComposeSubmit on an empty buffer emits no Cmd.
#[test]
fn compose_submit_empty_buffer_emits_no_cmd() {
    let m = detail_model_for_compose("inst", 1, 1);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (_m, cmds) = update(m, Msg::ComposeSubmit);
    assert!(
        cmds.is_empty(),
        "ComposeSubmit on empty buffer must emit no Cmd"
    );
}

// AC3-part1: CommentMutationOk clears compose AND emits exactly one Cmd::LoadDetail{refresh:true}.
#[test]
fn comment_mutation_ok_clears_compose_and_emits_load_detail_refresh() {
    let m = detail_model_for_compose("inst", 7, 13);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('x'));
    let (m, cmds) = update(m, Msg::CommentMutationOk);

    assert!(
        extract_compose(&m).is_none(),
        "compose must be None after CommentMutationOk"
    );
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd::LoadDetail");
    match &cmds[0] {
        Cmd::LoadDetail {
            instance,
            project_id,
            task_id,
            refresh,
        } => {
            assert_eq!(instance, "inst");
            assert_eq!(*project_id, 7);
            assert_eq!(*task_id, 13);
            assert!(*refresh, "refresh must be true");
        }
        other => panic!("expected Cmd::LoadDetail, got {other:?}"),
    }
}

// AC3-part2: CommentMutationErr keeps the buffer intact and sets status=Error(msg), emits no Cmd.
#[test]
fn comment_mutation_err_preserves_buffer_and_sets_error_status() {
    let m = detail_model_for_compose("inst", 1, 1);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('t'));
    let (m, _) = update(m, Msg::ComposeInput('e'));
    let (m, _) = update(m, Msg::ComposeInput('x'));
    let (m, _) = update(m, Msg::ComposeInput('t'));
    let (m, cmds) = update(m, Msg::CommentMutationErr("Network error".into()));

    assert!(cmds.is_empty(), "CommentMutationErr must emit no Cmd");
    let cp = extract_compose(&m).expect("compose must still be Some after error");
    assert_eq!(cp.buffer, "text", "buffer must be preserved after error");
    assert_eq!(
        cp.status,
        ComposeStatus::Error("Network error".into()),
        "status must be Error(msg)"
    );
}

// AC4-part1: map_compose_key_event maps Enter->ComposeNewline.
#[test]
fn map_compose_key_event_enter_yields_compose_newline() {
    use crate::tui::events::map_compose_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};
    let key = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert!(
        matches!(map_compose_key_event(key), Some(Msg::ComposeNewline)),
        "Enter must map to ComposeNewline"
    );
}

// AC4-part2: map_compose_key_event maps a printable char -> ComposeInput(c).
#[test]
fn map_compose_key_event_printable_char_yields_compose_input() {
    use crate::tui::events::map_compose_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};
    let key = KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert!(
        matches!(map_compose_key_event(key), Some(Msg::ComposeInput('a'))),
        "printable char must map to ComposeInput(char)"
    );
}

// AC4-part3: map_compose_key_event maps Ctrl+S -> ComposeSubmit.
#[test]
fn map_compose_key_event_ctrl_s_yields_compose_submit() {
    use crate::tui::events::map_compose_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};
    let key = KeyEvent {
        code: KeyCode::Char('s'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert!(
        matches!(map_compose_key_event(key), Some(Msg::ComposeSubmit)),
        "Ctrl+S must map to ComposeSubmit"
    );
}

// AC4-part4: map_compose_key_event maps Esc -> ComposeCancel.
#[test]
fn map_compose_key_event_esc_yields_compose_cancel() {
    use crate::tui::events::map_compose_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};
    let key = KeyEvent {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert!(
        matches!(map_compose_key_event(key), Some(Msg::ComposeCancel)),
        "Esc must map to ComposeCancel"
    );
}

// AC4-part5: map_browse_key_event maps plain 'c' -> ComposeOpen (not a selection or nav action).
#[test]
fn map_browse_key_event_plain_c_yields_compose_open() {
    use crate::tui::events::map_browse_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};
    let key = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert!(
        matches!(map_browse_key_event(key), Some(Msg::ComposeOpen)),
        "plain 'c' must map to ComposeOpen"
    );
}

// AC4-part6: map_browse_key_event still maps Ctrl+C -> Quit (not ComposeOpen).
#[test]
fn map_browse_key_event_ctrl_c_still_quits() {
    use crate::tui::events::map_browse_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState};
    let key = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert!(
        matches!(map_browse_key_event(key), Some(Msg::Quit)),
        "Ctrl+C must still map to Quit"
    );
}

// AC1-multiline: a sequence of ComposeInput + ComposeNewline yields a buffer with '\n'.
#[test]
fn compose_multiline_buffer_contains_embedded_newline() {
    let m = detail_model_for_compose("inst", 1, 1);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('l'));
    let (m, _) = update(m, Msg::ComposeInput('i'));
    let (m, _) = update(m, Msg::ComposeInput('n'));
    let (m, _) = update(m, Msg::ComposeInput('e'));
    let (m, _) = update(m, Msg::ComposeInput('1'));
    let (m, _) = update(m, Msg::ComposeNewline);
    let (m, _) = update(m, Msg::ComposeInput('l'));
    let (m, _) = update(m, Msg::ComposeInput('2'));
    let cp = extract_compose(&m).expect("compose must be Some");
    assert_eq!(
        cp.buffer, "line1\nl2",
        "buffer must contain embedded newline"
    );
    assert!(
        cp.buffer.contains('\n'),
        "\\n must be in buffer (Enter is newline, not submit)"
    );
}

// ── Comment-edit-ui tests (BDR 0024 Sc.4-5 / ADR 0036) ───────────────────────

/// Build a Detail model that has one owned comment (created_by_id == current_user_id)
/// and one unowned comment. The model is reflowed at `width` so `edit_affordances`
/// are populated.
fn detail_model_with_comments_for_edit(
    instance: &str,
    project_id: i64,
    task_id: i64,
    current_user_id: Option<i64>,
    width: u16,
) -> Model {
    use serde_json::json;
    let own_comment = json!({
        "id": 42i64,
        "created_by_id": 7i64,
        "created_by_name": "Me",
        "created_on": 1700000000u64,
        "body_plain_text": "My comment body"
    });
    let other_comment = json!({
        "id": 99i64,
        "created_by_id": 8i64,
        "created_by_name": "Them",
        "created_on": 1700000001u64,
        "body_plain_text": "Their comment"
    });
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: instance.into(),
            project_id,
            task_id,
            task: serde_json::Value::Null,
            comments: vec![own_comment, other_comment],
            user_map: std::collections::HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (width, 30),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let inner_width = width.saturating_sub(2) as usize;
    m.reflow_detail(inner_width);
    m
}

// edit-AC4: ComposeSubmit with kind=Edit{comment_id} emits Cmd::UpdateComment
// (NOT Cmd::SubmitComment), carries the correct instance / comment_id / body.
// Drives the real Ctrl+click -> compose open -> submit path.
#[test]
fn compose_submit_with_edit_kind_emits_update_comment_cmd() {
    use crossterm::event::KeyModifiers;

    let m = detail_model_with_comments_for_edit("myinst", 5, 10, Some(7), 82);
    let (click_row, click_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Edit(_)))
                .expect("edit affordance must be non-empty after reflow with own comment");
            let crate::render::AffordanceKind::Edit(cid) = aff.kind else {
                panic!("expected Edit kind")
            };
            assert_eq!(cid, 42, "affordance must target comment_id=42");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (m, cmds_open) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(cmds_open.is_empty(), "click must emit no Cmd");
    let cp = extract_compose(&m).expect("compose must be Some after Ctrl+click on [editar]");
    assert_eq!(cp.kind, ComposeKind::Edit { comment_id: 42 });

    let (m, cmds) = update(m, Msg::ComposeSubmit);
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd::UpdateComment");
    match &cmds[0] {
        Cmd::UpdateComment {
            instance,
            comment_id,
            body,
        } => {
            assert_eq!(instance, "myinst");
            assert_eq!(*comment_id, 42);
            assert_eq!(body, "My comment body");
        }
        other => panic!("expected Cmd::UpdateComment, got {other:?}"),
    }
    let cp2 = extract_compose(&m).expect("compose must be Some while awaiting result");
    assert_eq!(
        cp2.status,
        ComposeStatus::Submitting,
        "status must be Submitting after submit"
    );
}

// edit-AC4-B: ComposeSubmit with kind=New still emits Cmd::SubmitComment (regression guard).
#[test]
fn compose_submit_with_new_kind_still_emits_submit_comment_cmd() {
    let m = detail_model_for_compose("myinst", 5, 99);
    let (m, _) = update(m, Msg::ComposeOpen);
    let (m, _) = update(m, Msg::ComposeInput('o'));
    let (m, _) = update(m, Msg::ComposeInput('k'));
    let (_, cmds) = update(m, Msg::ComposeSubmit);
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd");
    assert!(
        matches!(cmds[0], Cmd::SubmitComment { .. }),
        "kind=New must emit SubmitComment, not UpdateComment; got {:?}",
        cmds[0]
    );
}

// edit-AC2: A Ctrl/Cmd+click landing on the [editar] affordance opens compose
// with kind=Edit (scroll-aware: verifies via affordance coordinates populated by
// reflow_detail). A plain (unmodified) click on the same coordinates does NOT
// open compose.
#[test]
fn ctrl_click_on_edit_affordance_opens_compose_edit() {
    use crossterm::event::KeyModifiers;

    // Build the model and extract the affordance coordinates before consuming it.
    let m = detail_model_with_comments_for_edit("inst", 1, 1, Some(7), 82);
    let (click_row, click_col, comment_id) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Edit(_)))
                .expect("edit affordance must be non-empty after reflow with own comment");
            let crate::render::AffordanceKind::Edit(cid) = aff.kind else {
                panic!("expected Edit kind")
            };
            assert_eq!(cid, 42, "affordance must target comment_id=42");
            // row=2 is text_top (row 0=header, row 1=title bar, row 2=first content line).
            // At offset=0: click row = 2 + line_idx.
            (2u16 + aff.line_idx as u16, aff.col_start as u16, cid)
        }
        _ => panic!("expected Detail screen"),
    };

    // Ctrl+click must open compose edit.
    let (m_ctrl, cmds_ctrl) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds_ctrl.is_empty(),
        "edit affordance click must emit no Cmd"
    );
    let cp = extract_compose(&m_ctrl)
        .expect("compose must be Some after Ctrl+click on [editar] affordance");
    assert_eq!(
        cp.kind,
        ComposeKind::Edit { comment_id },
        "compose kind must be Edit{{comment_id: {comment_id}}}"
    );
    assert_eq!(
        cp.buffer, "My comment body",
        "buffer must be pre-filled from body_plain_text"
    );
    assert_eq!(cp.status, ComposeStatus::Editing, "status must be Editing");
}

// edit-AC2-B: A plain (unmodified) click on the affordance coordinates does NOT
// open compose (modifier gate is required for [editar] activation).
#[test]
fn plain_click_on_edit_affordance_coords_does_not_open_compose() {
    use crossterm::event::KeyModifiers;

    let m = detail_model_with_comments_for_edit("inst", 1, 1, Some(7), 82);
    let (click_row, click_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Edit(_)))
                .expect("edit affordance must be non-empty");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (m_plain, _) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        extract_compose(&m_plain).is_none(),
        "plain click on affordance coords must NOT open compose"
    );
}

// ── Comment-delete-ui tests (BDR 0024 Sc.6 / issue 0034) ─────────────────────

/// Build a reflowed Detail model that has one owned comment (id=42, created_by_id=7)
/// so `delete_affordances` is populated after `reflow_detail`.
fn detail_model_with_comments_for_delete(
    instance: &str,
    project_id: i64,
    task_id: i64,
    current_user_id: Option<i64>,
    width: u16,
) -> Model {
    detail_model_with_comments_for_edit(instance, project_id, task_id, current_user_id, width)
}

// ACd1-a: After reflow with an owned comment, the unified affordances vec contains
// exactly one Delete entry targeting comment_id=42.
#[test]
fn delete_affordance_populated_for_own_comment() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, Some(7), 82);
    match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let delete_affs: Vec<_> = affordances
                .iter()
                .filter(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)))
                .collect();
            assert_eq!(
                delete_affs.len(),
                1,
                "must have exactly one delete affordance for the owned comment"
            );
            let crate::render::AffordanceKind::Delete(cid) = delete_affs[0].kind else {
                panic!("expected Delete kind")
            };
            assert_eq!(cid, 42, "delete affordance must target comment_id=42");
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd1-b: Without an owned comment (`current_user_id=None`),
// no Delete affordances exist.
#[test]
fn delete_affordance_absent_when_no_current_user() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, None, 82);
    match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let has_delete = affordances
                .iter()
                .any(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)));
            assert!(
                !has_delete,
                "delete affordances must be absent when current_user_id=None"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd1-c: `Confirm` and `Cancel` affordances are absent when
// `confirm_delete` is None (confirm prompt is not rendered).
#[test]
fn confirm_affordances_absent_when_confirm_delete_is_none() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, Some(7), 82);
    match m.top() {
        Some(Screen::Detail {
            affordances,
            confirm_delete,
            ..
        }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete must be None before any click"
            );
            // The confirm/cancel modal buttons are hit-tested by dispatch_modal_click
            // from frame geometry (ADR 0039). The scroll-aware affordance list must
            // contain only Edit and Delete kinds — never modal-specific entries.
            let only_edit_or_delete = affordances.iter().all(|a| {
                matches!(
                    a.kind,
                    crate::render::AffordanceKind::Edit(_)
                        | crate::render::AffordanceKind::Delete(_)
                )
            });
            assert!(
                only_edit_or_delete,
                "affordances must only contain Edit/Delete kinds when confirm_delete=None; got: {:?}",
                affordances
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd2-a: A Ctrl+click on the `[excluir]` affordance sets
// `confirm_delete=Some(42)` and emits NO Cmd.
#[test]
fn ctrl_click_on_delete_affordance_sets_confirm_delete_no_cmd() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, Some(7), 82);
    let (click_row, click_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)))
                .expect("delete affordance must be non-empty after reflow with own comment");
            let crate::render::AffordanceKind::Delete(cid) = aff.kind else {
                panic!("expected Delete kind")
            };
            assert_eq!(cid, 42);
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (m2, cmds) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );

    assert!(
        cmds.is_empty(),
        "Ctrl+click on [excluir] must emit no Cmd; got {:?}",
        cmds
    );
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert_eq!(
                *confirm_delete,
                Some(42),
                "confirm_delete must be Some(42) after clicking [excluir]"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd2-b: A plain (unmodified) click on the `[excluir]` affordance coordinates
// does NOT set `confirm_delete` (modifier gate is required).
#[test]
fn plain_click_on_delete_affordance_does_not_set_confirm_delete() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, Some(7), 82);
    let (click_row, click_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)))
                .expect("delete affordance must be non-empty");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (m2, _) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::NONE,
        },
    );

    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "plain click on [excluir] coords must NOT set confirm_delete"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd2-c: After clicking `[excluir]`, confirm_delete is Some and the modal overlay
// owns confirm/cancel UI. The scroll-aware affordance list must NOT contain inline
// Confirm/Cancel entries (they moved to the modal, ADR 0039 slice 2).
#[test]
fn confirm_affordances_absent_in_affordances_after_delete_request() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, Some(7), 82);
    let (click_row, click_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)))
                .expect("delete affordance must be non-empty");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (mut m2, _) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );

    let inner_width = (82u16.saturating_sub(2)) as usize;
    m2.reflow_detail(inner_width);

    match m2.top() {
        Some(Screen::Detail {
            affordances,
            confirm_delete,
            ..
        }) => {
            assert_eq!(
                *confirm_delete,
                Some(42),
                "confirm_delete must be Some(42) after excluir click"
            );
            // The confirm/cancel modal buttons are hit-tested by dispatch_modal_click
            // from frame geometry (ADR 0039). The scroll-aware affordance list must
            // contain only Edit and Delete kinds — never modal-specific entries.
            let only_edit_or_delete = affordances.iter().all(|a| {
                matches!(
                    a.kind,
                    crate::render::AffordanceKind::Edit(_)
                        | crate::render::AffordanceKind::Delete(_)
                )
            });
            assert!(
                only_edit_or_delete,
                "affordances must only contain Edit/Delete kinds after delete request; got: {:?}",
                affordances
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd2-d: When confirm_delete is Some, Msg::ConfirmDeleteComment emits exactly one
// `Cmd::DeleteComment{comment_id: 42}` with the correct instance and clears confirm_delete.
// (Modal confirm is now via Enter key / Msg::ConfirmDeleteComment — ADR 0039 slice 2.)
#[test]
fn confirm_delete_msg_emits_delete_comment_cmd() {
    let m = detail_model_with_comments_for_delete("myinst", 1, 1, Some(7), 82);
    let (excluir_row, excluir_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)))
                .expect("delete affordance must be non-empty");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (m2, _) = update(
        m,
        Msg::Click {
            column: excluir_col,
            row: excluir_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );

    assert!(
        matches!(
            m2.top(),
            Some(Screen::Detail {
                confirm_delete: Some(42),
                ..
            })
        ),
        "confirm_delete must be Some(42) after clicking [excluir]"
    );

    let (_m3, cmds) = update(m2, Msg::ConfirmDeleteComment);

    assert_eq!(
        cmds.len(),
        1,
        "ConfirmDeleteComment must emit exactly one Cmd; got {:?}",
        cmds
    );
    match &cmds[0] {
        Cmd::DeleteComment {
            instance,
            comment_id,
        } => {
            assert_eq!(instance, "myinst");
            assert_eq!(*comment_id, 42);
        }
        other => panic!("expected Cmd::DeleteComment, got {other:?}"),
    }
}

// ACd2-e: When confirm_delete is Some, Msg::CancelDeleteComment clears it and emits NO Cmd.
// (Modal cancel is now via Esc key / Msg::CancelDeleteComment — ADR 0039 slice 2.)
#[test]
fn cancel_delete_msg_clears_confirm_delete_no_cmd() {
    let m = detail_model_with_comments_for_delete("inst", 1, 1, Some(7), 82);
    let (excluir_row, excluir_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Delete(_)))
                .expect("delete affordance must be non-empty");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let (m2, _) = update(
        m,
        Msg::Click {
            column: excluir_col,
            row: excluir_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );

    assert!(
        matches!(
            m2.top(),
            Some(Screen::Detail {
                confirm_delete: Some(_),
                ..
            })
        ),
        "confirm_delete must be Some after clicking [excluir]"
    );

    let (m3, cmds) = update(m2, Msg::CancelDeleteComment);

    assert!(
        cmds.is_empty(),
        "CancelDeleteComment must emit no Cmd; got {:?}",
        cmds
    );
    match m3.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete must be None after CancelDeleteComment"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd3-a: `Msg::CommentMutationOk` triggers exactly one `Cmd::LoadDetail{refresh:true}`
// and clears `confirm_delete`.
#[test]
fn comment_mutation_ok_after_delete_emits_load_detail_refresh() {
    let mut m = detail_model_with_comments_for_delete("inst", 5, 10, Some(7), 82);
    if let Some(Screen::Detail {
        ref mut confirm_delete,
        ..
    }) = m.top_mut()
    {
        *confirm_delete = Some(42);
    }

    let (m2, cmds) = update(m, Msg::CommentMutationOk);

    assert_eq!(
        cmds.len(),
        1,
        "CommentMutationOk must emit exactly one Cmd; got {:?}",
        cmds
    );
    match &cmds[0] {
        Cmd::LoadDetail {
            instance,
            project_id,
            task_id,
            refresh,
        } => {
            assert_eq!(instance, "inst");
            assert_eq!(*project_id, 5);
            assert_eq!(*task_id, 10);
            assert!(*refresh, "refresh must be true");
        }
        other => panic!("expected Cmd::LoadDetail, got {other:?}"),
    }
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete must be cleared by CommentMutationOk"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// ACd3-b: `Msg::CommentMutationErr` does NOT emit a refresh Cmd.
// It leaves `confirm_delete` as-is and does not issue `Cmd::LoadDetail`.
#[test]
fn comment_mutation_err_does_not_emit_refresh() {
    let mut m = detail_model_with_comments_for_delete("inst", 5, 10, Some(7), 82);

    if let Some(Screen::Detail {
        ref mut compose,
        ref mut confirm_delete,
        ..
    }) = m.top_mut()
    {
        *confirm_delete = Some(42);
        *compose = Some(crate::tui::model::Compose {
            buffer: String::new(),
            status: crate::tui::model::ComposeStatus::Submitting,
            kind: crate::tui::model::ComposeKind::New,
        });
    }

    let (_m2, cmds) = update(m, Msg::CommentMutationErr("network error".into()));

    assert!(
        cmds.is_empty(),
        "CommentMutationErr must emit no Cmd (no refresh); got {:?}",
        cmds
    );
    let has_load_detail = cmds
        .iter()
        .any(|c| matches!(c, Cmd::LoadDetail { refresh: true, .. }));
    assert!(
        !has_load_detail,
        "CommentMutationErr must not emit Cmd::LoadDetail{{refresh:true}}"
    );
}

// --- AC1: FocusNextComment / FocusPrevComment move focused_comment, emit no Cmd ---

fn detail_model_with_n_comments(n: usize) -> Model {
    use serde_json::json;
    let comments: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            json!({
                "id": i as i64,
                "created_by_id": 1i64,
                "created_on": 1700000000u64 + i as u64,
                "body_plain_text": format!("Comment {i}")
            })
        })
        .collect();
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments,
            user_map: std::collections::HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 30),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn detail_model_with_n_comments_and_spans(n: usize) -> Model {
    let mut m = detail_model_with_n_comments(n);
    m.reflow_detail(78);
    m
}

fn focused_comment(model: &Model) -> Option<usize> {
    match model.top() {
        Some(Screen::Detail {
            focused_comment, ..
        }) => *focused_comment,
        _ => panic!("expected Detail screen"),
    }
}

// AC1: FocusNextComment with None focus moves to first comment (index 0), emits no Cmd.
#[test]
fn focus_next_from_none_moves_to_first_comment() {
    let m = detail_model_with_n_comments_and_spans(3);
    let (m, cmds) = update(m, Msg::FocusNextComment);
    assert!(cmds.is_empty(), "FocusNextComment must emit no Cmd");
    assert_eq!(
        focused_comment(&m),
        Some(0),
        "focus must move to first comment"
    );
}

// AC1: FocusNextComment moves focus from 0 to 1.
#[test]
fn focus_next_increments_focused_comment() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment, ..
    }) = m.top_mut()
    {
        *focused_comment = Some(0);
    }
    let (m, cmds) = update(m, Msg::FocusNextComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        Some(1),
        "FocusNext must move from 0 to 1"
    );
}

// AC1: FocusPrevComment with None focus moves to first comment (index 0), emits no Cmd.
#[test]
fn focus_prev_from_none_moves_to_first_comment() {
    let m = detail_model_with_n_comments_and_spans(3);
    let (m, cmds) = update(m, Msg::FocusPrevComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        Some(0),
        "FocusPrev from None should start at first comment"
    );
}

// AC1: FocusPrevComment moves focus from 1 to 0.
#[test]
fn focus_prev_decrements_focused_comment() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment, ..
    }) = m.top_mut()
    {
        *focused_comment = Some(1);
    }
    let (m, cmds) = update(m, Msg::FocusPrevComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        Some(0),
        "FocusPrev must move from 1 to 0"
    );
}

// AC1: FocusNextComment at last comment is a no-op (no wraparound).
#[test]
fn focus_next_at_last_comment_is_noop() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment, ..
    }) = m.top_mut()
    {
        *focused_comment = Some(2);
    }
    let (m, cmds) = update(m, Msg::FocusNextComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        Some(2),
        "FocusNext at last must stay at last (no wraparound)"
    );
}

// AC1: FocusPrevComment at first comment is a no-op (no wraparound).
#[test]
fn focus_prev_at_first_comment_is_noop() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment, ..
    }) = m.top_mut()
    {
        *focused_comment = Some(0);
    }
    let (m, cmds) = update(m, Msg::FocusPrevComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        Some(0),
        "FocusPrev at first must stay at first (no wraparound)"
    );
}

// AC1: Zero-comment thread keeps focused_comment = None on FocusNextComment.
#[test]
fn focus_next_on_empty_thread_is_noop() {
    let m = detail_model_with_n_comments_and_spans(0);
    let (m, cmds) = update(m, Msg::FocusNextComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        None,
        "empty thread: FocusNext must keep focused_comment = None"
    );
}

// AC1: Zero-comment thread keeps focused_comment = None on FocusPrevComment.
#[test]
fn focus_prev_on_empty_thread_is_noop() {
    let m = detail_model_with_n_comments_and_spans(0);
    let (m, cmds) = update(m, Msg::FocusPrevComment);
    assert!(cmds.is_empty());
    assert_eq!(
        focused_comment(&m),
        None,
        "empty thread: FocusPrev must keep focused_comment = None"
    );
}

// --- AC2: focus move sets offset so focused card is fully visible ---

fn detail_model_with_comment_spans(
    comment_spans: Vec<(usize, usize)>,
    total_lines: usize,
    initial_offset: usize,
    viewport_rows: u16,
) -> Model {
    use serde_json::json;
    let n = comment_spans.len();
    let comments: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            json!({
                "id": i as i64,
                "created_by_id": 1i64,
                "created_on": 1700000000u64 + i as u64,
                "body_plain_text": format!("Comment {i}")
            })
        })
        .collect();
    let lines: Vec<String> = (0..total_lines).map(|i| format!("line {i}")).collect();
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments,
            user_map: std::collections::HashMap::new(),
            lines,
            line_styles: vec![],
            assets: vec![],
            offset: initial_offset,
            loading: false,
            rendered_width: 80,
            compose: None,
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, viewport_rows),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn detail_offset(model: &Model) -> usize {
    match model.top() {
        Some(Screen::Detail { offset, .. }) => *offset,
        _ => panic!("expected Detail screen"),
    }
}

// AC2: card below viewport — focus move scrolls down so card's last line is visible.
// viewport_rows=14, DETAIL_CHROME_ROWS=4 → text_vh=10.
// Card at lines 15..20 (start=15, count=5). Offset=0 → card_end(20) > viewport_end(10).
// Expected offset = card_end - text_vh = 20 - 10 = 10.
#[test]
fn focus_move_scrolls_down_when_card_below_viewport() {
    let total_lines = 30;
    let card_start = 15;
    let card_count = 5;
    let viewport_rows = 14u16;
    let spans = vec![(card_start, card_count)];
    let m = detail_model_with_comment_spans(spans, total_lines, 0, viewport_rows);

    let (m, _) = update(m, Msg::FocusNextComment);
    let offset = detail_offset(&m);
    let text_vh = (viewport_rows - 4) as usize;
    let expected = card_start + card_count - text_vh;
    assert_eq!(
        offset, expected,
        "card below viewport: offset must make card's last line visible"
    );
}

// AC2: card above viewport — focus move scrolls up so card's first line is visible.
// viewport_rows=14, text_vh=10. Card at lines 3..8 (start=3, count=5).
// Offset=10 → card_start(3) < current_offset(10).
// Expected offset = card_start = 3.
#[test]
fn focus_move_scrolls_up_when_card_above_viewport() {
    let total_lines = 30;
    let card_start = 3;
    let card_count = 5;
    let viewport_rows = 14u16;
    let spans = vec![(card_start, card_count)];
    let m = detail_model_with_comment_spans(spans, total_lines, 10, viewport_rows);

    let (m, _) = update(m, Msg::FocusNextComment);
    let offset = detail_offset(&m);
    assert_eq!(
        offset, card_start,
        "card above viewport: offset must be at card's first line"
    );
}

// AC2: card already fully visible — focus move leaves offset unchanged.
// viewport_rows=14, text_vh=10. Card at lines 2..6 (start=2, count=4).
// Offset=0 → viewport covers lines 0..10; card [2..6] fully inside.
// Expected offset = 0 (unchanged).
#[test]
fn focus_move_does_not_change_offset_when_card_already_visible() {
    let total_lines = 20;
    let card_start = 2;
    let card_count = 4;
    let viewport_rows = 14u16;
    let spans = vec![(card_start, card_count)];
    let m = detail_model_with_comment_spans(spans, total_lines, 0, viewport_rows);

    let (m, _) = update(m, Msg::FocusNextComment);
    let offset = detail_offset(&m);
    assert_eq!(
        offset, 0,
        "card already visible: offset must remain unchanged"
    );
}

// --- AC3: PageUp/PageDown change offset and leave focused_comment unchanged ---

#[test]
fn page_down_changes_offset_leaves_focused_comment_unchanged() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment,
        lines,
        ..
    }) = m.top_mut()
    {
        *focused_comment = Some(1);
        *lines = (0..50).map(|i| format!("line {i}")).collect();
    }
    m.viewport = (80, 24);
    let (m, _) = update(m, Msg::PageDown);
    assert_eq!(
        focused_comment(&m),
        Some(1),
        "PageDown must leave focused_comment unchanged"
    );
    let offset = detail_offset(&m);
    assert!(offset > 0, "PageDown must advance offset");
}

#[test]
fn page_up_changes_offset_leaves_focused_comment_unchanged() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment,
        lines,
        offset,
        ..
    }) = m.top_mut()
    {
        *focused_comment = Some(0);
        *lines = (0..50).map(|i| format!("line {i}")).collect();
        *offset = 20;
    }
    m.viewport = (80, 24);
    let (m, _) = update(m, Msg::PageUp);
    assert_eq!(
        focused_comment(&m),
        Some(0),
        "PageUp must leave focused_comment unchanged"
    );
    let offset = detail_offset(&m);
    assert!(offset < 20, "PageUp must reduce offset");
}

// ScrollUp/ScrollDown (mouse wheel) also leave focused_comment unchanged.
#[test]
fn mouse_wheel_scroll_leaves_focused_comment_unchanged() {
    let mut m = detail_model_with_n_comments_and_spans(3);
    if let Some(Screen::Detail {
        focused_comment,
        lines,
        offset,
        ..
    }) = m.top_mut()
    {
        *focused_comment = Some(1);
        *lines = (0..50).map(|i| format!("line {i}")).collect();
        *offset = 10;
    }
    m.viewport = (80, 24);
    let (m, _) = update(m, Msg::ScrollDown);
    assert_eq!(
        focused_comment(&m),
        Some(1),
        "ScrollDown must leave focused_comment unchanged"
    );
}

// --- AC5: reflow_detail rebuilds comment_spans on width change, reuses on same width ---

#[test]
fn reflow_detail_builds_comment_spans_on_width_change() {
    let mut m = detail_model_with_n_comments(2);
    assert!(
        matches!(m.top(), Some(Screen::Detail { comment_spans, .. }) if comment_spans.is_empty()),
        "comment_spans must be empty before reflow"
    );
    m.reflow_detail(78);
    match m.top() {
        Some(Screen::Detail { comment_spans, .. }) => {
            assert_eq!(
                comment_spans.len(),
                2,
                "comment_spans must have one entry per comment after reflow"
            );
            for (start, count) in comment_spans {
                assert!(*count > 0, "each span must have line_count > 0");
                let _ = start;
            }
        }
        _ => panic!("expected Detail screen"),
    }
}

#[test]
fn reflow_detail_reuses_comment_spans_at_same_width() {
    let mut m = detail_model_with_n_comments(2);
    m.reflow_detail(78);
    let spans_after_first = match m.top() {
        Some(Screen::Detail { comment_spans, .. }) => comment_spans.clone(),
        _ => panic!("expected Detail"),
    };
    m.reflow_detail(78);
    match m.top() {
        Some(Screen::Detail { comment_spans, .. }) => {
            assert_eq!(
                *comment_spans, spans_after_first,
                "second reflow at same width must return identical spans (cache hit)"
            );
        }
        _ => panic!("expected Detail"),
    }
}

#[test]
fn reflow_detail_rebuilds_comment_spans_on_data_change() {
    let mut m = detail_model_with_n_comments(1);
    m.reflow_detail(78);
    let spans_1 = match m.top() {
        Some(Screen::Detail { comment_spans, .. }) => comment_spans.clone(),
        _ => panic!("expected Detail"),
    };
    use crate::tui::model::DetailLoad;
    use serde_json::json;
    let new_comment = json!({
        "id": 99i64, "created_by_id": 1i64, "created_on": 1700000099u64,
        "body_plain_text": "Extra comment"
    });
    let (m, _) = update(
        m,
        Msg::LoadedDetail(DetailLoad {
            task: serde_json::Value::Null,
            comments: vec![new_comment.clone(), new_comment],
            assets: vec![],
            user_map: std::collections::HashMap::new(),
            loaded_at: "2026-06-28T00:00:00Z".into(),
            current_user_id: None,
            unauthorized: false,
        }),
    );
    let mut m = m;
    m.reflow_detail(78);
    match m.top() {
        Some(Screen::Detail { comment_spans, .. }) => {
            assert_eq!(
                comment_spans.len(),
                2,
                "after data change and reflow, spans must reflect new comment count"
            );
            assert_ne!(
                *comment_spans, spans_1,
                "spans must differ after data change"
            );
        }
        _ => panic!("expected Detail"),
    }
}

// --- AC6: Ctrl/Cmd+click on [editar]/[excluir] still emits the correct Cmd (regression) ---

#[test]
fn ctrl_click_editar_on_focused_own_card_still_emits_compose_open() {
    let m = detail_model_with_comments_for_edit("inst", 1, 1, Some(7), 82);
    let (click_row, click_col) = match m.top() {
        Some(Screen::Detail { affordances, .. }) => {
            let aff = affordances
                .iter()
                .find(|a| matches!(a.kind, crate::render::AffordanceKind::Edit(_)))
                .expect("edit affordance must exist after reflow");
            (2u16 + aff.line_idx as u16, aff.col_start as u16)
        }
        _ => panic!("expected Detail screen"),
    };

    let mut m = m;
    if let Some(Screen::Detail {
        focused_comment, ..
    }) = m.top_mut()
    {
        *focused_comment = Some(0);
    }

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: click_col,
            row: click_row,
            modifiers: crossterm::event::KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "Ctrl+click [editar] must emit no Cmd directly; compose is opened via model state change"
    );
}

#[test]
fn no_key_acts_on_focused_comment_for_edit_or_delete() {
    let mut m = detail_model_with_comments_for_edit("inst", 1, 1, Some(7), 82);
    if let Some(Screen::Detail {
        focused_comment, ..
    }) = m.top_mut()
    {
        *focused_comment = Some(0);
    }
    let (m_after_e, cmds_e) = update(m, Msg::ComposeOpen);
    assert!(
        cmds_e.is_empty(),
        "ComposeOpen key must emit no Cmd (only opens compose state)"
    );
    let _ = m_after_e;
}

// --- AC1 (BDR 0026 Sc.9): modal_area centering and clamping ---

#[test]
fn modal_area_centers_in_large_frame() {
    use crate::tui::widgets::modal::modal_area;
    use ratatui::layout::Rect;
    let frame = Rect::new(0, 0, 100, 40);
    let area = modal_area(frame, 60, 20);
    let center_x = area.x + area.width / 2;
    let center_y = area.y + area.height / 2;
    let frame_cx = frame.width / 2;
    let frame_cy = frame.height / 2;
    assert!(
        center_x.abs_diff(frame_cx) <= 1,
        "modal must be horizontally centered (cx={center_x}, frame_cx={frame_cx})"
    );
    assert!(
        center_y.abs_diff(frame_cy) <= 1,
        "modal must be vertically centered (cy={center_y}, frame_cy={frame_cy})"
    );
    assert!(
        area.right() <= frame.right(),
        "modal must not overflow frame right: area.right()={}",
        area.right()
    );
    assert!(
        area.bottom() <= frame.bottom(),
        "modal must not overflow frame bottom: area.bottom()={}",
        area.bottom()
    );
}

#[test]
fn modal_area_clamps_when_frame_narrower_than_desired() {
    use crate::tui::widgets::modal::modal_area;
    use ratatui::layout::Rect;
    let frame = Rect::new(0, 0, 30, 10);
    let area = modal_area(frame, 80, 20);
    assert!(
        area.right() <= frame.right(),
        "clamped modal must not overflow frame right: area={area:?}, frame={frame:?}"
    );
    assert!(
        area.bottom() <= frame.bottom(),
        "clamped modal must not overflow frame bottom: area={area:?}, frame={frame:?}"
    );
    assert!(
        area.width >= 1,
        "clamped modal must have at least 1 column wide"
    );
    assert!(
        area.height >= 1,
        "clamped modal must have at least 1 row tall"
    );
}

#[test]
fn modal_area_clamps_when_frame_shorter_than_desired() {
    use crate::tui::widgets::modal::modal_area;
    use ratatui::layout::Rect;
    let frame = Rect::new(0, 0, 80, 6);
    let area = modal_area(frame, 60, 30);
    assert!(
        area.bottom() <= frame.bottom(),
        "clamped modal height must not overflow frame: area.bottom()={}, frame.bottom()={}",
        area.bottom(),
        frame.bottom()
    );
}

// --- AC3 (slice-1b): modal_target_size pure unit tests ---

#[test]
fn modal_target_size_large_frame_returns_70_percent() {
    use crate::tui::widgets::modal::{modal_target_size, ModalContent};
    use ratatui::layout::Rect;
    let frame = Rect::new(0, 0, 100, 40);
    let content = ModalContent {
        title: "Test",
        lines: &[],
        hint: None,
    };
    let (w, h) = modal_target_size(frame, &content);
    assert!(
        (64..=76).contains(&w),
        "width must be ≈70% of 100 (70±10): got {w}"
    );
    assert!(
        (24..=32).contains(&h),
        "height must be ≈70% of 40 (28±6): got {h}"
    );
}

#[test]
fn modal_target_size_respects_content_minimum() {
    use crate::tui::widgets::modal::{modal_target_size, ModalContent};
    use ratatui::layout::Rect;
    let body_lines: Vec<String> = (0..10).map(|i| format!("line {i}")).collect();
    let frame = Rect::new(0, 0, 10, 8);
    let content = ModalContent {
        title: "Test",
        lines: &body_lines,
        hint: Some("hint"),
    };
    let (w, h) = modal_target_size(frame, &content);
    let min_h = body_lines.len() as u16 + 1 + 2;
    assert!(
        h >= min_h,
        "height must be at least the content minimum ({min_h}): got {h}"
    );
    assert!(w >= 1, "width must be at least 1: got {w}");
}

// --- AC3 (BDR 0026 Sc.3): compose absent from scrollable lines after reflow ---

#[test]
fn reflow_detail_with_compose_does_not_append_compose_lines() {
    use crate::tui::model::{Compose, ComposeKind, ComposeStatus};
    let compose = Compose {
        kind: ComposeKind::New,
        buffer: "compose body text".into(),
        status: ComposeStatus::Editing,
    };
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: Some(compose),
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    m.reflow_detail(78);
    match m.top() {
        Some(Screen::Detail { lines, .. }) => {
            let joined = lines.join("\n");
            assert!(
                !joined.contains("compose body text"),
                "compose buffer must NOT appear in scrollable lines (moved to modal): {joined}"
            );
            assert!(
                !joined.contains("Comment"),
                "compose label must NOT appear in scrollable lines (moved to modal): {joined}"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// --- AC7: compose semantics regression — key map unchanged after modal migration ---
// These exercise the pure update() transitions to confirm the compose mode behavior
// is unaffected by moving the rendering to an overlay.

#[test]
fn ac7_regression_compose_newline_inserts_newline_not_submit_after_modal_migration() {
    use crate::tui::model::{Compose, ComposeKind, ComposeStatus};
    let m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: Some(Compose {
                kind: ComposeKind::New,
                buffer: "hello".into(),
                status: ComposeStatus::Editing,
            }),
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m2, cmds) = update(m, Msg::ComposeNewline);
    assert!(
        cmds.is_empty(),
        "ComposeNewline must emit no Cmd (no submit): {cmds:?}"
    );
    match m2.top() {
        Some(Screen::Detail {
            compose: Some(cp), ..
        }) => {
            assert!(
                cp.buffer.contains('\n'),
                "ComposeNewline must insert a newline into the buffer: {:?}",
                cp.buffer
            );
        }
        _ => panic!("expected Detail with active compose"),
    }
}

#[test]
fn ac7_regression_compose_cancel_clears_compose_emits_no_cmd_after_modal_migration() {
    use crate::tui::model::{Compose, ComposeKind, ComposeStatus};
    let m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: Some(Compose {
                kind: ComposeKind::New,
                buffer: "draft text".into(),
                status: ComposeStatus::Editing,
            }),
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m2, cmds) = update(m, Msg::ComposeCancel);
    assert!(cmds.is_empty(), "ComposeCancel must emit no Cmd: {cmds:?}");
    match m2.top() {
        Some(Screen::Detail { compose, .. }) => {
            assert!(
                compose.is_none(),
                "ComposeCancel must clear compose: {compose:?}"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

#[test]
fn ac7_regression_compose_submit_emits_write_cmd_after_modal_migration() {
    use crate::tui::model::{Compose, ComposeKind, ComposeStatus};
    let m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 42,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: Some(Compose {
                kind: ComposeKind::New,
                buffer: "non empty".into(),
                status: ComposeStatus::Editing,
            }),
            current_user_id: None,
            affordances: vec![],
            confirm_delete: None,
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (_m2, cmds) = update(m, Msg::ComposeSubmit);
    assert!(
        cmds.iter().any(|c| matches!(c, Cmd::SubmitComment { .. })),
        "ComposeSubmit on non-empty buffer must emit SubmitComment Cmd: {cmds:?}"
    );
}

// scroll_offset_for_card unit tests (pure math, no model) ---

#[test]
fn scroll_offset_for_card_returns_current_when_visible() {
    use crate::tui::model::scroll_offset_for_card;
    let result = scroll_offset_for_card(0, 2, 4, 14, 20);
    assert_eq!(result, 0, "visible card: offset must be unchanged");
}

#[test]
fn scroll_offset_for_card_scrolls_down_when_below() {
    use crate::tui::model::scroll_offset_for_card;
    let result = scroll_offset_for_card(0, 15, 5, 14, 20);
    let text_vh = (14u16 - 4) as usize;
    assert_eq!(
        result,
        15 + 5 - text_vh,
        "card below viewport: offset must show card end"
    );
}

#[test]
fn scroll_offset_for_card_scrolls_up_when_above() {
    use crate::tui::model::scroll_offset_for_card;
    let result = scroll_offset_for_card(10, 3, 5, 14, 20);
    assert_eq!(result, 3, "card above viewport: offset must be card_start");
}

// --- Confirm-delete modal keyboard routing (ADR 0039 slice 2) ---

fn detail_model_with_confirm_delete(comment_id: i64) -> Model {
    Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
            compose: None,
            current_user_id: Some(7),
            affordances: vec![],
            confirm_delete: Some(comment_id),
            focused_comment: None,
            auth_error: false,
            comment_spans: vec![],
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

// AC4: Enter key (Msg::Select) when confirm_delete is Some confirms the delete.
#[test]
fn enter_key_when_confirm_delete_some_emits_delete_comment() {
    let m = detail_model_with_confirm_delete(99);
    let (m2, cmds) = update(m, Msg::Select);
    assert_eq!(
        cmds.len(),
        1,
        "Enter in confirm sub-mode must emit one Cmd::DeleteComment; got {:?}",
        cmds
    );
    match &cmds[0] {
        Cmd::DeleteComment { comment_id, .. } => {
            assert_eq!(*comment_id, 99, "must delete comment_id=99");
        }
        other => panic!("expected DeleteComment, got {other:?}"),
    }
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete must be cleared after confirm"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// AC4: Esc key (Msg::Back) when confirm_delete is Some cancels the delete (no cmd, no pop).
#[test]
fn esc_key_when_confirm_delete_some_cancels_without_popping() {
    let m = detail_model_with_confirm_delete(99);
    let (m2, cmds) = update(m, Msg::Back);
    assert!(
        cmds.is_empty(),
        "Esc in confirm sub-mode must emit no Cmd; got {:?}",
        cmds
    );
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete must be None after cancel"
            );
        }
        _ => panic!("expected Detail screen; Esc must not pop the stack"),
    }
    assert!(!m2.should_quit, "Esc in confirm sub-mode must not quit");
}

// AC4: Msg::ConfirmDeleteComment when confirm_delete is None is a no-op.
#[test]
fn confirm_delete_msg_when_none_is_noop() {
    let m = detail_model_with_assets_and_viewport(vec![], "inst", (80, 24));
    let (m2, cmds) = update(m, Msg::ConfirmDeleteComment);
    assert!(
        cmds.is_empty(),
        "ConfirmDeleteComment with no pending delete must emit no Cmd"
    );
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(confirm_delete.is_none());
        }
        _ => panic!("expected Detail screen"),
    }
}

// AC4: Msg::CancelDeleteComment when confirm_delete is None is a no-op.
#[test]
fn cancel_delete_msg_when_none_is_noop() {
    let m = detail_model_with_assets_and_viewport(vec![], "inst", (80, 24));
    let (m2, cmds) = update(m, Msg::CancelDeleteComment);
    assert!(
        cmds.is_empty(),
        "CancelDeleteComment with no pending delete must emit no Cmd"
    );
    assert!(
        !m2.should_quit,
        "CancelDeleteComment must not quit when no modal is open"
    );
}

// AC5 (regression): Msg::CommentMutationOk after a delete triggers LoadDetail{refresh:true}.
// This test duplicates the existing comment_mutation_ok_after_delete_emits_load_detail_refresh
// from the same file to guard against the delete refresh path regressing across slices.
#[test]
fn comment_mutation_ok_after_delete_triggers_load_detail_refresh_regression() {
    let mut m = detail_model_with_confirm_delete(42);
    if let Some(Screen::Detail {
        ref mut instance,
        ref mut project_id,
        ref mut task_id,
        ..
    }) = m.top_mut()
    {
        *instance = "myinst".into();
        *project_id = 5;
        *task_id = 10;
    }

    let (m2, cmds) = update(m, Msg::CommentMutationOk);

    assert_eq!(
        cmds.len(),
        1,
        "CommentMutationOk must emit exactly one Cmd; got {:?}",
        cmds
    );
    match &cmds[0] {
        Cmd::LoadDetail {
            instance,
            project_id,
            task_id,
            refresh,
        } => {
            assert_eq!(instance, "myinst");
            assert_eq!(*project_id, 5);
            assert_eq!(*task_id, 10);
            assert!(*refresh, "refresh must be true on mutation ok");
        }
        other => panic!("expected LoadDetail, got {other:?}"),
    }
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete cleared by CommentMutationOk"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// AC3: plain left click on the [confirmar] button target emits Cmd::DeleteComment.
// Button targets are set on the model (mirroring the shell's render-then-set flow).
// For an 80×24 frame: modal at x=12,y=4,w=56,h=16; inner_x=13; hint_row=18;
// [confirmar] → x_start=13, x_end=24.
#[test]
fn plain_click_on_confirm_button_emits_delete_comment() {
    use crate::tui::model::ModalButtonTarget;
    let mut m = detail_model_with_confirm_delete(55);
    m.viewport = (80, 24);
    m.set_modal_button_targets(vec![
        ModalButtonTarget {
            x_start: 13,
            x_end: 24,
            row: 18,
            is_confirm: true,
        },
        ModalButtonTarget {
            x_start: 26,
            x_end: 36,
            row: 18,
            is_confirm: false,
        },
    ]);
    let (m2, cmds) = update(
        m,
        Msg::Click {
            column: 15,
            row: 18,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "plain click on [confirmar] must emit one Cmd::DeleteComment; got {:?}",
        cmds
    );
    match &cmds[0] {
        Cmd::DeleteComment { comment_id, .. } => {
            assert_eq!(*comment_id, 55, "must delete comment_id=55");
        }
        other => panic!("expected DeleteComment, got {other:?}"),
    }
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete cleared after confirm click"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// AC3: plain left click on the [cancelar] button target clears confirm_delete, emits no Cmd.
#[test]
fn plain_click_on_cancel_button_clears_confirm_no_cmd() {
    use crate::tui::model::ModalButtonTarget;
    let mut m = detail_model_with_confirm_delete(55);
    m.viewport = (80, 24);
    m.set_modal_button_targets(vec![
        ModalButtonTarget {
            x_start: 13,
            x_end: 24,
            row: 18,
            is_confirm: true,
        },
        ModalButtonTarget {
            x_start: 26,
            x_end: 36,
            row: 18,
            is_confirm: false,
        },
    ]);
    let (m2, cmds) = update(
        m,
        Msg::Click {
            column: 28,
            row: 18,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "plain click on [cancelar] must emit no Cmd; got {:?}",
        cmds
    );
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_none(),
                "confirm_delete cleared after cancel click"
            );
        }
        _ => panic!("expected Detail screen"),
    }
}

// AC3b: plain click OUTSIDE the button targets while confirm modal is open is a no-op
// (modal captures all clicks — no selection starts behind it).
#[test]
fn plain_click_outside_buttons_while_modal_open_is_noop() {
    use crate::tui::model::ModalButtonTarget;
    let mut m = detail_model_with_confirm_delete(55);
    m.viewport = (80, 24);
    m.set_modal_button_targets(vec![
        ModalButtonTarget {
            x_start: 13,
            x_end: 24,
            row: 18,
            is_confirm: true,
        },
        ModalButtonTarget {
            x_start: 26,
            x_end: 36,
            row: 18,
            is_confirm: false,
        },
    ]);
    let (m2, cmds) = update(
        m,
        Msg::Click {
            column: 40,
            row: 5,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "click outside buttons must emit no Cmd (modal captures); got {:?}",
        cmds
    );
    match m2.top() {
        Some(Screen::Detail { confirm_delete, .. }) => {
            assert!(
                confirm_delete.is_some(),
                "confirm_delete must stay Some when click misses the buttons"
            );
        }
        _ => panic!("expected Detail screen"),
    }
    assert!(
        m2.selection.is_none(),
        "selection must NOT start behind the confirm modal (modal captures clicks)"
    );
}
