use crate::i18n::t;
use crate::render::{Asset, MineTableRow};
use serde_json::Value;
use std::collections::HashMap;

/// A row in the task list (shared by Projects and Tasks screens).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRow {
    pub task_id: i64,
    pub task_number: i64,
    pub name: String,
    pub instance: String,
    pub project_id: i64,
}

/// A project group returned by the controller.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectGroup {
    pub project_id: i64,
    pub project_name: String,
    pub tasks: Vec<TaskRow>,
}

/// Elm-style effect: what the shell should do after a pure update.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    LoadTasksByProject,
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
}

/// A screen on the navigation stack.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Projects {
        groups: Vec<ProjectGroup>,
        selected: usize,
        loading: bool,
    },
    Tasks {
        project_name: String,
        tasks: Vec<TaskRow>,
        selected: usize,
        loading: bool,
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
        /// Memoized rendered cache rebuilt by reflow_detail.
        ///
        /// Starts as vec![] and rendered_width = usize::MAX (not yet rendered at
        /// any real width). The shell calls reflow_detail before each draw.
        lines: Vec<String>,
        assets: Vec<Asset>,
        offset: usize,
        loading: bool,
        pending_download: bool,
        /// Width at which `lines` was last built; usize::MAX means "not yet built".
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
}

/// All messages the update function understands.
pub enum Msg {
    Up,
    Down,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    Click(usize),
    Select,
    Back,
    Quit,
    Refresh,
    LoadedTasksByProject(Vec<ProjectGroup>),
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
}

impl Model {
    /// Build the initial browse model: Projects screen in loading state.
    pub fn browse() -> Self {
        Model {
            stack: vec![Screen::Projects {
                groups: vec![],
                selected: 0,
                loading: true,
            }],
            should_quit: false,
        }
    }

    pub fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }

    fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }

    /// Rebuild the Detail screen's rendered line cache if the width changed.
    ///
    /// Guard clauses ensure this is a no-op when:
    /// - the top screen is not Detail
    /// - the Detail is still loading
    /// - the cache is already current for `inner_width`
    ///
    /// After a rebuild, `offset` is clamped so a resize that shortens content
    /// keeps the scroll position in range.
    pub fn reflow_detail(&mut self, inner_width: usize) {
        let Some(Screen::Detail {
            task,
            comments,
            user_map,
            lines,
            rendered_width,
            offset,
            loading,
            ..
        }) = self.top_mut()
        else {
            return;
        };

        if *loading {
            return;
        }

        if *rendered_width == inner_width {
            return;
        }

        *lines = crate::render::build_detail_lines(task, comments, user_map, inner_width);
        *rendered_width = inner_width;

        let max_offset = lines.len().saturating_sub(1);
        if *offset > max_offset {
            *offset = max_offset;
        }
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
        Msg::Click(row) => (handle_click(model, row), vec![]),
        Msg::Select => handle_select(model),
        Msg::Back => (handle_back(model), vec![]),
        Msg::Quit => (handle_quit(model), vec![]),
        Msg::Refresh => handle_refresh(model),
        Msg::LoadedTasksByProject(groups) => (handle_loaded_tasks(model, groups), vec![]),
        Msg::LoadedDetail(load) => (handle_loaded_detail(model, load), vec![]),
        Msg::UserMapResolved(map) => (handle_user_map_resolved(model, map), vec![]),
        Msg::AssetOpen(digit) => handle_asset_open(model, digit),
        Msg::TogglePendingDownload => (handle_toggle_pending_download(model), vec![]),
        Msg::AssetActionResult => (model, vec![]),
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
    match model.top_mut() {
        Some(Screen::Detail { offset, lines, .. }) => {
            let max_offset = lines.len().saturating_sub(1);
            *offset = (*offset + 1).min(max_offset);
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
    if let Some(Screen::Detail { offset, lines, .. }) = model.top_mut() {
        let max_offset = lines.len().saturating_sub(1);
        *offset = (*offset + PAGE_SIZE).min(max_offset);
    }
    model
}

fn handle_click(mut model: Model, row: usize) -> Model {
    match model.top_mut() {
        Some(Screen::Detail { .. }) => {}
        Some(screen) => {
            let count = screen.row_count();
            if count > 0 {
                screen.set_selected(row.min(count - 1));
            }
        }
        None => {}
    }
    model
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
                ref mut loading, ..
            } => {
                *loading = true;
                cmds.push(Cmd::LoadTasksByProject);
            }
        }
    }
    (model, cmds)
}

fn handle_loaded_tasks(mut model: Model, groups: Vec<ProjectGroup>) -> Model {
    if let Some(Screen::Projects {
        groups: ref mut g,
        loading: ref mut l,
        ..
    }) = model.top_mut()
    {
        *g = groups;
        *l = false;
    }
    model
}

fn handle_loaded_detail(mut model: Model, load: DetailLoad) -> Model {
    if let Some(Screen::Detail {
        task: ref mut t,
        comments: ref mut c,
        assets: ref mut a,
        user_map: ref mut um,
        lines: ref mut l,
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
        // Invalidate the render cache so reflow_detail rebuilds at current width.
        *l = vec![];
        *rw = usize::MAX;
    }
    model
}

fn handle_user_map_resolved(mut model: Model, map: HashMap<i64, String>) -> Model {
    if let Some(Screen::Detail {
        user_map: ref mut um,
        lines: ref mut l,
        rendered_width: ref mut rw,
        ..
    }) = model.top_mut()
    {
        *um = map;
        // Invalidate the render cache so the next reflow shows the updated assignee.
        *l = vec![];
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

/// Initial browse boot: emits Cmd::LoadTasksByProject and marks loading.
///
/// Called by the shell once at startup, not on every event — keeps the
/// shell minimal while the effect intent is declared in the pure layer.
pub fn init_browse() -> (Model, Vec<Cmd>) {
    let model = Model::browse();
    (model, vec![Cmd::LoadTasksByProject])
}

/// Build the initial mine model from already-fetched rows.
///
/// Rows are mapped directly to TaskRow; no LoadTasksByProject is emitted
/// because the caller already has the data. This seeds the shared TEA core
/// at the Tasks screen so Enter/click opens Detail exactly like browse.
pub fn mine_model(rows: Vec<MineTableRow>) -> Model {
    let tasks: Vec<TaskRow> = rows
        .into_iter()
        .map(|r| TaskRow {
            task_id: r.task_id,
            task_number: r.task_number,
            name: r.name,
            instance: r.instance,
            project_id: r.project_id,
        })
        .collect();

    Model {
        stack: vec![Screen::Tasks {
            project_name: t("My Tasks"),
            tasks,
            selected: 0,
            loading: false,
        }],
        should_quit: false,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/app.rs"]
mod tests;
