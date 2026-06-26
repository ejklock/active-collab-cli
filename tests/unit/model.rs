use super::*;
use crate::render::Asset;
use crate::tui::screens::detail::{detail_asset_panel_rect, draw_detail, DetailParams};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
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
    pending_download: bool,
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
            body_links: vec![],
            assets,
            offset: 0,
            loading: false,
            pending_download,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
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
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
    };
    let (m, cmds) = update(m, Msg::Click { column: 10, row: 5 });
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
}

// V2a-A1: Click on the first asset row opens assets[0] via OpenAsset.
// Viewport 80x24, 2 assets → panel_height = min(2+2, 8) = 4.
// first_asset_row = panel.y + 1 (one row inside the top border).
#[test]
fn click_first_asset_row_emits_open_asset_cmd() {
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets, "inst", (80, 24), false);

    let area = Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 2).expect("panel must exist");
    let first_asset_row = panel.y + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: first_asset_row,
        },
    );
    assert_eq!(cmds.len(), 1, "must emit exactly one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { instance, url } => {
            assert_eq!(instance, "inst");
            assert_eq!(url, "https://example.com/a.pdf");
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

// V2a-A1: Click on the last asset row opens the last asset (no off-by-one at bottom boundary).
#[test]
fn click_last_asset_row_opens_last_asset() {
    let assets = vec![
        make_asset("first.pdf", "https://example.com/first.pdf"),
        make_asset("second.pdf", "https://example.com/second.pdf"),
        make_asset("third.pdf", "https://example.com/third.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets, "inst", (80, 30), false);

    let area = Rect::new(0, 0, 80, 30);
    let panel = detail_asset_panel_rect(area, 3).expect("panel must exist");
    let last_asset_row = panel.y + panel.height - 2;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: last_asset_row,
        },
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(url, "https://example.com/third.pdf");
        }
        other => panic!("expected OpenAsset for last asset, got {other:?}"),
    }
}

// V2a-A1: Click on the panel top border row (panel.y) is a no-op.
#[test]
fn click_on_panel_top_border_row_is_noop() {
    let assets = vec![make_asset("x.pdf", "https://example.com/x.pdf")];
    let m = detail_model_with_assets_and_viewport(assets, "inst", (80, 24), false);

    let area = Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 1).expect("panel must exist");
    let border_row = panel.y;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: border_row,
        },
    );
    assert!(cmds.is_empty(), "click on top border must be a no-op");
}

// V2a-A1: Click on the panel bottom border row (panel.y + height - 1) is a no-op.
#[test]
fn click_on_panel_bottom_border_row_is_noop() {
    let assets = vec![make_asset("x.pdf", "https://example.com/x.pdf")];
    let m = detail_model_with_assets_and_viewport(assets, "inst", (80, 24), false);

    let area = Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 1).expect("panel must exist");
    let bottom_border = panel.y + panel.height - 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: bottom_border,
        },
    );
    assert!(cmds.is_empty(), "click on bottom border must be a no-op");
}

// V2a-A1: Click above the panel (in the content area) is a no-op.
#[test]
fn click_above_panel_is_noop() {
    let assets = vec![make_asset("y.pdf", "https://example.com/y.pdf")];
    let m = detail_model_with_assets_and_viewport(assets, "inst", (80, 24), false);

    let area = Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 1).expect("panel must exist");
    let above_row = panel.y.saturating_sub(1);

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: above_row,
        },
    );
    assert!(cmds.is_empty(), "click above panel must be a no-op");
}

