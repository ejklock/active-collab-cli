use crate::i18n::t;
use crate::render::{asset_row_lines, Asset, MineTableRow, StyleRun};
use crate::store::instances::Instance;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Identity bar shown at the top of every screen.
///
/// Built once at startup from the active instance list and the cached user
/// directory; name may be None when user_id is absent or not yet in the cache.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub name: Option<String>,
    pub email: String,
    pub instance: String,
    pub extra: usize,
}

impl Header {
    /// Build a Header from the active instance list and an optional display name.
    ///
    /// The first element is treated as the primary (active) instance.
    /// `extra` is the number of additional instances beyond the first.
    /// An empty slice produces empty strings with `extra = 0`.
    pub fn from_instances(instances: &[Instance], name: Option<String>) -> Header {
        match instances.first() {
            Some(inst) => Header {
                name,
                email: inst.email.clone(),
                instance: inst.name.clone(),
                extra: instances.len().saturating_sub(1),
            },
            None => Header {
                name,
                email: String::new(),
                instance: String::new(),
                extra: 0,
            },
        }
    }

    /// Format the identity line for rendering in the header bar.
    ///
    /// Produces `"NAME <email> · instance"` when name is Some, or
    /// `"<email> · instance"` when name is None.
    /// Appends `" (+N more)"` when extra > 0.
    pub fn header_line(&self) -> String {
        let base = match &self.name {
            Some(n) => format!("{} <{}> · {}", n, self.email, self.instance),
            None => format!("<{}> · {}", self.email, self.instance),
        };
        if self.extra > 0 {
            format!("{} (+{} more)", base, self.extra)
        } else {
            base
        }
    }
}

/// A row in the task list (shared by Projects and Tasks screens).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRow {
    pub task_id: i64,
    pub task_number: i64,
    pub name: String,
    pub instance: String,
    pub project_id: i64,
}

/// A project group returned by the controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroup {
    pub project_id: i64,
    pub project_name: String,
    pub instance: String,
    pub tasks: Vec<TaskRow>,
}

/// A y-range record mapping a rendered terminal row span to a list index.
///
/// Populated by `render_table` each frame and stored in the model so that
/// `handle_click_list` can resolve a click terminal-row → list index without
/// re-running geometry logic outside the render pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ClickTarget {
    /// First terminal row of this list item (inclusive).
    pub y_start: u16,
    /// One past the last terminal row of this list item (exclusive).
    pub y_end: u16,
    /// Zero-based index into the data list.
    pub index: usize,
}

/// Elm-style effect: what the shell should do after a pure update.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    LoadTasksByProject,
    /// Fetch the mine task list in the background (SWR revalidation for the mine TUI).
    LoadMineTasks,
    LoadDetail {
        instance: String,
        project_id: i64,
        task_id: i64,
        refresh: bool,
    },
    OpenAsset {
        instance: String,
        url: String,
    },
    DownloadAsset {
        instance: String,
        url: String,
        name: String,
    },
    /// Enable or disable crossterm mouse capture in the terminal.
    ///
    /// true = capture ON (normal browsing); false = capture OFF (native text selection).
    SetMouseCapture(bool),
}

/// Structured payload sent by spawn_load_detail phase 1.
///
/// Carries the raw task/comments/assets + whatever user_map was available
/// (cached or empty). The shell calls reflow_detail before each draw, so
/// the pure layer never touches terminal width.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailLoad {
    pub task: Value,
    pub comments: Vec<Value>,
    pub assets: Vec<Asset>,
    pub user_map: HashMap<i64, String>,
    /// ISO wall-clock string (YYYY-MM-DDTHH:MM:SSZ) stamped by the shell when the load completes.
    pub loaded_at: String,
}

