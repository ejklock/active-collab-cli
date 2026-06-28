use crate::i18n::t;
use crate::render::{asset_row_lines, Asset, PANEL_HPAD, PANEL_VPAD};

/// Width available for inline asset content rows in the global scrollable detail body.
///
/// `inner_width` is the detail content block's inner width (viewport_cols minus 2
/// border columns). Subtracts `PANEL_HPAD` for the left indent used by section_lines.
/// This is the ONE canonical width formula shared by the render splice (build_detail_content)
/// and the click hit-test (asset_panel_cmd_at), so wrap count and row→asset map cannot drift.
pub fn inline_content_width(inner_width: usize) -> usize {
    inner_width.saturating_sub(PANEL_HPAD)
}

/// A single interior row in the Artifacts panel, carrying its kind and wrapped text.
pub enum PanelRow {
    Pad,
    Asset { idx: usize, text: String },
    Separator,
    Hint(String),
}

/// Pure interior composition of the Artifacts panel, top to bottom.
///
/// Returns an empty Vec when `assets` is empty (no panel is shown).
/// This is the single place that calls `asset_row_lines`, so the wrap count exists exactly once.
///
/// Layout order: top-pad(s), per-asset rows (with blank Separator between consecutive
/// assets), then Separator + Hint + bottom-pad(s) appended unconditionally.
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

/// Produce the inline asset section as styled content lines, derived from `layout()`.
///
/// Returns `Vec::new()` when `assets` is empty (no header is emitted for an empty list).
/// For non-empty assets the output is:
///
/// - row 0: the section header (`t("Artifacts")`) with a Bold `StyleRun` over the header text;
/// - rows 1..: each `PanelRow` from `layout(assets, content_width)` mapped 1:1 in order:
///   `Pad`/`Separator` → blank `""` with no style runs;
///   `Asset{text,..}` → `" ".repeat(PANEL_HPAD) + text` with a Link `StyleRun` over the asset token
///   (layout emits link affordance structurally — it cannot be inferred from the visible text);
///   `Hint(text)` → `" ".repeat(PANEL_HPAD) + text` with an Italic `StyleRun` over the hint text.
///
/// Both this function and `asset_index_for_section_row` call `layout()` as the single
/// composition source so the header-offset contract (section row = layout row + 1) is
/// maintained in one place.
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
            PanelRow::Asset { text, .. } => {
                let asset_line = format!("{hpad}{text}");
                let run = crate::render::StyleRun {
                    start: PANEL_HPAD,
                    len: crate::render::display_width(&text),
                    style: crate::richtext::RichStyle::Link,
                };
                (asset_line, vec![run])
            }
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