// V2a-A1: With pending_download set, clicking an asset row emits DownloadAsset.
#[test]
fn click_asset_row_with_pending_download_emits_download_cmd() {
    let assets = vec![make_asset("report.pdf", "https://example.com/report.pdf")];
    let m = detail_model_with_assets_and_viewport(assets, "acme", (80, 24), true);

    let area = Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 1).expect("panel must exist");
    let asset_row = panel.y + 1;

    let (m_after, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: asset_row,
        },
    );
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::DownloadAsset {
            instance,
            url,
            name,
        } => {
            assert_eq!(instance, "acme");
            assert_eq!(url, "https://example.com/report.pdf");
            assert_eq!(name, "report.pdf");
        }
        other => panic!("expected DownloadAsset, got {other:?}"),
    }
    match m_after.top() {
        Some(Screen::Detail {
            pending_download, ..
        }) => {
            assert!(
                !pending_download,
                "pending_download must be cleared after click-open"
            );
        }
        other => panic!("expected Detail screen, got {other:?}"),
    }
}

// V2a-A1: After a click-open, pending_download is cleared (matches AssetOpen shortcut behavior).
#[test]
fn click_asset_clears_pending_download_flag() {
    let assets = vec![make_asset("z.pdf", "https://example.com/z.pdf")];
    let m = detail_model_with_assets_and_viewport(assets, "inst", (80, 24), true);

    let area = Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 1).expect("panel must exist");
    let asset_row = panel.y + 1;

    let (m_after, _cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: asset_row,
        },
    );
    match m_after.top() {
        Some(Screen::Detail {
            pending_download, ..
        }) => {
            assert!(
                !pending_download,
                "pending_download must be false after open"
            );
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

// V2a-A1: Detail with no assets — any click is a no-op (no panel exists).
#[test]
fn click_detail_with_no_assets_is_noop() {
    let m = detail_model_with_assets_and_viewport(vec![], "inst", (80, 24), false);
    let (_m, cmds) = update(m, Msg::Click { column: 5, row: 20 });
    assert!(
        cmds.is_empty(),
        "click on detail with no assets must be a no-op"
    );
}

// V2a-A3: detail_asset_panel_rect returns None for zero assets.
#[test]
fn detail_asset_panel_rect_none_for_zero_assets() {
    let area = Rect::new(0, 0, 80, 24);
    assert!(
        detail_asset_panel_rect(area, 0).is_none(),
        "panel rect must be None when assets_len is 0"
    );
}

// V2a-A3: The shared geometry function produces correct height and position for
// several viewport sizes and asset counts — the renderer and click mapper agree
// because they both call this same function.
#[test]
fn detail_asset_panel_rect_consistent_geometry_across_viewport_sizes() {
    for (viewport_w, viewport_h, assets_len) in [
        (80u16, 24u16, 1usize),
        (80, 24, 3),
        (80, 24, 6),
        (120, 40, 2),
        (40, 20, 5),
    ] {
        let area = Rect::new(0, 0, viewport_w, viewport_h);
        let panel = detail_asset_panel_rect(area, assets_len)
            .unwrap_or_else(|| panic!("panel must exist for assets_len={assets_len}"));

        let expected_height = (assets_len as u16 + 2).min(8);
        assert_eq!(
            panel.height, expected_height,
            "panel_height must be min(assets_len+2, 8) for \
             assets_len={assets_len}, viewport=({viewport_w}x{viewport_h})"
        );
        assert_eq!(
            panel.y + panel.height,
            viewport_h,
            "panel bottom must align with viewport bottom for \
             viewport=({viewport_w}x{viewport_h}), assets_len={assets_len}"
        );
        assert_eq!(panel.x, 0, "panel must start at column 0");
        assert_eq!(panel.width, viewport_w, "panel must span full width");
    }
}

// V2a-A3: The click mapper uses detail_asset_panel_rect so geometry cannot diverge.
// Clicking the first asset row computed from the shared fn opens the first asset,
// verified across multiple viewport sizes.
#[test]
fn click_mapper_agrees_with_panel_rect_for_multiple_viewport_sizes() {
    for (viewport_w, viewport_h) in [(80u16, 24u16), (120, 40), (40, 20)] {
        let assets = vec![
            make_asset("doc1.pdf", "https://example.com/doc1.pdf"),
            make_asset("doc2.pdf", "https://example.com/doc2.pdf"),
        ];
        let m =
            detail_model_with_assets_and_viewport(assets, "inst", (viewport_w, viewport_h), false);

        let area = Rect::new(0, 0, viewport_w, viewport_h);
        let panel = detail_asset_panel_rect(area, 2).expect("panel must exist");
        let first_row = panel.y + 1;

        let (_m, cmds) = update(
            m,
            Msg::Click {
                column: 0,
                row: first_row,
            },
        );
        assert_eq!(
            cmds.len(),
            1,
            "click on first asset row must emit one cmd at \
             viewport ({viewport_w}x{viewport_h})"
        );
        match &cmds[0] {
            Cmd::OpenAsset { url, .. } => {
                assert_eq!(
                    url, "https://example.com/doc1.pdf",
                    "first asset row must open first asset at \
                     viewport ({viewport_w}x{viewport_h})"
                );
            }
            other => panic!("expected OpenAsset, got {other:?}"),
        }
    }
}

// V2a-A3: Renderer/mapper geometry agreement — draw_detail places the Artifacts panel
// at exactly the Rect returned by detail_asset_panel_rect. The test renders via
// TestBackend and checks that the "Artifacts" border title appears on the row
// panel_rect.y. It fails if draw_detail uses any other formula.
#[test]
fn draw_detail_panel_rows_match_detail_asset_panel_rect_for_multiple_viewports() {
    for (viewport_w, viewport_h, assets_len) in [(80u16, 24u16, 2usize), (120, 40, 3), (40, 20, 1)]
    {
        let assets: Vec<Asset> = (0..assets_len)
            .map(|i| make_asset(&format!("file{i}.pdf"), &format!("https://x.com/f{i}.pdf")))
            .collect();

        let area = Rect::new(0, 0, viewport_w, viewport_h);
        let panel_rect = detail_asset_panel_rect(area, assets_len)
            .unwrap_or_else(|| panic!("panel must exist for assets_len={assets_len}"));

        let backend = TestBackend::new(viewport_w, viewport_h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw_detail(
                    frame,
                    area,
                    DetailParams {
                        lines: &["body".to_string()],
                        line_styles: &[],
                        assets: &assets,
                        offset: 0,
                        loading: false,
                        task_id: 1,
                        task_name: "T",
                    },
                );
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let panel_top_row: String = (0..viewport_w)
            .map(|x| buf.cell((x, panel_rect.y)).unwrap().symbol().to_string())
            .collect();

        assert!(
            panel_top_row.contains("Artifacts"),
            "Artifacts panel top border must appear at row {} (panel_rect.y) for \
             viewport=({viewport_w}x{viewport_h}), assets_len={assets_len}. \
             Renderer and shared fn geometry disagree if this fails. \
             row={panel_top_row:?}",
            panel_rect.y
        );
    }
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
            body_links: vec![],
            assets,
            offset: 0,
            loading: false,
            pending_download: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
    }
}

// V5-A1: detail_max_offset — no assets, viewport_rows=24, lines_len=50.
// chrome=4, text_vh=24-4=20, max=50-20=30.
#[test]
fn detail_max_offset_no_assets_viewport_24_lines_50() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(24, 50, 0);
    assert_eq!(max, 30, "viewport=24, no assets: text_vh=20, max=50-20=30");
}

// V5-A1: detail_max_offset — 2 assets (panel_h=4), viewport_rows=24, lines_len=50.
// text_vh=24-4-4=16, max=50-16=34.
#[test]
fn detail_max_offset_with_assets_shrinks_text_viewport() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(24, 50, 2);
    assert_eq!(
        max, 34,
        "viewport=24, 2 assets (panel_h=4): text_vh=16, max=50-16=34"
    );
}