/// A screen on the navigation stack.
///
/// The Detail variant is intentionally large — it holds the full render cache
/// for the open task. The stack is always shallow (≤ 3 elements) so heap
/// boxing of the large variant would add indirection with no practical benefit.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Screen {
    Projects {
        groups: Vec<ProjectGroup>,
        selected: usize,
        loading: bool,
        /// True while an in-flight revalidation fetch is running on a warm-seeded list.
        /// Distinct from `loading`: loading blanks the list; revalidating keeps it painted.
        revalidating: bool,
    },
    Tasks {
        project_name: String,
        tasks: Vec<TaskRow>,
        selected: usize,
        loading: bool,
        /// True while an in-flight revalidation fetch is running on a warm-seeded mine list.
        /// Distinct from `loading`: revalidating keeps rows painted while the refresh runs.
        revalidating: bool,
    },
    Detail {
        instance: String,
        project_id: i64,
        task_id: i64,
        /// Structured task JSON — source of truth for reflow.
        task: Value,
        /// Structured comments — source of truth for reflow.
        comments: Vec<Value>,
        /// User id→name map, updated on phase-2 resolution.
        user_map: HashMap<i64, String>,
        /// Memoized line cache rebuilt by reflow_detail.
        lines: Vec<String>,
        /// Parallel emphasis-style channel, index-aligned with `lines`.
        /// Each element holds the `StyleRun`s for the corresponding line.
        /// Populated exclusively by reflow_detail; initialized empty at construction.
        line_styles: Vec<Vec<StyleRun>>,
        assets: Vec<Asset>,
        offset: usize,
        loading: bool,
        pending_download: bool,
        /// Width at which the cache was last built; usize::MAX means "not yet built".
        rendered_width: usize,
    },
}

impl Screen {
    fn row_count(&self) -> usize {
        match self {
            Screen::Projects { groups, .. } => groups.len(),
            Screen::Tasks { tasks, .. } => tasks.len(),
            Screen::Detail { .. } => 0,
        }
    }

    fn selected(&self) -> usize {
        match self {
            Screen::Projects { selected, .. } => *selected,
            Screen::Tasks { selected, .. } => *selected,
            Screen::Detail { .. } => 0,
        }
    }

    fn set_selected(&mut self, value: usize) {
        match self {
            Screen::Projects { selected, .. } => *selected = value,
            Screen::Tasks { selected, .. } => *selected = value,
            Screen::Detail { .. } => {}
        }
    }

    fn is_loading(&self) -> bool {
        match self {
            Screen::Projects { loading, .. } => *loading,
            Screen::Tasks { loading, .. } => *loading,
            Screen::Detail { loading, .. } => *loading,
        }
    }
}

/// Page size for Detail screen scroll (PageUp/PageDown).
pub const PAGE_SIZE: usize = 10;

/// Rows consumed by the Detail content block's chrome that are not scrollable text.
/// Breakdown: 1 top border + 1 bottom border + 1 header bar row + 1 footer bar row.
const DETAIL_CHROME_ROWS: u16 = 4;

/// True maximum scroll offset for the Detail screen.
///
/// Uses the width-aware wrapped asset-panel height (same formula as `draw_detail`)
/// so the scroll clamp accounts for asset labels that wrap across multiple rows.
/// Reads only its arguments — no terminal, time, or async sources — so it is
/// safe to call from the pure TEA update loop.
///
/// `viewport_cols` is the full terminal width; `inner_width` passed to
/// `asset_panel_render_height` is derived as `viewport_cols.saturating_sub(2)`,
/// mirroring how `draw_detail` computes it.
///
/// When the viewport is too small to show any text rows the function clamps
/// `text_viewport_height` to 1 (guaranteeing max = lines_len - 1), which is
/// the least surprising bound and keeps model-only tests (viewport=(0,0))
/// consistent with render behaviour.
pub fn detail_max_offset(
    viewport_rows: u16,
    viewport_cols: u16,
    lines_len: usize,
    assets: &[Asset],
) -> usize {
    use crate::tui::screens::asset_panel_render_height;
    let inner_width = viewport_cols.saturating_sub(2) as usize;
    let panel_h = asset_panel_render_height(assets, inner_width);
    let raw = viewport_rows
        .saturating_sub(DETAIL_CHROME_ROWS)
        .saturating_sub(panel_h) as usize;
    let text_viewport_height = raw.max(1);
    lines_len.saturating_sub(text_viewport_height)
}

