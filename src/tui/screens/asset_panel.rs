use crate::i18n::t;
use crate::render::{asset_row_lines, Asset, PANEL_HPAD, PANEL_VPAD};
use crate::tui::theme;
use ratatui::{
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Height ceiling that bounds the asset card; sized to clear a common spaced
/// multi-link card (4 rows + 3 separators + 2 vpad + 2 borders = 11).
const ASSET_PANEL_MAX_ROWS: u16 = 14;

/// Extra rows appended inside the Artifacts card after the asset list:
/// one blank separator row + one italic footnote hint line.
///
/// Placed AFTER the capped asset region so `index_at` returns None for them,
/// and clicks on these rows resolve to nothing.
pub const ASSET_HINT_ROWS: u16 = 2;

/// Width available for asset content rows inside the panel.
///
/// Removes `2 * PANEL_HPAD` from `panel_inner_width` so label text clears both
/// border insets. `panel_inner_width` is the panel area width minus 2 border
/// columns (i.e. the inner ratatui content width for the asset panel block).
pub fn asset_content_width(panel_inner_width: usize) -> usize {
    panel_inner_width.saturating_sub(2 * PANEL_HPAD)
}

/// A single interior row in the Artifacts panel, carrying its kind and wrapped text.
///
/// The layout is pure: no ratatui Style, no spans. Styling is applied at render time
/// by mapping the variant to the appropriate theme function.
pub enum PanelRow {
    Pad,
    Asset { idx: usize, text: String },
    Separator,
    Hint(String),
}

/// Pure interior composition of the Artifacts panel, top to bottom, before the cap and borders.
///
/// Returns an empty Vec when `assets` is empty (no panel is shown).
/// This is the single place that calls `asset_row_lines`, so the wrap count exists exactly once.
///
/// Layout order: top-pad(s), per-asset rows (with blank Separator between consecutive
/// assets), then Separator + Hint + bottom-pad(s) appended unconditionally. The height
/// function and the renderer both derive from this same sequence.
pub fn layout(assets: &[Asset], content_width: usize) -> Vec<PanelRow> {
    if assets.is_empty() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for _ in 0..PANEL_VPAD {
        rows.push(PanelRow::Pad);
    }
    for (i, asset) in assets.iter().enumerate() {
        if i > 0 {
            rows.push(PanelRow::Separator);
        }
        for text in asset_row_lines(i + 1, asset, content_width) {
            rows.push(PanelRow::Asset { idx: i, text });
        }
    }
    rows.push(PanelRow::Separator);
    rows.push(PanelRow::Hint(t("Ctrl/Cmd+click to open")));
    for _ in 0..PANEL_VPAD {
        rows.push(PanelRow::Pad);
    }
    rows
}

/// Apply the `ASSET_PANEL_MAX_ROWS` cap and return the interior rows to render.
///
/// Reproduces the semantics of the old `asset_panel_render_height` formula:
/// ```text
/// capped = min(row_count + seps_between + 2*PANEL_VPAD + 2_borders, ASSET_PANEL_MAX_ROWS)
/// height  = capped + ASSET_HINT_ROWS
/// ```
/// The body region (top-pad + asset rows + between-separators + bottom-pad) is sized to
/// `min(body, ASSET_PANEL_MAX_ROWS − 2)` interior rows; the trailing Separator + Hint +
/// bottom-pad are re-appended after the cap. When the body exceeds the cap the bottom-pad
/// is dropped (ratatui clips overflow anyway — this just keeps the vector honest).
pub fn apply_cap(rows: Vec<PanelRow>) -> Vec<PanelRow> {
    if rows.is_empty() {
        return rows;
    }
    let trailing_pads = PANEL_VPAD;
    let hint_block_size = ASSET_HINT_ROWS as usize;
    let body_end = rows.len().saturating_sub(hint_block_size + trailing_pads);
    let max_body = (ASSET_PANEL_MAX_ROWS as usize).saturating_sub(2);
    let capped_body_end = body_end.min(max_body);

    let mut result: Vec<PanelRow> = rows.into_iter().take(capped_body_end).collect();
    result.push(PanelRow::Separator);
    result.push(PanelRow::Hint(t("Ctrl/Cmd+click to open")));
    for _ in 0..PANEL_VPAD {
        result.push(PanelRow::Pad);
    }
    result
}

/// Wrapped panel height shared by the render and model paths.
///
/// Reproduces the authoritative formula from the old `asset_panel_render_height`:
/// `min(row_count + seps_between + 2*PANEL_VPAD + 2, ASSET_PANEL_MAX_ROWS) + ASSET_HINT_ROWS`.
/// Returns `0` when `assets` is empty (no card is drawn).
///
/// `inner_width` is the asset panel's inner content width, equal to the chunk
/// width minus 2 border columns (i.e. `area.width.saturating_sub(2)` from the
/// renderer chunk, or `viewport_cols.saturating_sub(2)` from the model).
pub fn height(assets: &[Asset], inner_width: usize) -> u16 {
    if assets.is_empty() {
        return 0;
    }
    let content_w = asset_content_width(inner_width);
    let row_count: usize = assets
        .iter()
        .enumerate()
        .map(|(i, asset)| asset_row_lines(i + 1, asset, content_w).len())
        .sum();
    let separators = assets.len().saturating_sub(1);
    let capped = (row_count as u16 + separators as u16 + 2 * PANEL_VPAD as u16 + 2)
        .min(ASSET_PANEL_MAX_ROWS);
    capped + ASSET_HINT_ROWS
}

/// Render the Artifacts panel into `area`.
///
/// Maps each `PanelRow` to a ratatui `Line`: `Pad` and `Separator` become blank
/// lines; `Asset` and `Hint` receive the leading `PANEL_HPAD` space plus the
/// appropriate theme style (asset_style or asset_hint_style). No Color or Style
/// literals appear here — all styling is via theme:: calls.
pub fn render(frame: &mut Frame, area: ratatui::layout::Rect, assets: &[Asset]) {
    let panel_title = format!(" {} ", t("Artifacts"));
    let panel_inner_width = area.width.saturating_sub(2) as usize;
    let content_w = asset_content_width(panel_inner_width);
    let hpad = " ".repeat(PANEL_HPAD);

    let rows = apply_cap(layout(assets, content_w));
    let lines: Vec<Line> = rows
        .into_iter()
        .map(|row| match row {
            PanelRow::Pad | PanelRow::Separator => Line::raw(""),
            PanelRow::Asset { text, .. } => Line::from(vec![
                Span::raw(hpad.clone()),
                Span::styled(text, theme::asset_style()),
            ]),
            PanelRow::Hint(text) => Line::from(vec![
                Span::raw(hpad.clone()),
                Span::styled(text, theme::asset_hint_style()),
            ]),
        })
        .collect();

    let panel = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(panel_title)
            .title_style(theme::header_style()),
    );
    frame.render_widget(panel, area);
}