// V5-A1: detail_max_offset — 6 assets (panel_h=8, capped), viewport_rows=24, lines_len=50.
// text_vh=24-4-8=12, max=50-12=38.
#[test]
fn detail_max_offset_many_assets_caps_panel_height() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(24, 50, 6);
    assert_eq!(
        max, 38,
        "viewport=24, 6 assets (panel_h=8 capped): text_vh=12, max=50-12=38"
    );
}

// V5-A1: detail_max_offset — tiny viewport (rows < chrome): text_vh clamps to 1,
// so max = lines_len - 1 (not lines_len), the same bound as a 1-row visible area.
#[test]
fn detail_max_offset_tiny_viewport_clamps_text_vh_to_one() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(2, 30, 0);
    assert_eq!(
        max, 29,
        "viewport=2 < chrome(4): raw text_vh=0, clamped to 1, max=30-1=29"
    );
}

// V5-A1: detail_max_offset — zero viewport: text_vh clamps to 1, max = lines_len - 1.
#[test]
fn detail_max_offset_zero_viewport_clamps_text_vh_to_one() {
    use crate::tui::model::detail_max_offset;
    let max = detail_max_offset(0, 20, 0);
    assert_eq!(
        max, 19,
        "viewport=0: raw text_vh=0, clamped to 1, max=20-1=19"
    );
}