fn clamp_offset(offset: &mut usize, len: usize) {
    let max = len.saturating_sub(1);
    if *offset > max {
        *offset = max;
    }
}

enum SelectAction {
    PushTasks {
        project_name: String,
        tasks: Vec<TaskRow>,
    },
    PushDetail {
        instance: String,
        project_id: i64,
        task_id: i64,
    },
}

/// Determine the action to take when the user presses Select on the top screen.
///
/// Returns None when no action applies (e.g. empty list, Detail screen active).
fn select_action(stack: &[Screen]) -> Option<SelectAction> {
    match stack.last()? {
        Screen::Projects {
            groups, selected, ..
        } => {
            let group = groups.get(*selected)?;
            Some(SelectAction::PushTasks {
                project_name: group.project_name.clone(),
                tasks: group.tasks.clone(),
            })
        }
        Screen::Tasks {
            tasks, selected, ..
        } => {
            let row = tasks.get(*selected)?;
            Some(SelectAction::PushDetail {
                instance: row.instance.clone(),
                project_id: row.project_id,
                task_id: row.task_id,
            })
        }
        Screen::Detail { .. } => None,
    }
}

/// Digit keys 1–9 on the Detail screen.
fn digit_to_asset_index(c: char) -> Option<usize> {
    c.to_digit(10)
        .and_then(|d| if d >= 1 { Some((d - 1) as usize) } else { None })
}

/// Application model — the single source of truth for the pure TEA layer.
pub struct Model {
    pub stack: Vec<Screen>,
    pub should_quit: bool,
    pub header: Header,
    /// Terminal viewport size (cols, rows) written by the shell each frame before drawing.
    pub viewport: (u16, u16),
    /// Hit-map from the last render pass, written by the shell after `terminal.draw`.
    ///
    /// Each entry records the terminal y-range and list index of a visible data row.
    /// Empty until the first draw; stale during the frame that produces it (acceptable
    /// because clicks arrive in the following event loop iteration).
    pub click_targets: Vec<ClickTarget>,
    /// ISO wall-clock string (YYYY-MM-DDTHH:MM:SSZ) of when the currently-displayed data was loaded.
    /// None until the first load completes; stamped exclusively by the shell via Msg payloads.
    pub last_loaded: Option<String>,
    /// When true, mouse capture is OFF so the terminal can perform native text selection.
    /// Toggled by Msg::ToggleSelection; the shell reacts to Cmd::SetMouseCapture.
    pub selection_mode: bool,
}

/// All messages the update function understands.
pub enum Msg {
    Up,
    Down,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    Click {
        column: u16,
        row: u16,
    },
    Select,
    Back,
    Quit,
    Refresh,
    LoadedTasksByProject {
        groups: Vec<ProjectGroup>,
        /// ISO wall-clock string stamped by the shell when the load completed.
        loaded_at: String,
    },
    /// SWR revalidation completed for the mine task list.
    /// Replaces the current tasks, clears revalidating, and stamps last_loaded.
    LoadedMineTasks {
        rows: Vec<MineTableRow>,
        /// ISO wall-clock string stamped by the shell when the load completed.
        loaded_at: String,
    },
    /// Phase-1 load: structured data + cached (or empty) user_map.
    /// The shell calls reflow_detail before drawing to materialise lines.
    LoadedDetail(DetailLoad),
    /// Phase-2: fresh user directory resolved; invalidates the render cache
    /// so the next reflow shows the assignee name.
    UserMapResolved(HashMap<i64, String>),
    /// Digit 1–9 on the Detail screen.
    /// When pending_download is true, triggers download; otherwise opens in browser.
    AssetOpen(char),
    AssetActionResult,
    /// 'd' key: arm the download-next-digit flag on the Detail screen.
    TogglePendingDownload,
    /// 's' key: toggle selection mode (disables mouse capture for native text selection).
    ToggleSelection,
    /// Background user directory resolved a display name for the header.
    /// Sets model.header.name when it was previously absent.
    HeaderNameResolved(String),
}

