use crate::render::AffordanceKind;
use crate::tui::model::{ImageAssetRef, Model, Screen};

/// The result of resolving a Ctrl/Cmd+click in the Detail screen.
///
/// Returned by `resolve_detail_click`; the caller maps each variant to the
/// appropriate TEA effect without performing any coordinate math itself.
///
/// `ViewImage` is produced by `target_for` from an `AffordanceKind::ViewImage`
/// span (ADR 0065 §1) and consumed by `apply_detail_click_target`, which opens
/// the viewer overlay.
pub enum DetailClickTarget {
    CommentEdit(i64),
    CommentDelete(i64),
    OpenUrl(String),
    OpenAsset(String),
    ViewImage(ImageAssetRef),
}

/// Translate a viewport click coordinate to a scroll-aware `(line_idx, char_col)`
/// within the Detail text area.
///
/// Returns `None` when:
/// - the top screen is not a Detail screen,
/// - `row` falls outside the scrollable body area (delegated to `detail_geometry::row_to_line_idx`),
/// - the resolved `line_idx` is past the end of `lines` (the guard that
///   `affordance_at` previously lacked, closing AC5).
fn viewport_to_line_col(model: &Model, column: u16, row: u16) -> Option<(usize, usize)> {
    let Screen::Detail { lines, offset, .. } = model.top()? else {
        return None;
    };

    let (_, viewport_rows) = model.viewport;
    let line_idx = crate::tui::detail_geometry::row_to_line_idx(*offset, viewport_rows, row)?;
    if line_idx >= lines.len() {
        return None;
    }

    Some((line_idx, column as usize))
}

/// Resolve a Ctrl/Cmd+click in the Detail screen to a typed `DetailClickTarget`.
///
/// Returns `None` when:
/// - `has_modifier` is false (plain click is reserved for text selection),
/// - the top screen is not a Detail screen,
/// - the click falls outside the text viewport,
/// - the resolved `line_idx` is past `lines.len()` (bounds guard),
/// - no affordance span covers the coordinate.
///
/// The asset whole-row rule (ADR 0029) is implemented via `AffordanceKind::is_row_target`:
/// an `OpenAsset` affordance matches any column on its line, while all other kinds
/// require the click to fall within `[col_start, col_end)`.
///
/// Pure: no Model mutation, no Cmd construction, no I/O.
pub fn resolve_detail_click(
    model: &Model,
    column: u16,
    row: u16,
    has_modifier: bool,
) -> Option<DetailClickTarget> {
    if !has_modifier {
        return None;
    }

    let (line_idx, char_col) = viewport_to_line_col(model, column, row)?;

    let Screen::Detail { affordances, .. } = model.top()? else {
        return None;
    };

    let aff = affordances.iter().find(|a| {
        a.line_idx == line_idx
            && (a.kind.is_row_target() || (char_col >= a.col_start && char_col < a.col_end))
    })?;

    target_for(&aff.kind)
}

/// Map an `AffordanceKind` to its typed `DetailClickTarget`.
///
/// Returns `None` for any kind that does not produce a navigable target
/// (currently all five kinds do produce one, but `None` keeps the contract
/// open without requiring an unreachable arm).
///
/// Pure: no Model mutation, no Cmd construction.
fn target_for(kind: &AffordanceKind) -> Option<DetailClickTarget> {
    match kind {
        AffordanceKind::Edit(id) => Some(DetailClickTarget::CommentEdit(*id)),
        AffordanceKind::Delete(id) => Some(DetailClickTarget::CommentDelete(*id)),
        AffordanceKind::OpenUrl(url) => Some(DetailClickTarget::OpenUrl(url.clone())),
        AffordanceKind::OpenAsset(url) => Some(DetailClickTarget::OpenAsset(url.clone())),
        AffordanceKind::ViewImage { url, label } => {
            Some(DetailClickTarget::ViewImage(ImageAssetRef {
                url: url.clone(),
                label: label.clone(),
            }))
        }
    }
}