// V5-A1: handle_down clamps to detail_max_offset, not lines.len()-1.
// viewport=24, 50 lines, no assets → max=30. Scroll 60 times → offset stays at 30.
#[test]
fn handle_down_clamps_to_detail_max_offset_no_assets() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    for _ in 0..60 {
        model = update(model, Msg::Down).0;
    }

    let expected_max = detail_max_offset(24, 50, 0);
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

// V5-A1: handle_down is idempotent at max — pressing Down when already at max does nothing.
#[test]
fn handle_down_idempotent_at_max() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    let max = detail_max_offset(24, 50, 0);
    for _ in 0..100 {
        model = update(model, Msg::Down).0;
    }

    let offset_at_max = match model.top() {
        Some(Screen::Detail { offset, .. }) => *offset,
        other => panic!("expected Detail, got {other:?}"),
    };
    assert_eq!(offset_at_max, max, "offset must be clamped to max={max}");

    model = update(model, Msg::Down).0;
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(*offset, max, "offset must stay at max after another Down");
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

// V5-A1: handle_down clamps to a tighter max when assets are present.
// viewport=24, 50 lines, 2 assets (panel_h=4) → max=34.
#[test]
fn handle_down_clamps_to_detail_max_offset_with_assets() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let mut model = detail_model_scrollable(lines.clone(), assets, (80, 24));

    for _ in 0..100 {
        model = update(model, Msg::Down).0;
    }

    let expected_max = detail_max_offset(24, 50, 2);
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, expected_max,
                "offset with 2 assets must clamp to {expected_max}"
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

    let expected_max = detail_max_offset(24, 50, 0);
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

    let max = detail_max_offset(24, 50, 0);
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
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
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

    let (m, cmds) = update(m, Msg::Click { column: 10, row: 4 });
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

// --- V4b body-link click tests ---

/// Build a Detail model with specific lines, body_links, assets, offset and viewport.
fn detail_model_with_links(
    lines: Vec<String>,
    body_links: Vec<String>,
    assets: Vec<Asset>,
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
            body_links,
            assets,
            offset,
            loading: false,
            pending_download: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
    }
}

// V4b-A1: A left click on a "↗ Link N" label in the content area emits OpenAsset
// carrying body_links[N-1] and the correct instance.
// Viewport 80x24, no assets. text_top=2, content_text_height=24-4=20.
// The label line is at logical line 0, which maps to row text_top = 2.
// The label "↗ Link 1" starts at display col 2 (border col 0, padding col 1).
#[test]
fn click_body_link_label_emits_open_asset_with_correct_url() {
    let url = "https://example.com/doc.pdf".to_string();
    let label_line = "\u{2502} \u{2197} Link 1            \u{2502}".to_string();
    let m = detail_model_with_links(vec![label_line], vec![url.clone()], vec![], 0, (80, 24));

    let (_m, cmds) = update(m, Msg::Click { column: 3, row: 2 });
    assert_eq!(cmds.len(), 1, "must emit exactly one cmd");
    match &cmds[0] {
        Cmd::OpenAsset {
            instance,
            url: cmd_url,
        } => {
            assert_eq!(instance, "inst");
            assert_eq!(cmd_url, &url);
        }
        other => panic!("expected OpenAsset for body link, got {other:?}"),
    }
}