impl Model {
    /// Build the initial browse model.
    ///
    /// When `seed` is `Some` with a non-empty list the Projects screen is painted
    /// immediately (`loading=false`, `revalidating=true`) so the user sees content
    /// before the revalidation fetch returns.  A cold start (`None` or empty seed)
    /// falls back to the classic `loading=true` placeholder.
    pub fn browse(header: Header, seed: Option<Vec<ProjectGroup>>) -> Self {
        let (groups, loading, revalidating) = match seed {
            Some(g) if !g.is_empty() => (g, false, true),
            _ => (vec![], true, false),
        };
        Model {
            stack: vec![Screen::Projects {
                groups,
                selected: 0,
                loading,
                revalidating,
            }],
            should_quit: false,
            header,
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection_mode: false,
        }
    }

    /// Replace the hit-map recorded by the last render pass.
    ///
    /// Called by the shell after `terminal.draw` completes, mirroring the
    /// `viewport` write pattern. Only the shell touches this field.
    pub fn set_click_targets(&mut self, targets: Vec<ClickTarget>) {
        self.click_targets = targets;
    }

    pub fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }

    fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }

    /// Rebuild the Detail screen's line cache if the width changed.
    ///
    /// Guard clauses ensure this is a no-op when the top screen is not Detail,
    /// is still loading, or the cache is already current for `inner_width`.
    ///
    /// After a rebuild, `offset` is clamped to `lines.len().saturating_sub(1)`
    /// so a resize that shortens content keeps the scroll position in range.
    pub fn reflow_detail(&mut self, inner_width: usize) {
        let Some(Screen::Detail {
            task,
            comments,
            user_map,
            lines,
            line_styles,
            rendered_width,
            offset,
            loading,
            ..
        }) = self.top_mut()
        else {
            return;
        };

        if *loading || *rendered_width == inner_width {
            return;
        }

        let content = crate::render::build_detail_content(task, comments, user_map, inner_width);
        *lines = content.lines;
        *line_styles = content.line_styles;
        *rendered_width = inner_width;

        clamp_offset(offset, lines.len());
    }
}

/// Pure update — returns new model and any effects to run.
///
/// All navigation, selection, and loader logic lives here so it is
/// headlessly unit-testable with no terminal or async runtime.
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::Up | Msg::ScrollUp => (handle_up(model), vec![]),
        Msg::Down | Msg::ScrollDown => (handle_down(model), vec![]),
        Msg::PageUp => (handle_page_up(model), vec![]),
        Msg::PageDown => (handle_page_down(model), vec![]),
        Msg::Click { column, row } => handle_click(model, column, row),
        Msg::Select => handle_select(model),
        Msg::Back => (handle_back(model), vec![]),
        Msg::Quit => (handle_quit(model), vec![]),
        Msg::Refresh => handle_refresh(model),
        Msg::LoadedTasksByProject { groups, loaded_at } => {
            (handle_loaded_tasks(model, groups, loaded_at), vec![])
        }
        Msg::LoadedMineTasks { rows, loaded_at } => {
            (handle_loaded_mine_tasks(model, rows, loaded_at), vec![])
        }
        Msg::LoadedDetail(load) => (handle_loaded_detail(model, load), vec![]),
        Msg::UserMapResolved(map) => (handle_user_map_resolved(model, map), vec![]),
        Msg::AssetOpen(digit) => handle_asset_open(model, digit),
        Msg::TogglePendingDownload => (handle_toggle_pending_download(model), vec![]),
        Msg::ToggleSelection => handle_toggle_selection(model),
        Msg::AssetActionResult => (model, vec![]),
        Msg::HeaderNameResolved(name) => (handle_header_name_resolved(model, name), vec![]),
    }
}

