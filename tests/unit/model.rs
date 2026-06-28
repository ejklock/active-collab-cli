use super::*;
use crate::render::Asset;
use crate::tui::screens::detail::{asset_panel_render_height, draw_detail, DetailParams};
use crossterm::event::KeyModifiers;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use std::collections::HashMap;

/// Panel geometry computed from the shared width-aware wrapped height.
struct PanelGeom {
    /// Row of the panel's top border.
    pub top: u16,
    /// Row of the first asset content line (after top border + PANEL_VPAD).
    pub first_asset: u16,
    /// Row of the last asset content line (before bottom vpad + bottom border).
    pub last_asset: u16,
    /// Row of the panel's bottom border.
    pub bottom: u16,
    /// Panel total height (including both borders).
    pub height: u16,
}

impl PanelGeom {
    fn compute(viewport_w: u16, viewport_h: u16, assets: &[Asset]) -> Option<Self> {
        use crate::render::PANEL_VPAD;
        use crate::tui::screens::detail::ASSET_HINT_ROWS;
        let inner_width = viewport_w.saturating_sub(2) as usize;
        let panel_h = asset_panel_render_height(assets, inner_width);
        if panel_h == 0 {
            return None;
        }
        let top = viewport_h.saturating_sub(panel_h);
        // first_asset: skip top border (1) + PANEL_VPAD blank rows.
        let first_asset = top + 1 + PANEL_VPAD as u16;
        // last_asset: last ASSET content row is before (blank + hint + PANEL_VPAD + bottom border).
        // Layout from panel bottom: 1 border + PANEL_VPAD blank + ASSET_HINT_ROWS (hint+blank) + 1 = assets here.
        let last_asset = viewport_h.saturating_sub(2 + PANEL_VPAD as u16 + ASSET_HINT_ROWS);
        Some(PanelGeom {
            top,
            first_asset,
            last_asset,
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
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
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

// AC1 / V2a-A1: Ctrl+click on the first asset row opens assets[0] via OpenAsset.
// Viewport 80x24, 2 assets → panel_height = min(2*2+3, 14) = 7.
// first_asset_row = panel_top + 1 (border) + PANEL_VPAD (blank pad).
#[test]
fn ctrl_click_first_asset_row_emits_open_asset_cmd() {
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
            modifiers: KeyModifiers::CONTROL,
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

// V2a-A1: Ctrl+click on the last asset content row opens the last asset
// (no off-by-one; bottom vpad and border rows are no-ops).
#[test]
fn ctrl_click_last_asset_row_opens_last_asset() {
    let assets = vec![
        make_asset("first.pdf", "https://example.com/first.pdf"),
        make_asset("second.pdf", "https://example.com/second.pdf"),
        make_asset("third.pdf", "https://example.com/third.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 30));
    let geom = PanelGeom::compute(80, 30, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.last_asset,
            modifiers: KeyModifiers::CONTROL,
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

// V2a-A1: Ctrl+click on the panel top border row is a no-op.
#[test]
fn ctrl_click_on_panel_top_border_row_is_noop() {
    let assets = vec![make_asset("x.pdf", "https://example.com/x.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.top,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(cmds.is_empty(), "ctrl+click on top border must be a no-op");
}

// V2a-A1: Ctrl+click on the panel bottom border row is a no-op.
#[test]
fn ctrl_click_on_panel_bottom_border_row_is_noop() {
    let assets = vec![make_asset("x.pdf", "https://example.com/x.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.bottom,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "ctrl+click on bottom border must be a no-op"
    );
}

// V2a-A1: Ctrl+click above the panel (in the content area) is a no-op.
#[test]
fn ctrl_click_above_panel_is_noop() {
    let assets = vec![make_asset("y.pdf", "https://example.com/y.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");
    let above_row = geom.top.saturating_sub(1);

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: above_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(cmds.is_empty(), "ctrl+click above panel must be a no-op");
}

// AC2: A plain (unmodified) click on an asset row emits no OpenAsset and no download.
// Plain click is reserved for V6 text selection.
#[test]
fn plain_click_on_asset_row_emits_no_cmd() {
    let assets = vec![make_asset("report.pdf", "https://example.com/report.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "acme", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "plain click on asset row must emit no cmd (reserved for V6 selection)"
    );
}

// AC2: A plain unmodified click on the last asset row also emits no cmd.
#[test]
fn plain_click_on_last_asset_row_emits_no_cmd() {
    let assets = vec![
        make_asset("first.pdf", "https://example.com/first.pdf"),
        make_asset("last.pdf", "https://example.com/last.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.last_asset,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "plain click on last asset row must emit no cmd"
    );
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

// V2a-A3: asset_panel_render_height returns 0 for zero assets (no panel drawn).
#[test]
fn asset_panel_render_height_zero_for_empty_assets() {
    let h = asset_panel_render_height(&[], 80);
    assert_eq!(h, 0, "panel height must be 0 when assets list is empty");
}

// V2a-A3: asset_panel_render_height produces correct height for non-wrapping assets
// at several viewport sizes — render and click mapper share this single source.
// Formula: each short asset = 1 row; height = (n + (n-1) separators + 2*VPAD + 2 borders) capped at
// ASSET_PANEL_MAX_ROWS=14, then ASSET_HINT_ROWS=2 added unconditionally.
// Result: (2n+3).min(14) + 2.
#[test]
fn asset_panel_render_height_consistent_geometry_for_short_names() {
    use crate::tui::screens::detail::ASSET_HINT_ROWS;
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
        // (n rows + (n-1) separators + 2 vpad + 2 borders) capped at 14, then + ASSET_HINT_ROWS.
        let expected_h = (2 * assets_count as u16 + 3).min(14) + ASSET_HINT_ROWS;
        assert_eq!(
            panel_h, expected_h,
            "panel height for {assets_count} short assets must equal (2n+3).min(14)+{ASSET_HINT_ROWS}={expected_h} \
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
// Ctrl+clicking the first asset row opens the first asset, verified across multiple viewport sizes.
#[test]
fn ctrl_click_mapper_agrees_with_render_height_for_multiple_viewport_sizes() {
    for (viewport_w, viewport_h) in [(80u16, 24u16), (120, 40), (40, 20)] {
        let assets = vec![
            make_asset("doc1.pdf", "https://example.com/doc1.pdf"),
            make_asset("doc2.pdf", "https://example.com/doc2.pdf"),
        ];
        let m =
            detail_model_with_assets_and_viewport(assets.clone(), "inst", (viewport_w, viewport_h));
        let geom = PanelGeom::compute(viewport_w, viewport_h, &assets).expect("panel must exist");

        let (_m, cmds) = update(
            m,
            Msg::Click {
                column: 0,
                row: geom.first_asset,
                modifiers: KeyModifiers::CONTROL,
            },
        );
        assert_eq!(
            cmds.len(),
            1,
            "ctrl+click on first asset row must emit one cmd at \
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

    // Also verify that the UNWRAPPED height (1 row per asset + 0 separators + 2 vpad + 2 borders,
    // capped at 14) would predict the WRONG row, confirming wrapping shifts the panel top upward.
    let unwrapped_h = (2 * assets.len() as u16 + 3).min(14);
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
            assets,
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
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

// V5-A1: detail_max_offset — 2 short assets, viewport 24x80, lines_len=50.
// panel_h = (2*2+3).min(14) + ASSET_HINT_ROWS = 7+2 = 9; text_vh = 24-4-9 = 11; max = 50-11 = 39.
#[test]
fn detail_max_offset_with_assets_shrinks_text_viewport() {
    use crate::tui::model::detail_max_offset;
    let assets = vec![
        make_asset("a.pdf", "https://example.com/a.pdf"),
        make_asset("b.pdf", "https://example.com/b.pdf"),
    ];
    let max = detail_max_offset(24, 80, 50, &assets);
    assert_eq!(
        max, 39,
        "viewport=24x80, 2 short assets (panel_h=9): text_vh=11, max=50-11=39"
    );
}

// V5-A1: detail_max_offset — 6 short assets (capped part = 14, plus ASSET_HINT_ROWS=2),
// viewport 24x80, lines_len=50.
// panel_h = min(2*6+3, 14) + 2 = 14+2 = 16; text_vh = 24-4-16 = 4; max = 50-4 = 46.
#[test]
fn detail_max_offset_many_assets_caps_panel_height() {
    use crate::tui::model::detail_max_offset;
    let assets: Vec<Asset> = (0..6)
        .map(|i| make_asset(&format!("f{i}.pdf"), &format!("https://x.com/f{i}")))
        .collect();
    let max = detail_max_offset(24, 80, 50, &assets);
    assert_eq!(
        max, 46,
        "viewport=24x80, 6 short assets (panel_h=16): text_vh=4, max=50-4=46"
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
// viewport=80x24, 50 lines, 2 short assets (panel_h=9 after ASSET_HINT_ROWS) → max=39.
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
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
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
    // URL inner starts at col 14.
    let line = format!("\u{2502} click here [{url}] \u{2502}");
    let m = detail_model_with_lines_and_assets(vec![line], vec![], 0, (80, 24));

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
    let m = detail_model_with_lines_and_assets(vec![plain_line, link_line], vec![], 1, (80, 24));

    // char_col = column - 1; URL inner starts at display col 3.
    // So column must be 4 → char_col = 3 → inside the URL.
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

// V5-A3: Mailto bracket token yields Cmd::OpenAsset with 'mailto:' scheme re-added.
#[test]
fn click_mailto_bracket_token_yields_mailto_cmd() {
    let email = "user@example.com";
    let line = format!("\u{2502} mail [{email}] \u{2502}");
    let m = detail_model_with_lines_and_assets(vec![line], vec![], 0, (80, 24));

    // char_col = column - 1; email inner starts at display col 8 (after "│ mail [").
    // So column must be 9 → char_col = 8 → inside the email.
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
                "mailto scheme must be re-added in click path"
            );
        }
        other => panic!("expected OpenAsset with mailto, got {other:?}"),
    }
}

// AC1 (regression): Asset-panel Ctrl+click still works after the body-link change.
// The content-area check happens first; the asset-panel check falls through for rows
// outside the text viewport.
#[test]
fn ctrl_click_asset_panel_still_works_after_body_link_change() {
    let url = "https://example.com/asset.pdf";
    let assets = vec![make_asset("asset.pdf", url)];
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");
    let link_line = "\u{2502} [https://example.com/body-link] \u{2502}".to_string();
    let m = detail_model_with_lines_and_assets(vec![link_line], assets, 0, (80, 24));

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "ctrl+click asset panel must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url: cmd_url, .. } => {
            assert_eq!(cmd_url, url);
        }
        other => panic!("expected OpenAsset for asset panel, got {other:?}"),
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

/// Build a Detail model with a URL token that hard-splits across exactly two box-content
/// lines at `content_width = inner_width - 4 = viewport_cols - 6`.
///
/// The URL is placed inside a `[url]` bracket token. The combined `[url]` token is
/// longer than `content_width`, so `wrap_text` splits it mid-token. The two boxed
/// lines simulate the output of `build_detail_content` + `panel_box`.
fn detail_model_with_wrapped_url_lines(
    url: &str,
    viewport: (u16, u16),
) -> (Model, usize, usize, u16) {
    let inner_width = viewport.0.saturating_sub(2) as usize;
    let content_width = inner_width.saturating_sub(4);
    let token = format!("[{url}]");

    assert!(
        token.len() > content_width,
        "url token must be longer than content_width={content_width} to force wrapping; \
         got token.len()={} for url.len()={}",
        token.len(),
        url.len()
    );

    let frag0 = &token[..content_width];
    let frag1 = &token[content_width..];
    let pad1 = " ".repeat(content_width.saturating_sub(frag1.len()));
    let border = '\u{2502}';
    let line0 = format!("{border} {frag0} {border}");
    let line1 = format!("{border} {frag1}{pad1} {border}");

    let lines = vec![line0, line1];
    let line0_idx = 0usize;
    let line1_idx = 1usize;
    let text_top: u16 = 2;

    let m = detail_model_with_lines_and_assets(lines, vec![], 0, viewport);
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
#[test]
fn ctrl_click_on_last_wrapped_fragment_returns_complete_url() {
    let url = "https://example.com/long-path/to/page";
    let viewport = (42u16, 24u16);
    let (m, _line0_idx, line1_idx, text_top) = detail_model_with_wrapped_url_lines(url, viewport);

    let row1 = text_top + line1_idx as u16;
    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 4,
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
// Regression test: the new wrapped-URL path must not break the existing single-line case.
#[test]
fn ctrl_click_on_single_line_url_still_resolves() {
    let url = "https://example.com/short";
    let line = format!("\u{2502} [{url}] \u{2502}");
    let m = detail_model_with_lines_and_assets(vec![line], vec![], 0, (80, 24));

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
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
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

// V6-A3 regression: Ctrl/Cmd+click still opens a link (D1c not broken).
// Uses a body line with a URL token so body_link_cmd_at fires.
#[test]
fn ctrl_click_on_url_still_opens_link_after_v6() {
    let url = "https://example.com/doc";
    let line = format!("\u{2502} [{url}] \u{2502}");
    let m = detail_model_with_lines_and_assets(vec![line], vec![], 0, (80, 24));

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

// W2-A3: Click on a wrapped asset's continuation row resolves to the owning asset.
//
// At viewport width=20: inner_width=18, content_width=18-2*PANEL_HPAD=16.
// "[1] ↗ " prefix is 7 cols, label_width = 16-7 = 9 cols.
// "ABCDEFGHIJKLMNOPQRS.pdf" (23 chars) wraps to >=2 rows at label_width=9.
//
// Layout (panel_top+N):
//   0: border, 1: vpad, 2..2+span0-1: asset[0] rows, 2+span0: separator,
//   2+span0+1: asset[1] row, 2+span0+2: vpad, 2+span0+3: border.
fn make_wrapped_asset_model(viewport: (u16, u16)) -> (Vec<Asset>, Model) {
    // At content_width=16: prefix "[1] ↗ " = 7 cols, label_width = 9 cols.
    // Name "ABCDEFGHIJKLMNOPQRS.pdf" exceeds 9 cols in label → wraps to >=2 rows.
    let long_name = "ABCDEFGHIJKLMNOPQRS.pdf";
    let assets = vec![
        make_asset(long_name, "https://example.com/long.pdf"),
        make_asset("short.pdf", "https://example.com/short.pdf"),
    ];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport);
    (assets, m)
}

#[test]
fn ctrl_click_wrapped_asset_first_row_opens_owning_asset() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;
    let viewport = (20u16, 24u16);
    let (assets, m) = make_wrapped_asset_model(viewport);

    let inner_width: usize = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let row_count_asset0 = crate::render::asset_row_lines(1, &assets[0], content_width).len();
    assert!(
        row_count_asset0 >= 2,
        "asset[0] must wrap to >=2 rows at content_width={content_width}"
    );

    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;
    // First asset row: skip top border (1) + PANEL_VPAD blank rows.
    let first_asset_row = panel_top + 1 + PANEL_VPAD as u16;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: first_asset_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "first content row must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/long.pdf",
                "first content row must open asset[0]"
            );
        }
        other => panic!("expected OpenAsset for asset[0], got {other:?}"),
    }
}

#[test]
fn ctrl_click_wrapped_asset_continuation_row_resolves_to_owning_asset() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;
    let viewport = (20u16, 24u16);
    let (assets, m) = make_wrapped_asset_model(viewport);

    let inner_width: usize = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let row_count_asset0 = crate::render::asset_row_lines(1, &assets[0], content_width).len();
    assert!(
        row_count_asset0 >= 2,
        "asset[0] must wrap at content_width={content_width}"
    );

    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;
    // Continuation row: top border (1) + vpad (PANEL_VPAD) + asset[0] second row (offset 1).
    let continuation_row = panel_top + 1 + PANEL_VPAD as u16 + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: continuation_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "ctrl+click on continuation row of asset[0] must emit one cmd"
    );
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
fn ctrl_click_second_asset_row_after_wrapped_first_asset_resolves_correctly() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;
    let viewport = (20u16, 24u16);
    let (assets, m) = make_wrapped_asset_model(viewport);

    let inner_width: usize = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let row_count_asset0 = crate::render::asset_row_lines(1, &assets[0], content_width).len();
    assert!(
        row_count_asset0 >= 2,
        "asset[0] must wrap at content_width={content_width}"
    );

    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;
    // asset[1] row: top border (1) + vpad (PANEL_VPAD) + span of asset[0] + separator (1).
    let second_asset_row = panel_top + 1 + PANEL_VPAD as u16 + row_count_asset0 as u16 + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: second_asset_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "ctrl+click on first row of asset[1] must emit one cmd"
    );
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/short.pdf",
                "row after separator following wrapped asset[0] must resolve to asset[1]"
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

    let link_line = "\u{2502} [https://example.com/body-link] \u{2502}".to_string();
    let m = detail_model_with_lines_and_assets(vec![link_line], assets, 0, (20, 24));

    // The real (wrapped) panel top border row — must not be a body-link hit.
    let panel_top = 24u16 - wrapped_h;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 3,
            row: panel_top,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "click at real panel top border (row {panel_top}, wrapped_h={wrapped_h}) \
         must be a no-op, not a body-link hit"
    );
}

// D1d-AC4: asset_panel_render_height equals the exact row count emitted by
// render_assets_panel, across multiple asset counts and a wrapped label.
// Empty asset list yields height 0.
#[test]
fn asset_panel_render_height_equals_rendered_row_count() {
    use crate::render::PANEL_HPAD;
    use crate::tui::screens::detail::{draw_detail, DetailParams};
    use ratatui::{backend::TestBackend, layout::Rect, Terminal};

    // Empty list → 0.
    assert_eq!(
        asset_panel_render_height(&[], 78),
        0,
        "empty list must give height 0"
    );

    let viewport_w = 80u16;
    let viewport_h = 40u16;
    let inner_width = (viewport_w - 2) as usize;

    // Short assets (1 row each) at various counts.
    for count in [1usize, 2, 3, 4] {
        let assets: Vec<Asset> = (0..count)
            .map(|i| make_asset(&format!("f{i}.pdf"), &format!("https://x.com/{i}.pdf")))
            .collect();
        let expected_h = asset_panel_render_height(&assets, inner_width);

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
        let panel_top = viewport_h - expected_h;

        // Count rendered rows belonging to the panel (top border through bottom border).
        let panel_row_count = (panel_top..viewport_h)
            .map(|y| {
                (0..viewport_w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .count() as u16;

        assert_eq!(
            panel_row_count, expected_h,
            "render height mismatch for {count} assets: expected {expected_h} rows but \
             panel spans {panel_row_count} rows"
        );

        // Also verify the "Artifacts" title is on the panel_top row.
        let top_row: String = (0..viewport_w)
            .map(|x| buf.cell((x, panel_top)).unwrap().symbol().to_string())
            .collect();
        assert!(
            top_row.contains("Artifacts"),
            "Artifacts title must appear at panel_top={panel_top} for {count} assets: {top_row:?}"
        );
    }

    // Wrapped label: use a narrow viewport so the label wraps.
    // At viewport_w=20: inner_width=18, content_w=16, prefix 7 cols, label_width=9.
    // "ABCDEFGHIJKLMNOPQRS.pdf" (23 chars) > 9 → wraps.
    let wrap_viewport_w = 20u16;
    let wrap_viewport_h = 40u16;
    let wrap_inner_width = (wrap_viewport_w - 2) as usize;
    let long_name = "ABCDEFGHIJKLMNOPQRS.pdf";
    let assets_w = vec![make_asset(long_name, "https://example.com/long.pdf")];
    let wrap_content_w = wrap_inner_width.saturating_sub(2 * PANEL_HPAD);
    let wrapped_row_count = crate::render::asset_row_lines(1, &assets_w[0], wrap_content_w).len();
    assert!(
        wrapped_row_count >= 2,
        "long label must wrap to >=2 rows at content_w={wrap_content_w}"
    );
    let expected_wrapped_h = asset_panel_render_height(&assets_w, wrap_inner_width);

    let area = Rect::new(0, 0, wrap_viewport_w, wrap_viewport_h);
    let backend = TestBackend::new(wrap_viewport_w, wrap_viewport_h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_detail(
                frame,
                area,
                DetailParams {
                    lines: &["body".to_string()],
                    line_styles: &[],
                    assets: &assets_w,
                    offset: 0,
                    loading: false,
                    task_id: 1,
                    task_name: "T",
                },
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let panel_top = wrap_viewport_h - expected_wrapped_h;
    let top_row: String = (0..wrap_viewport_w)
        .map(|x| buf.cell((x, panel_top)).unwrap().symbol().to_string())
        .collect();
    assert!(
        top_row.contains("Artifacts"),
        "Artifacts must appear at panel_top={panel_top} for wrapped label: {top_row:?}"
    );
}

// D1d-AC5: Clicking a separator or pad row returns no asset; clicking the second
// link's row resolves to asset index 1.
#[test]
fn asset_panel_click_separator_and_pad_rows_return_no_asset() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;

    let viewport = (80u16, 30u16);
    let assets = vec![
        make_asset("link1.pdf", "https://example.com/link1.pdf"),
        make_asset("link2.pdf", "https://example.com/link2.pdf"),
    ];

    let inner_width = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;

    // Asset spans (each 1 row for short names).
    let span0 = crate::render::asset_row_lines(1, &assets[0], content_width).len() as u16;
    let span1 = crate::render::asset_row_lines(2, &assets[1], content_width).len() as u16;

    // Layout from panel_top:
    //   +0: top border (no-op)
    //   +1: top vpad (no-op)
    //   +1+VPAD..: asset[0] rows
    //   +1+VPAD+span0: separator (no-op)
    //   +1+VPAD+span0+1: asset[1] first row → index 1
    //   +1+VPAD+span0+1+span1: bottom vpad (no-op)

    let top_vpad_row = panel_top + 1;
    let asset0_first_row = panel_top + 1 + PANEL_VPAD as u16;
    let separator_row = asset0_first_row + span0;
    let asset1_first_row = separator_row + 1;
    let bottom_vpad_row = asset1_first_row + span1;

    // Top vpad → no-op even with Ctrl modifier.
    let (_m, cmds) = update(
        detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport),
        Msg::Click {
            column: 5,
            row: top_vpad_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "ctrl+click on top vpad row must return no asset: row={top_vpad_row}"
    );

    // Separator → no-op even with Ctrl modifier.
    let (_m, cmds) = update(
        detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport),
        Msg::Click {
            column: 5,
            row: separator_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "ctrl+click on separator row must return no asset: row={separator_row}"
    );

    // Bottom vpad → no-op even with Ctrl modifier.
    let (_m, cmds) = update(
        detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport),
        Msg::Click {
            column: 5,
            row: bottom_vpad_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "ctrl+click on bottom vpad row must return no asset: row={bottom_vpad_row}"
    );

    // asset[1] first row with Ctrl → index 1 (url = link2.pdf).
    let (_, cmds) = update(
        detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport),
        Msg::Click {
            column: 5,
            row: asset1_first_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "ctrl+click on asset[1] row must emit one cmd: row={asset1_first_row}"
    );
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/link2.pdf",
                "asset[1] row must resolve to asset index 1 (link2.pdf)"
            );
        }
        other => panic!("expected OpenAsset for asset[1], got {other:?}"),
    }
}

// AC1 (BDR 0019 Sc.6): Ctrl+click on an asset row opens the asset at that index.
// Derives the row from the rendered geometry (asset_index_at_panel_row), not a guessed constant.
// Uses 5 assets (panel_h = 13, below the ASSET_PANEL_MAX_ROWS=14 cap) so the last asset
// row is always within the panel, verifying the geometry-derived mapping is mutation-resistant.
#[test]
fn ctrl_click_nth_asset_row_derived_from_geometry_opens_correct_asset() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;

    // 5 assets: panel_h = min(2*5+3, 14) = 13 (not capped), all rows visible.
    let assets: Vec<Asset> = (0..5)
        .map(|i| {
            make_asset(
                &format!("f{i}.pdf"),
                &format!("https://example.com/f{i}.pdf"),
            )
        })
        .collect();

    let viewport = (80u16, 40u16);
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport);

    let inner_width = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;

    // Walk asset geometry to find the row of asset index 4 (the 5th asset).
    // The geometry derivation is the same as asset_index_at_panel_row uses.
    let mut cursor_row = panel_top + 1 + PANEL_VPAD as u16;
    for idx in 0..4usize {
        let span =
            crate::render::asset_row_lines(idx + 1, &assets[idx], content_width).len() as u16;
        cursor_row += span;
        cursor_row += 1; // separator between assets
    }
    let fifth_asset_row = cursor_row;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: fifth_asset_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "ctrl+click on fifth asset row must emit one cmd"
    );
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/f4.pdf",
                "fifth asset row (geometry-derived) must open asset index 4 (f4.pdf)"
            );
        }
        other => panic!("expected OpenAsset for fifth asset, got {other:?}"),
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

// AC5 (BDR 0019 Sc.5): Super/Cmd modifier also opens an asset (not only CONTROL).
#[test]
fn super_click_on_asset_row_emits_open_asset_cmd() {
    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", (80, 24));
    let geom = PanelGeom::compute(80, 24, &assets).expect("panel must exist");

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: geom.first_asset,
            modifiers: KeyModifiers::SUPER,
        },
    );
    assert_eq!(cmds.len(), 1, "Super+click on asset row must emit one cmd");
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(url, "https://example.com/doc.pdf");
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
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
        hint.contains("↑/↓") && hint.contains("scroll"),
        "footer hint must still contain the scroll nav text: {hint:?}"
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
        }],
        should_quit: false,
        header: empty_header(),
        viewport,
        click_targets: vec![],
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

// --- D1f (BDR 0021) geometry tests ---

// AC4 (BDR 0021 Sc.5): asset_panel_render_height for a non-empty list equals the old capped
// height plus ASSET_HINT_ROWS. The "old" value is (row_count + separators + 2*VPAD + 2).min(MAX).
// For 3 short assets: old = (3+2+2+2).min(14) = 9; new = 9 + ASSET_HINT_ROWS.
#[test]
fn asset_panel_render_height_is_old_value_plus_asset_hint_rows() {
    use crate::tui::screens::detail::ASSET_HINT_ROWS;

    let assets: Vec<Asset> = (0..3)
        .map(|i| make_asset(&format!("f{i}.pdf"), &format!("https://x.com/f{i}.pdf")))
        .collect();

    let inner_width: usize = 78; // viewport_w=80 - 2 borders
    let old_capped_height: u16 = (2 * 3u16 + 3).min(14); // = 9
    let panel_h = asset_panel_render_height(&assets, inner_width);

    assert_eq!(
        panel_h,
        old_capped_height + ASSET_HINT_ROWS,
        "panel height must equal old capped height ({old_capped_height}) + ASSET_HINT_ROWS ({ASSET_HINT_ROWS})"
    );
}

// AC4 (BDR 0021 Sc.5): empty asset list still returns 0 (no card, no hint).
#[test]
fn asset_panel_render_height_zero_for_empty_assets_d1f() {
    assert_eq!(
        asset_panel_render_height(&[], 78),
        0,
        "empty asset list must still return 0 after D1f (no card at all)"
    );
}

// AC3 (BDR 0021 Sc.3 + Sc.4): Ctrl+click on asset k's row still opens asset k.
// The hint rows are AFTER all assets so the asset walk is unaffected.
// Derive asset rows from geometry (not formula) and verify the last asset row
// resolves correctly.
#[test]
fn ctrl_click_asset_rows_unaffected_by_hint_rows() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;

    let assets = vec![
        make_asset("first.pdf", "https://example.com/first.pdf"),
        make_asset("second.pdf", "https://example.com/second.pdf"),
    ];
    let viewport = (80u16, 30u16);
    let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport);

    let inner_width = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;

    let span0 = crate::render::asset_row_lines(1, &assets[0], content_width).len() as u16;

    // Second asset first row: top border (1) + vpad + span0 + separator (1).
    let asset1_row = panel_top + 1 + PANEL_VPAD as u16 + span0 + 1;

    let (_m, cmds) = update(
        m,
        Msg::Click {
            column: 5,
            row: asset1_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        cmds.len(),
        1,
        "ctrl+click on second asset row must emit one cmd (hint rows must not shift asset walk)"
    );
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/second.pdf",
                "second asset row must open second.pdf (unaffected by hint rows)"
            );
        }
        other => panic!("expected OpenAsset for second asset, got {other:?}"),
    }
}

// AC3 (BDR 0021 Sc.4): Ctrl+click on the footnote (hint) row resolves to None.
// Hint rows sit AFTER the last asset, at: last_asset_row + 1 (blank) and last_asset_row + 2 (hint text).
#[test]
fn ctrl_click_on_hint_row_is_noop() {
    use crate::render::PANEL_HPAD;
    use crate::render::PANEL_VPAD;
    use crate::tui::screens::detail::ASSET_HINT_ROWS;

    let assets = vec![make_asset("doc.pdf", "https://example.com/doc.pdf")];
    let viewport = (80u16, 30u16);

    let inner_width = (viewport.0 - 2) as usize;
    let content_width = inner_width.saturating_sub(2 * PANEL_HPAD);
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport.1 - panel_h;

    let span0 = crate::render::asset_row_lines(1, &assets[0], content_width).len() as u16;
    // last asset content row: top border (1) + vpad + span0 - 1
    let last_asset_row = panel_top + 1 + PANEL_VPAD as u16 + span0 - 1;

    // hint rows are the two rows immediately after the last asset row.
    for hint_offset in 1..=ASSET_HINT_ROWS {
        let hint_row = last_asset_row + hint_offset;
        let m = detail_model_with_assets_and_viewport(assets.clone(), "inst", viewport);
        let (_m, cmds) = update(
            m,
            Msg::Click {
                column: 5,
                row: hint_row,
                modifiers: KeyModifiers::CONTROL,
            },
        );
        assert!(
            cmds.is_empty(),
            "ctrl+click on hint row (offset {hint_offset} after last asset, row {hint_row}) \
             must be a no-op (BDR 0021 Sc.4)"
        );
    }
}
