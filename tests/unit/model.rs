use super::*;
use crate::render::Asset;
use crate::tui::screens::detail::{asset_panel_render_height, draw_detail, DetailParams};
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use std::collections::HashMap;

/// Panel geometry computed from the shared width-aware wrapped height.
struct PanelGeom {
    /// Row of the panel's top border.
    pub top: u16,
    /// Row immediately inside the top border (first asset row).
    pub first_asset: u16,
    /// Row immediately inside the bottom border (last asset row).
    pub last_asset: u16,
    /// Row of the panel's bottom border.
    pub bottom: u16,
    /// Panel total height (including both borders).
    pub height: u16,
}

impl PanelGeom {
    fn compute(viewport_w: u16, viewport_h: u16, assets: &[Asset]) -> Option<Self> {
        let inner_width = viewport_w.saturating_sub(2) as usize;
        let panel_h = asset_panel_render_height(assets, inner_width);
        if panel_h == 0 {
            return None;
        }
        let top = viewport_h.saturating_sub(panel_h);
        Some(PanelGeom {
            top,
            first_asset: top + 1,
            last_asset: viewport_h.saturating_sub(2),
            bottom: viewport_h.saturating_sub(1),
            height: panel_h,
        })
    }
}

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
            revalidating: false,
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
// first_asset_row = panel_top + 1 (one row inside the top border).
#[test]
fn click_first_asset_row_emits_open_asset_cmd() {
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24), false);
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
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
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 30), false);
    let geom = PanelGeom::compute(80, 30, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.last_asset,
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

// V2a-A1: Click on the panel top border row is a no-op.
#[test]
fn click_on_panel_top_border_row_is_noop() {
    let assets = vec![make_asset("x.pdf", "https://example.com/x.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24), false);
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.top,
        },
    );
    assert!(cmds.is_empty(), "click on top border must be a no-op");
}

// V2a-A1: Click on the panel bottom border row is a no-op.
#[test]
fn click_on_panel_bottom_border_row_is_noop() {
    let assets = vec![make_asset("x.pdf", "https://example.com/x.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24), false);
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.bottom,
        },
    );
    assert!(cmds.is_empty(), "click on bottom border must be a no-op");
}

// V2a-A1: Click above the panel (in the content area) is a no-op.
#[test]
fn click_above_panel_is_noop() {
    let assets = vec![make_asset("y.pdf", "https://example.com/y.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24), false);
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");
    let above_row = geom.top.saturating_sub(1);

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
    let m = detail_model_with_assets_and_viewport(assets.clone(), "acme", (80, 24), true);
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (m_after, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
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
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24), true);
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (m_after, _cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: geom.first_asset,
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

// V2a-A3: asset_panel_render_height returns 0 for zero assets (no panel drawn).
#[test]
fn asset_panel_render_height_zero_for_empty_assets() {
    let h = asset_panel_render_height(&[], 80);
    assert_eq!(h, 0, "panel height must be 0 when assets list is empty");
}

// V2a-A3: asset_panel_render_height produces correct height for non-wrapping assets
// at several viewport sizes — render and click mapper share this single source.
#[test]
fn asset_panel_render_height_consistent_geometry_for_short_names() {
    for (viewport_w, viewport_h, assets_count) in [
        (80u16, 24u16, 1usize),
        (80, 24, 3),
        (80, 24, 6),
        (120, 40, 2),
        (40, 20, 5),
    ] {
        let assets: Vec<Asset> = (0..assets_count)
            .map(|i| make_asset(&format!("f{i}.pdf"), &format!("https://x.com/f{i}.pdf")))
            .collect();
        let inner_width = (viewport_w - 2) as usize;
        let panel_h = asset_panel_render_height(&assets, inner_width);
        let expected_h = (assets_count as u16 + 2).min(8);
        assert_eq!(
            panel_h, expected_h,
            "panel height for {assets_count} short assets must equal min(n+2,8)={expected_h} \
             at viewport ({viewport_w}x{viewport_h})"
        );

        let geom = PanelGeom::compute(viewport_w, viewport_h, &assets)
            .unwrap_or_else(|| panic!("panel must exist for {assets_count} assets"));
        assert_eq!(
            geom.top + geom.height,
            viewport_h,
            "panel bottom must align with viewport bottom"
        );
    }
}

// V2a-A3: Render and click mapper use asset_panel_render_height so geometry cannot diverge.
// Clicking the first asset row opens the first asset, verified across multiple viewport sizes.
#[test]
fn click_mapper_agrees_with_render_height_for_multiple_viewport_sizes() {
    for (viewport_w, viewport_h) in [(80u16, 24u16), (120, 40), (40, 20)] {
        let assets = vec![
            make_asset("doc1.pdf", "https://example.com/doc1.pdf"),
            make_asset("doc2.pdf", "https://example.com/doc2.pdf"),
        ];
        let m = detail_model_with_assets_and_viewport(
            assets.clone(),
            "inst",
            (viewport_w, viewport_h),
            false,
        );
        let geom = PanelGeom::compute(viewport_w, viewport_h, &assets).expect("panel must exist");

        let (_m, cmds) = update(
            m,
            Msg::Click {
                column: 0,
                row: geom.first_asset,
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
// at exactly the row computed by asset_panel_render_height (the shared source of truth).
// The test renders via TestBackend and checks that the "Artifacts" border title appears
// at the expected panel_top row.
#[test]
fn draw_detail_panel_rows_match_asset_panel_render_height_for_multiple_viewports() {
    for (viewport_w, viewport_h, assets_len) in [(80u16, 24u16, 2usize), (120, 40, 3), (40, 20, 1)]
    {
        let assets: Vec<Asset> = (0..assets_len)
            .map(|i| make_asset(&format!("file{i}.pdf"), &format!("https://x.com/f{i}.pdf")))
            .collect();
        let geom = PanelGeom::compute(viewport_w, viewport_h, &assets)
            .unwrap_or_else(|| panic!("panel must exist for assets_len={assets_len}"));

        let area = Rect::new(0, 0, viewport_w, viewport_h);
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
            .map(|x| buf.cell((x, geom.top)).unwrap().symbol().to_string())
            .collect();

        assert!(
            panel_top_row.contains("Artifacts"),
            "Artifacts panel top border must appear at row {} (asset_panel_render_height-derived) \
             for viewport=({viewport_w}x{viewport_h}), assets_len={assets_len}. \
             Renderer and shared fn geometry disagree if this fails. \
             row={panel_top_row:?}",
            geom.top
        );
    }
}

// W2-A3 geometry: when an asset label wraps, both draw_detail and asset_panel_cmd_at
// use asset_panel_render_height so they share ONE source of truth.
// This test renders via TestBackend and verifies "Artifacts" appears at the row
// computed by asset_panel_render_height (not the shorter unwrapped height).
#[test]
fn draw_detail_wrapped_asset_panel_top_matches_asset_panel_render_height() {
    let viewport_w = 20u16;
    let viewport_h = 24u16;

    // At panel_inner=16: "[1] ↗ " = 7 cols, label_width = 9 cols.
    // Name "ABCDEFGHIJKLMNOPQRS.pdf" > 9 cols in label → wraps to 2 rows.
    let long_name = "ABCDEFGHIJKLMNOPQRS.pdf";
    let assets = vec![make_asset(long_name, "https://example.com/long.pdf")];

    let inner_width = (viewport_w - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let expected_panel_top = viewport_h - panel_h;

    let backend = TestBackend::new(viewport_w, viewport_h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_detail(
                frame,
                Rect::new(0, 0, viewport_w, viewport_h),
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
        .map(|x| {
            buf.cell((x, expected_panel_top))
                .unwrap()
                .symbol()
                .to_string()
        })
        .collect();

    assert!(
        panel_top_row.contains("Artifacts"),
        "Artifacts panel top must appear at row {expected_panel_top} (from asset_panel_render_height) \
         for a wrapped-asset label at viewport={viewport_w}x{viewport_h}. \
         If this fails, render and model height formulas have diverged. \
         row content: {panel_top_row:?}"
    );

    // Also verify that the UNWRAPPED height (1 row per asset + 2 borders, capped at 8) would
    // predict the WRONG row, confirming that wrapping actually shifts the panel top upward.
    let unwrapped_h = (assets.len() as u16 + 2).min(8);
    if panel_h != unwrapped_h {
        let wrong_top = viewport_h - unwrapped_h;
        let wrong_row: String = (0..viewport_w)
            .map(|x| buf.cell((x, wrong_top)).unwrap().symbol().to_string())
            .collect();
        assert!(
            !wrong_row.contains("Artifacts"),
            "Artifacts must NOT appear at unwrapped-height row {wrong_top} \
             (panel moved up by wrapping): {wrong_row:?}"
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

// V5-A1: detail_max_offset — 2 short assets (panel_h=4 unwrapped=wrapped), viewport 24x80, lines_len=50.
// text_vh=24-4-4=16, max=50-16=34.
#[test]
fn detail_max_offset_with_assets_shrinks_text_viewport() {
    use crate::tui::model::detail_max_offset;
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let max = detail_max_offset(24, 80, 50, &assets);
    assert_eq!(
        max, 34,
        "viewport=24x80, 2 short assets (panel_h=4): text_vh=16, max=50-16=34"
    );
}

// V5-A1: detail_max_offset — 6 short assets (panel_h=8 capped), viewport 24x80, lines_len=50.
// text_vh=24-4-8=12, max=50-12=38.
#[test]
fn detail_max_offset_many_assets_caps_panel_height() {
    use crate::tui::model::detail_max_offset;
    let assets: Vec<Asset> = (0..6)
        .map(|i| make_asset(&format!("f{i}.pdf"), &format!("https://x.com/f{i}")))
        .collect();
    let max = detail_max_offset(24, 80, 50, &assets);
    assert_eq!(
        max, 38,
        "viewport=24x80, 6 short assets (panel_h=8 capped): text_vh=12, max=50-12=38"
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

// V5-A1: handle_down clamps to detail_max_offset, not lines.len()-1.
// viewport=80x24, 50 lines, no assets → max=30. Scroll 60 times → offset stays at 30.
#[test]
fn handle_down_clamps_to_detail_max_offset_no_assets() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    for _ in 0..60 {
        model = update(model, Msg::Down).0;
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

// V5-A1: handle_down is idempotent at max — pressing Down when already at max does nothing.
#[test]
fn handle_down_idempotent_at_max() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let mut model = detail_model_scrollable(lines.clone(), vec![], (80, 24));

    let max = detail_max_offset(24, 80, 50, &[]);
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
// viewport=80x24, 50 lines, 2 short assets (panel_h=4 unwrapped=wrapped) → max=34.
#[test]
fn handle_down_clamps_to_detail_max_offset_with_assets() {
    use crate::tui::model::detail_max_offset;
    let lines: Vec<String> = (0..50).map(|i| format!("line {i}")).collect();
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let mut model = detail_model_scrollable(lines.clone(), assets.clone(), (80, 24));

    for _ in 0..100 {
        model = update(model, Msg::Down).0;
    }

    let expected_max = detail_max_offset(24, 80, 50, &assets);
    match model.top() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, expected_max,
                "offset with 2 short assets must clamp to {expected_max}"
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
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");
    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["https://example.com/body-link".to_string()],
        assets,
        0,
        (80, 24),
    );

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
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
            revalidating: false,
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
    let (m, _) = init_browse(empty_header(), None);
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
            revalidating: false,
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
            revalidating: false,
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
        last_loaded: None,
        selection_mode: false,
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

// W2-A3: Click on a wrapped asset's continuation row resolves to the owning asset,
// not the (now mis-shifted) asset that would follow at 1-row-per-asset arithmetic.
//
// At viewport width=20: inner_width=18, panel_inner_width=16.
// "[1] ↗ " prefix is 7 cols, so label_width = 16 - 7 = 9 cols.
// A name that exceeds 9 chars wraps to a second row.
// So asset[0] occupies panel rows 0 and 1; asset[1] occupies panel row 2.
//
// panel_h includes border rows (wrapped_row_count + 2).min(8).
// first_asset_row = panel_top + 1 (skip top border).
// Click on first_asset_row+0 → asset[0].
// Click on first_asset_row+1 → asset[0] (continuation row).
// Click on first_asset_row+(rows of asset[0]) → asset[1].
fn make_wrapped_asset_model(viewport: (u16, u16), pending_download: bool) -> (Vec<Asset>, Model) {
    // At panel_inner=16: prefix "[1] ↗ " = 7 cols, label_width = 9 cols.
    // Name "ABCDEFGHIJKLMNOPQRS.pdf" exceeds 9 cols in label → wraps to 2 rows.
    let long_name = "ABCDEFGHIJKLMNOPQRS.pdf";
    let assets = vec![
        make_asset(long_name, "https://example.com/long.pdf"),
        make_asset("short.pdf", "https://example.com/short.pdf"),
    ];
    let m =
        detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport, pending_download);
    (assets, m)
}

#[test]
fn click_wrapped_asset_first_row_opens_owning_asset() {
    let viewport = (20u16, 24u16);
    let (assets, m) = make_wrapped_asset_model(viewport, false);

    let panel_inner: usize = 16;
    let row_count_asset0 = crate::render::asset_row_lines(1, &assets[0], panel_inner).len();
    assert!(
        row_count_asset0 >= 2,
        "asset[0] must wrap to >=2 rows at panel_inner={panel_inner}"
    );

    let inner_width: usize = (viewport.0 - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;
    let first_asset_row = panel_top + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: first_asset_row,
        },
    );
    assert_eq!(cmds.len(), 1, "panel_row=0 must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/long.pdf",
                "panel_row=0 must open asset[0]"
            );
        }
        other => panic!("expected OpenAsset for asset[0], got {other:?}"),
    }
}

#[test]
fn click_wrapped_asset_continuation_row_resolves_to_owning_asset() {
    let viewport = (20u16, 24u16);
    let (assets, m) = make_wrapped_asset_model(viewport, false);

    let panel_inner: usize = 16;
    let row_count_asset0 = crate::render::asset_row_lines(1, &assets[0], panel_inner).len();
    assert!(
        row_count_asset0 >= 2,
        "asset[0] must wrap at panel_inner={panel_inner}"
    );

    let inner_width: usize = (viewport.0 - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;
    let continuation_row = panel_top + 1 + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: continuation_row,
        },
    );
    assert_eq!(cmds.len(), 1, "continuation panel_row=1 must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/long.pdf",
                "continuation row must resolve to asset[0], not asset[1]"
            );
        }
        other => panic!("expected OpenAsset for asset[0] on continuation, got {other:?}"),
    }
}

#[test]
fn click_second_asset_row_after_wrapped_first_asset_resolves_correctly() {
    let viewport = (20u16, 24u16);
    let (assets, m) = make_wrapped_asset_model(viewport, false);

    let panel_inner: usize = 16;
    let row_count_asset0 = crate::render::asset_row_lines(1, &assets[0], panel_inner).len();
    assert!(
        row_count_asset0 >= 2,
        "asset[0] must wrap at panel_inner={panel_inner}"
    );

    let inner_width: usize = (viewport.0 - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;
    let second_asset_row = panel_top + 1 + row_count_asset0 as u16;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: second_asset_row,
        },
    );
    assert_eq!(cmds.len(), 1, "first row of asset[1] must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/short.pdf",
                "row after wrapped asset[0] must resolve to asset[1]"
            );
        }
        other => panic!("expected OpenAsset for asset[1], got {other:?}"),
    }
}

// W2-A2: detail_max_offset uses the width-aware wrapped panel height so the body
// cannot scroll behind the taller panel.
//
// When an asset label wraps, wrapped_panel_height > unwrapped_panel_height, which
// leaves fewer text rows.  With fewer text rows the text viewport is smaller and
// max_offset is larger (body must scroll further to reveal the same content, but
// correctly stops before the panel).
#[test]
fn detail_max_offset_with_wrapped_asset_accounts_for_taller_panel() {
    use crate::tui::model::detail_max_offset;

    let long_name = "ABCDEFGHIJKLMNOPQRS.pdf";
    let wrapping_asset = vec![make_asset(long_name, "https://example.com/long.pdf")];
    let short_asset = vec![make_asset("a.pdf", "https://example.com/a.pdf")];

    let lines_len = 50usize;
    let viewport_rows = 24u16;
    let viewport_cols = 20u16;
    let inner_width = (viewport_cols - 2) as usize;

    let wrapped_panel_h = asset_panel_render_height(&wrapping_asset, inner_width);
    let short_panel_h = asset_panel_render_height(&short_asset, inner_width);
    assert!(
        wrapped_panel_h > short_panel_h,
        "wrapping asset must produce taller panel: wrapped={wrapped_panel_h} short={short_panel_h}"
    );

    let max_wrapping = detail_max_offset(viewport_rows, viewport_cols, lines_len, &wrapping_asset);
    let max_short = detail_max_offset(viewport_rows, viewport_cols, lines_len, &short_asset);

    // Taller panel → smaller text viewport → larger max_offset (more content above the fold).
    assert!(
        max_wrapping > max_short,
        "taller wrapped panel must produce a larger max_offset (less text viewport): \
         wrapping={max_wrapping} short={max_short}"
    );

    // Sanity: verify the numeric values match the expected geometry.
    // wrapped: text_vh = 24 - 4 - wrapped_panel_h; max = 50 - text_vh
    let expected_text_vh_wrapping = viewport_rows
        .saturating_sub(4)
        .saturating_sub(wrapped_panel_h) as usize;
    let expected_max_wrapping = lines_len.saturating_sub(expected_text_vh_wrapping.max(1));
    assert_eq!(
        max_wrapping, expected_max_wrapping,
        "max_offset with wrapping asset must equal 50 - text_viewport_height(wrapped)"
    );
}

// W2-A2: body_link_cmd_at rejects a click at a row that falls inside the real
// (wrapped) panel region.  At viewport 20x24 with 1 wrapping asset:
// wrapped_panel_h >= 4 → real panel_top = 24 - 4 = 20.
// A click at row 20 must NOT emit a body-link cmd (it is a panel border row).
#[test]
fn body_link_click_at_wrapped_panel_region_is_noop() {
    let long_name = "ABCDEFGHIJKLMNOPQRS.pdf";
    let assets = vec![make_asset(long_name, "https://example.com/long.pdf")];

    let inner_width: usize = 18; // viewport_cols=20 - 2
    let wrapped_h = asset_panel_render_height(&assets, inner_width);
    assert!(
        wrapped_h >= 4,
        "wrapping asset must produce panel_h >= 4 at inner_width={inner_width}: got {wrapped_h}"
    );

    let label_line = "\u{2502} \u{2197} Link 1 \u{2502}".to_string();
    let m = detail_model_with_links(
        vec![label_line],
        vec!["https://example.com/body-link".to_string()],
        assets,
        0,
        (20, 24),
    );

    // The real (wrapped) panel top border row — must not be a body-link hit.
    let panel_top = 24u16 - wrapped_h;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 3,
            row: panel_top,
        },
    );
    assert!(
        cmds.is_empty(),
        "click at real panel top border (row {panel_top}, wrapped_h={wrapped_h}) \
         must be a no-op, not a body-link hit"
    );
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
        },
        MineTableRow {
            instance: "inst-b".into(),
            project_id: 2,
            task_number: 20,
            task_id: 200,
            name: "Task Beta".into(),
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