fn handle_up(mut model: Model) -> Model {
    match model.top_mut() {
        Some(Screen::Detail { offset, .. }) => {
            *offset = offset.saturating_sub(1);
        }
        Some(screen) => {
            let sel = screen.selected();
            screen.set_selected(sel.saturating_sub(1));
        }
        None => {}
    }
    model
}

fn handle_down(mut model: Model) -> Model {
    let (viewport_cols, viewport_rows) = model.viewport;
    match model.top_mut() {
        Some(Screen::Detail {
            offset,
            lines,
            assets,
            ..
        }) => {
            let max = detail_max_offset(viewport_rows, viewport_cols, lines.len(), assets);
            *offset = (*offset + 1).min(max);
        }
        Some(screen) => {
            let count = screen.row_count();
            if count > 0 {
                let sel = screen.selected();
                screen.set_selected((sel + 1).min(count - 1));
            }
        }
        None => {}
    }
    model
}

fn handle_page_up(mut model: Model) -> Model {
    if let Some(Screen::Detail { offset, .. }) = model.top_mut() {
        *offset = offset.saturating_sub(PAGE_SIZE);
    }
    model
}

fn handle_page_down(mut model: Model) -> Model {
    let (viewport_cols, viewport_rows) = model.viewport;
    if let Some(Screen::Detail {
        offset,
        lines,
        assets,
        ..
    }) = model.top_mut()
    {
        let max = detail_max_offset(viewport_rows, viewport_cols, lines.len(), assets);
        *offset = (*offset + PAGE_SIZE).min(max);
    }
    model
}

fn handle_click(model: Model, column: u16, row: u16) -> (Model, Vec<Cmd>) {
    match model.top() {
        Some(Screen::Detail { .. }) => handle_click_detail(model, column, row),
        Some(_) => handle_click_list(model, row),
        None => (model, vec![]),
    }
}

fn handle_click_list(model: Model, row: u16) -> (Model, Vec<Cmd>) {
    let Some(target) = model
        .click_targets
        .iter()
        .find(|t| row >= t.y_start && row < t.y_end)
        .cloned()
    else {
        return (model, vec![]);
    };

    let mut model = model;
    if let Some(screen) = model.top_mut() {
        screen.set_selected(target.index);
    }
    handle_select(model)
}

fn handle_click_detail(model: Model, column: u16, row: u16) -> (Model, Vec<Cmd>) {
    if let Some(cmd) = body_link_cmd_at(&model, column, row) {
        return (model, vec![cmd]);
    }
    asset_panel_cmd_at(model, row)
}

/// Try to resolve a body-link click in the Detail content text area.
///
/// Resolves the URL from the visible text at the clicked display column via
/// `url_at`. On a bracketed bare email address, re-adds the `mailto:` scheme.
/// Returns `None` when the click lands outside the text viewport, on a border,
/// on padding, on plain text, or on a `[note]` that is not a URL/email.
///
/// Uses the wrapped asset-panel height (same as `draw_detail`) so the body
/// hit region stops above the real panel top even when asset labels wrap.
fn body_link_cmd_at(model: &Model, column: u16, row: u16) -> Option<Cmd> {
    use crate::tui::screens::asset_panel_render_height;

    let Screen::Detail {
        instance,
        assets,
        lines,
        offset,
        ..
    } = model.top()?
    else {
        return None;
    };

    let (viewport_cols, viewport_rows) = model.viewport;
    let text_top: u16 = 2;
    let inner_width = viewport_cols.saturating_sub(2) as usize;
    let panel_h = asset_panel_render_height(assets, inner_width);
    let content_text_height =
        viewport_rows.saturating_sub(DETAIL_CHROME_ROWS.saturating_add(panel_h));

    if row < text_top || row >= text_top + content_text_height {
        return None;
    }

    let logical_line = offset + (row - text_top) as usize;
    let line = lines.get(logical_line)?;
    let char_col = (column as usize).saturating_sub(1);
    let token = crate::render::url_at(line, char_col)?;
    let url = normalize_link_url(&token);
    if !crate::render::is_openable_url(&url) && !is_mailto_url(&url) {
        return None;
    }
    Some(Cmd::OpenAsset {
        instance: instance.clone(),
        url,
    })
}