// V4b-A1: Scroll offset is accounted for: with offset=1, row text_top=2 maps to
// logical_line = 1 + (2 - 2) = 1. A click at row 2 must open body_links[0]
// from line 1 (the label line), not line 0.
#[test]
fn click_body_link_accounts_for_scroll_offset() {
    let url = "https://example.com/offset-test".to_string();
    let plain_line = "\u{2502} plain text \u{2502}".to_string();
    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![plain_line, label_line],
        vec![url.clone()],
        vec![],
        1,
        (80, 24),
    );

    let (_m, cmds) = update(m, Msg::Click { column: 3, row: 2 });
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(cmd_url, &url);
        }
        other => panic!("expected OpenAsset for offset body link, got {other:?}"),
    }
}

// V4b-A3: A click on a non-label content cell (border or plain text) is a no-op.
#[test]
fn click_non_label_content_cell_is_noop() {
    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["https://example.com/doc".to_string()],
        vec![],
        0,
        (80, 24),
    );

    // Column 0 is the "│" border — no label there.
    let (_m, cmds) = update(m, Msg::Click { column: 0, row: 2 });
    assert!(cmds.is_empty(), "click on border must be a no-op");
}

// V4b-A3: A label whose number exceeds body_links length is a safe no-op.
#[test]
fn click_body_link_number_beyond_links_length_is_safe_noop() {
    // Line contains "↗ Link 2" but body_links has only 1 entry (index 0 = Link 1).
    let label_line = "\u{2502} \u{2197} Link 2 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["https://example.com/only-one".to_string()],
        vec![],
        0,
        (80, 24),
    );

    let (_m, cmds) = update(m, Msg::Click { column: 3, row: 2 });
    assert!(
        cmds.is_empty(),
        "label number beyond body_links length must be a no-op (no panic)"
    );
}

// V4b-A3: A URL that does not pass is_openable_url is not opened.
#[test]
fn click_body_link_non_openable_url_is_noop() {
    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["javascript:alert(1)".to_string()],
        vec![],
        0,
        (80, 24),
    );

    let (_m, cmds) = update(m, Msg::Click { column: 3, row: 2 });
    assert!(
        cmds.is_empty(),
        "non-http/https URL must not be opened (is_openable_url guard)"
    );
}

// V4b-A3 (regression): Asset-panel clicks still work after the body-link change.
// The content-area check happens first; the asset-panel check falls through for rows
// outside the text viewport.
#[test]
fn click_asset_panel_still_works_after_body_link_change() {
    let url = "https://example.com/asset.pdf";
    let assets = vec![make_asset("asset.pdf", url)];
    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["https://example.com/body-link".to_string()],
        assets,
        0,
        (80, 24),
    );

    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let panel = detail_asset_panel_rect(area, 1).expect("panel must exist");
    let asset_row = panel.y + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: asset_row,
        },
    );
    assert_eq!(cmds.len(), 1, "asset panel click must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(cmd_url, url);
        }
        other => panic!("expected OpenAsset for asset panel, got {other:?}"),
    }
}

// V4b-A1: A click row outside the text viewport (e.g. content border row 1
// or a row >= text_top + content_text_height) does not emit a body-link cmd.
#[test]
fn click_outside_content_text_area_is_noop_when_no_label_row() {
    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["https://example.com/doc".to_string()],
        vec![],
        0,
        (80, 24),
    );

    // Row 1 is the top border of the content block (< text_top=2).
    let (_m, cmds) = update(m, Msg::Click { column: 3, row: 1 });
    assert!(
        cmds.is_empty(),
        "click on content top border (row 1) must be a no-op"
    );
}

// --- V3-A1: ToggleSelection model tests ---

fn projects_browse_model() -> Model {
    Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
    }
}