/// Produce the inline asset section as styled content lines, derived from `layout()`.
///
/// Returns `Vec::new()` when `assets` is empty (no header is emitted for an empty list).
/// For non-empty assets the output is:
///
/// - row 0: the section header (`t("Artifacts")`) with a Bold `StyleRun` over the header text;
/// - rows 1..: each `PanelRow` from `layout(assets, content_width)` mapped 1:1 in order:
///   `Pad`/`Separator` → blank `""` with no style runs;
///   `Asset{text,..}` → `" ".repeat(PANEL_HPAD) + text` with no style runs
///   (link color is applied at render time via `asset_index_for_section_row`);
///   `Hint(text)` → `" ".repeat(PANEL_HPAD) + text` with an Italic `StyleRun` over the hint text.
///
/// Both this function and `asset_index_for_section_row` call `layout()` as the single
/// composition source so the header-offset contract (section row = layout row + 1) is
/// maintained in one place.
#[allow(dead_code)]
pub fn section_lines(
    assets: &[Asset],
    content_width: usize,
) -> Vec<(String, Vec<crate::render::StyleRun>)> {
    let rows = layout(assets, content_width);
    if rows.is_empty() {
        return Vec::new();
    }

    let header_text = t("Artifacts");
    let header_len = crate::render::display_width(&header_text);
    let header_run = crate::render::StyleRun {
        start: 0,
        len: header_len,
        style: crate::richtext::RichStyle::Bold,
    };

    let mut result: Vec<(String, Vec<crate::render::StyleRun>)> =
        Vec::with_capacity(1 + rows.len());
    result.push((header_text, vec![header_run]));

    let hpad = " ".repeat(PANEL_HPAD);
    for row in rows {
        let (text, runs) = match row {
            PanelRow::Pad | PanelRow::Separator => (String::new(), vec![]),
            PanelRow::Asset { text, .. } => (format!("{hpad}{text}"), vec![]),
            PanelRow::Hint(text) => {
                let hint_line = format!("{hpad}{text}");
                let hint_start = PANEL_HPAD;
                let hint_len = crate::render::display_width(&text);
                let run = crate::render::StyleRun {
                    start: hint_start,
                    len: hint_len,
                    style: crate::richtext::RichStyle::Italic,
                };
                (hint_line, vec![run])
            }
        };
        result.push((text, runs));
    }

    result
}

/// Map a 0-based row index into the `section_lines` vector to the owning asset index.
///
/// `interior_row == 0` is the header line and always returns `None`. For `interior_row >= 1`
/// the function classifies `layout(assets, content_width)[interior_row - 1]`:
/// `PanelRow::Asset{idx,..}` → `Some(idx)` (wrapped continuation lines share the same idx);
/// `Pad`/`Separator`/`Hint` → `None`. Out-of-range `interior_row` → `None`.
///
/// Both this function and `section_lines` call `layout()` as the single composition source,
/// so the header-offset invariant (section row = layout row + 1) is maintained in one place.
#[allow(dead_code)]
pub fn asset_index_for_section_row(
    assets: &[Asset],
    content_width: usize,
    interior_row: usize,
) -> Option<usize> {
    if interior_row == 0 {
        return None;
    }
    let rows = layout(assets, content_width);
    match rows.get(interior_row - 1) {
        Some(PanelRow::Asset { idx, .. }) => Some(*idx),
        _ => None,
    }
}

/// Map a screen row to the asset index at that position, or `None` for pad, separator, or hint rows.
///
/// `panel_top` is the screen row of the panel's top border. `row` is the screen row being clicked.
/// `viewport_rows` is the total terminal height. Returns `Some(idx)` only when the row maps to a
/// `PanelRow::Asset { idx, .. }` inside the capped interior.
pub fn index_at(
    assets: &[Asset],
    content_width: usize,
    panel_top: u16,
    row: u16,
    viewport_rows: u16,
) -> Option<usize> {
    let first_interior_row = panel_top + 1;
    let last_interior_row = viewport_rows.saturating_sub(2);
    if row < first_interior_row || row > last_interior_row {
        return None;
    }
    let interior_idx = (row - first_interior_row) as usize;
    let rows = apply_cap(layout(assets, content_width));
    match rows.get(interior_idx) {
        Some(PanelRow::Asset { idx, .. }) => Some(*idx),
        _ => None,
    }
}