/// Prepend `mailto:` when `token` is a bare email (contains `@`, no scheme).
fn normalize_link_url(token: &str) -> String {
    if token.contains('@') && !token.contains("://") && !token.starts_with("mailto:") {
        format!("mailto:{token}")
    } else {
        token.to_string()
    }
}

/// Return true for `mailto:` URLs (not caught by `is_openable_url` which accepts only http/https).
fn is_mailto_url(url: &str) -> bool {
    url.starts_with("mailto:")
}

/// Try to resolve an asset-panel click in the Detail screen.
///
/// Uses the width-aware wrapped panel height (same as `draw_detail`) so that:
///   - the panel's top row is computed correctly when asset labels wrap,
///   - a click on any continuation row of a wrapped label resolves to the
///     owning asset rather than the following (mis-shifted) one.
///
/// Returns the appropriate `Cmd` (open or download) when the click lands on
/// an asset row inside the panel. Returns `(model, vec![])` for any click that
/// misses the panel or falls on a border row.
fn asset_panel_cmd_at(model: Model, row: u16) -> (Model, Vec<Cmd>) {
    use crate::tui::screens::asset_panel_render_height;

    let (viewport_cols, viewport_rows) = model.viewport;

    let Some(Screen::Detail {
        instance,
        assets,
        pending_download,
        ..
    }) = model.top()
    else {
        return (model, vec![]);
    };

    if assets.is_empty() {
        return (model, vec![]);
    }

    let inner_width = viewport_cols.saturating_sub(2) as usize;
    let panel_h = asset_panel_render_height(assets, inner_width);
    if panel_h == 0 {
        return (model, vec![]);
    }

    let panel_top = viewport_rows.saturating_sub(panel_h);
    let first_asset_row = panel_top + 1;
    // last_asset_row is the last row before the bottom border
    let last_asset_row = viewport_rows.saturating_sub(2);

    if row < first_asset_row || row > last_asset_row {
        return (model, vec![]);
    }

    // Map the clicked row to an asset by walking wrapped row spans.
    // panel_inner_width mirrors the subtraction inside asset_panel_render_height.
    let panel_inner_width = inner_width.saturating_sub(2);
    let panel_row = (row - first_asset_row) as usize;

    let mut cursor = 0usize;
    let mut found_asset: Option<Asset> = None;
    for (i, asset) in assets.iter().enumerate() {
        let span = asset_row_lines(i + 1, asset, panel_inner_width).len();
        if panel_row < cursor + span {
            found_asset = Some(asset.clone());
            break;
        }
        cursor += span;
    }

    let Some(asset) = found_asset else {
        return (model, vec![]);
    };

    let cmd = if *pending_download {
        Cmd::DownloadAsset {
            instance: instance.clone(),
            url: asset.url.clone(),
            name: asset.name.clone(),
        }
    } else {
        Cmd::OpenAsset {
            instance: instance.clone(),
            url: asset.url.clone(),
        }
    };

    let mut model = model;
    if let Some(Screen::Detail {
        pending_download, ..
    }) = model.top_mut()
    {
        *pending_download = false;
    }

    (model, vec![cmd])
}