// V3-A1 S5: selection_mode defaults to false on a fresh browse model.
#[test]
fn selection_mode_defaults_false_on_browse_model() {
    use crate::tui::model::init_browse;
    let (m, _) = init_browse(empty_header());
    assert!(
        !m.selection_mode,
        "selection_mode must default to false on init_browse"
    );
}

// V3-A1 S1: pressing 's' (ToggleSelection) flips selection_mode to true and emits
// exactly one Cmd::SetMouseCapture(false) — capture OFF so the terminal can select text.
#[test]
fn toggle_selection_enters_selection_mode_and_emits_set_mouse_capture_false() {
    let m = projects_browse_model();
    let (m, cmds) = update(m, Msg::ToggleSelection);
    assert!(
        m.selection_mode,
        "selection_mode must be true after first ToggleSelection"
    );
    assert_eq!(
        cmds.len(),
        1,
        "must emit exactly one Cmd after ToggleSelection"
    );
    assert_eq!(
        cmds[0],
        Cmd::SetMouseCapture(false),
        "entering selection mode must emit SetMouseCapture(false)"
    );
}

// V3-A1 S2: pressing 's' again leaves selection mode and emits SetMouseCapture(true).
#[test]
fn toggle_selection_leaves_selection_mode_and_emits_set_mouse_capture_true() {
    let m = projects_browse_model();
    let (m, _) = update(m, Msg::ToggleSelection);
    let (m, cmds) = update(m, Msg::ToggleSelection);
    assert!(
        !m.selection_mode,
        "selection_mode must be false after second ToggleSelection"
    );
    assert_eq!(
        cmds.len(),
        1,
        "must emit exactly one Cmd after second ToggleSelection"
    );
    assert_eq!(
        cmds[0],
        Cmd::SetMouseCapture(true),
        "leaving selection mode must emit SetMouseCapture(true)"
    );
}

// V3-A1 S5: two toggles are idempotent — state and Cmd are the same as the start
// after an enter+leave pair.
#[test]
fn two_toggles_return_to_initial_state() {
    let m = projects_browse_model();
    let initial_selection_mode = m.selection_mode;
    let (m, _) = update(m, Msg::ToggleSelection);
    let (m, cmds) = update(m, Msg::ToggleSelection);
    assert_eq!(
        m.selection_mode, initial_selection_mode,
        "two toggles must return selection_mode to initial value"
    );
    assert_eq!(
        cmds[0],
        Cmd::SetMouseCapture(true),
        "second toggle must re-enable capture"
    );
}

// V3-A3: navigation messages produce the same state transitions regardless of selection_mode.
#[test]
fn navigation_msgs_behave_identically_in_selection_and_normal_mode() {
    use crate::tui::model::ProjectGroup;

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

    let normal_model = Model {
        stack: vec![Screen::Projects {
            groups: groups.clone(),
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
    };
    let selection_model = Model {
        stack: vec![Screen::Projects {
            groups,
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (80, 24),
        click_targets: vec![],
        last_loaded: None,
        selection_mode: true,
    };

    let (normal_after, normal_cmds) = update(normal_model, Msg::Down);
    let (selection_after, selection_cmds) = update(selection_model, Msg::Down);

    assert_eq!(
        normal_cmds, selection_cmds,
        "Down must emit identical cmds in both modes"
    );
    match (normal_after.top(), selection_after.top()) {
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
                "Down must advance selection identically in both modes"
            );
        }
        _ => panic!("expected Projects screen in both models"),
    }
}

// V3-A3: Quit sets should_quit regardless of selection_mode.
#[test]
fn quit_sets_should_quit_regardless_of_selection_mode() {
    let normal = projects_browse_model();
    let (normal_after, _) = update(normal, Msg::Quit);
    assert!(
        normal_after.should_quit,
        "Quit must set should_quit in normal mode"
    );

    let m = projects_browse_model();
    let (m_in_selection, _) = update(m, Msg::ToggleSelection);
    let (m_after, _) = update(m_in_selection, Msg::Quit);
    assert!(
        m_after.should_quit,
        "Quit must set should_quit even in selection mode"
    );
}