fn handle_select(mut model: Model) -> (Model, Vec<Cmd>) {
    let mut cmds = vec![];
    if let Some(action) = select_action(&model.stack) {
        match action {
            SelectAction::PushTasks {
                project_name,
                tasks,
            } => {
                model.stack.push(Screen::Tasks {
                    project_name,
                    tasks,
                    selected: 0,
                    loading: false,
                    revalidating: false,
                });
            }
            SelectAction::PushDetail {
                instance,
                project_id,
                task_id,
            } => {
                cmds.push(Cmd::LoadDetail {
                    instance: instance.clone(),
                    project_id,
                    task_id,
                    refresh: false,
                });
                model.stack.push(Screen::Detail {
                    instance,
                    project_id,
                    task_id,
                    task: Value::Null,
                    comments: vec![],
                    user_map: HashMap::new(),
                    lines: vec![],
                    line_styles: vec![],
                    assets: vec![],
                    offset: 0,
                    loading: true,
                    pending_download: false,
                    rendered_width: usize::MAX,
                });
            }
        }
    }
    (model, cmds)
}

fn handle_back(mut model: Model) -> Model {
    if model.stack.len() <= 1 {
        model.should_quit = true;
    } else {
        model.stack.pop();
    }
    model
}

fn handle_quit(mut model: Model) -> Model {
    model.should_quit = true;
    model
}

fn handle_refresh(mut model: Model) -> (Model, Vec<Cmd>) {
    let mut cmds = vec![];
    if let Some(top) = model.top_mut() {
        if top.is_loading() {
            // Single-flight guard: drop the duplicate refresh while one is in flight.
            return (model, cmds);
        }
        match top {
            Screen::Detail {
                instance,
                project_id,
                task_id,
                ref mut loading,
                ref mut offset,
                ..
            } => {
                *loading = true;
                *offset = 0;
                cmds.push(Cmd::LoadDetail {
                    instance: instance.clone(),
                    project_id: *project_id,
                    task_id: *task_id,
                    refresh: true,
                });
            }
            Screen::Projects {
                ref mut loading, ..
            } => {
                *loading = true;
                cmds.push(Cmd::LoadTasksByProject);
            }
            Screen::Tasks {
                ref mut loading,
                ref mut revalidating,
                ..
            } => {
                *loading = true;
                *revalidating = false;
                cmds.push(Cmd::LoadTasksByProject);
            }
        }
    }
    (model, cmds)
}

fn handle_loaded_tasks(mut model: Model, groups: Vec<ProjectGroup>, loaded_at: String) -> Model {
    if let Some(Screen::Projects {
        groups: ref mut g,
        loading: ref mut l,
        revalidating: ref mut rv,
        ..
    }) = model.top_mut()
    {
        *g = groups;
        *l = false;
        *rv = false;
    }
    model.last_loaded = Some(loaded_at);
    model
}

fn handle_loaded_mine_tasks(mut model: Model, rows: Vec<MineTableRow>, loaded_at: String) -> Model {
    if let Some(Screen::Tasks {
        tasks: ref mut t,
        loading: ref mut l,
        revalidating: ref mut rv,
        ..
    }) = model.top_mut()
    {
        *t = rows_to_task_rows(rows);
        *l = false;
        *rv = false;
    }
    model.last_loaded = Some(loaded_at);
    model
}

fn rows_to_task_rows(rows: Vec<MineTableRow>) -> Vec<TaskRow> {
    rows.into_iter()
        .map(|r| TaskRow {
            task_id: r.task_id,
            task_number: r.task_number,
            name: r.name,
            instance: r.instance,
            project_id: r.project_id,
        })
        .collect()
}

fn handle_loaded_detail(mut model: Model, load: DetailLoad) -> Model {
    if let Some(Screen::Detail {
        task: ref mut t,
        comments: ref mut c,
        assets: ref mut a,
        user_map: ref mut um,
        lines: ref mut ls,
        line_styles: ref mut lss,
        rendered_width: ref mut rw,
        ref mut loading,
        ..
    }) = model.top_mut()
    {
        *t = load.task;
        *c = load.comments;
        *a = load.assets;
        *um = load.user_map;
        *loading = false;
        // Invalidate line cache so reflow_detail rebuilds at current width.
        *ls = vec![];
        *lss = vec![];
        *rw = usize::MAX;
    }
    model.last_loaded = Some(load.loaded_at);
    model
}

fn handle_user_map_resolved(mut model: Model, map: HashMap<i64, String>) -> Model {
    if let Some(Screen::Detail {
        user_map: ref mut um,
        lines: ref mut ls,
        line_styles: ref mut lss,
        rendered_width: ref mut rw,
        ..
    }) = model.top_mut()
    {
        *um = map;
        // Invalidate line cache so the next reflow shows the updated assignee.
        *ls = vec![];
        *lss = vec![];
        *rw = usize::MAX;
    }
    model
}

fn handle_asset_open(model: Model, digit: char) -> (Model, Vec<Cmd>) {
    let mut cmds = vec![];
    if let Some(Screen::Detail {
        instance,
        assets,
        pending_download,
        ..
    }) = model.top()
    {
        let is_download = *pending_download;
        let instance = instance.clone();
        if let Some(idx) = digit_to_asset_index(digit) {
            if let Some(asset) = assets.get(idx) {
                if is_download {
                    cmds.push(Cmd::DownloadAsset {
                        instance,
                        url: asset.url.clone(),
                        name: asset.name.clone(),
                    });
                } else {
                    cmds.push(Cmd::OpenAsset {
                        instance,
                        url: asset.url.clone(),
                    });
                }
            }
        }
    }
    let mut model = model;
    if let Some(Screen::Detail {
        pending_download, ..
    }) = model.top_mut()
    {
        *pending_download = false;
    }
    (model, cmds)
}

fn handle_toggle_pending_download(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        pending_download, ..
    }) = model.top_mut()
    {
        *pending_download = !*pending_download;
    }
    model
}

fn handle_toggle_selection(mut model: Model) -> (Model, Vec<Cmd>) {
    model.selection_mode = !model.selection_mode;
    let capture_on = !model.selection_mode;
    (model, vec![Cmd::SetMouseCapture(capture_on)])
}

fn handle_header_name_resolved(mut model: Model, name: String) -> Model {
    model.header.name = Some(name);
    model
}

/// Initial browse boot: emits Cmd::LoadTasksByProject and marks loading (or revalidating).
///
/// Accepts an optional seed snapshot so the shell can pass a warm cache entry.
/// A non-empty seed seeds the Projects list immediately while always dispatching
/// Cmd::LoadTasksByProject for revalidation.  A cold start (None or empty) produces
/// the classic loading placeholder.
///
/// Called by the shell once at startup, not on every event — keeps the
/// shell minimal while the effect intent is declared in the pure layer.
pub fn init_browse(header: Header, seed: Option<Vec<ProjectGroup>>) -> (Model, Vec<Cmd>) {
    let model = Model::browse(header, seed);
    (model, vec![Cmd::LoadTasksByProject])
}

/// Build the initial mine model and the init commands for the mine TUI.
///
/// Accepts an optional snapshot seed so the shell can pass a warm cache entry.
/// A non-empty seed paints the task list immediately (`loading=false`,
/// `revalidating=true`) while revalidation runs in the background.
/// A cold start (`None` or empty) falls back to `loading=true`.
///
/// ALWAYS emits `Cmd::LoadMineTasks` so the mine list is revalidated on every
/// entry, regardless of whether a snapshot was present.
pub fn init_mine(header: Header, seed: Option<Vec<MineTableRow>>) -> (Model, Vec<Cmd>) {
    let (tasks, loading, revalidating) = match seed {
        Some(rows) if !rows.is_empty() => (rows_to_task_rows(rows), false, true),
        _ => (vec![], true, false),
    };
    let model = Model {
        stack: vec![Screen::Tasks {
            project_name: t("My Tasks"),
            tasks,
            selected: 0,
            loading,
            revalidating,
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection_mode: false,
    };
    (model, vec![Cmd::LoadMineTasks])
}

#[cfg(test)]
#[path = "../../tests/unit/app.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/model.rs"]
mod model_tests;
