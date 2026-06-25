use crate::i18n::t;
use crate::render::Asset;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// A row in the task list (shared by Projects and Tasks screens).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRow {
    pub task_id: i64,
    pub task_number: i64,
    pub name: String,
    pub instance: String,
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
        lines: Vec<String>,
        assets: Vec<Asset>,
        offset: usize,
        loading: bool,
        pending_download: bool,
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
    LoadedDetail(Vec<String>, Vec<Asset>),
    /// Digit 1–9 on the Detail screen.
    /// When pending_download is true, triggers download; otherwise opens in browser.
    AssetOpen(char),
    AssetActionResult(String),
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

    fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }

    fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }
}

/// Pure update — returns new model and any effects to run.
///
/// All navigation, selection, and loader logic lives here so it is
/// headlessly unit-testable with no terminal or async runtime.
pub fn update(mut model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    let mut cmds: Vec<Cmd> = vec![];
    match msg {
        Msg::Up | Msg::ScrollUp => match model.top_mut() {
            Some(Screen::Detail { offset, .. }) => {
                *offset = offset.saturating_sub(1);
            }
            Some(screen) => {
                let sel = screen.selected();
                screen.set_selected(sel.saturating_sub(1));
            }
            None => {}
        },
        Msg::Down | Msg::ScrollDown => match model.top_mut() {
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
        },
        Msg::PageUp => {
            if let Some(Screen::Detail { offset, .. }) = model.top_mut() {
                *offset = offset.saturating_sub(PAGE_SIZE);
            }
        }
        Msg::PageDown => {
            if let Some(Screen::Detail { offset, lines, .. }) = model.top_mut() {
                let max_offset = lines.len().saturating_sub(1);
                *offset = (*offset + PAGE_SIZE).min(max_offset);
            }
        }
        Msg::Click(row) => match model.top_mut() {
            Some(Screen::Detail { .. }) => {}
            Some(screen) => {
                let count = screen.row_count();
                if count > 0 {
                    screen.set_selected(row.min(count - 1));
                }
            }
            None => {}
        },
        Msg::Select => {
            if let Some(detail_push) = select_action(&model.stack) {
                match detail_push {
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
                            lines: vec![],
                            assets: vec![],
                            offset: 0,
                            loading: true,
                            pending_download: false,
                        });
                    }
                }
            }
        }
        Msg::Back => {
            if model.stack.len() <= 1 {
                model.should_quit = true;
            } else {
                model.stack.pop();
            }
        }
        Msg::Quit => {
            model.should_quit = true;
        }
        Msg::Refresh => {
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
        }
        Msg::LoadedTasksByProject(groups) => {
            if let Some(Screen::Projects {
                groups: ref mut g,
                loading: ref mut l,
                ..
            }) = model.top_mut()
            {
                *g = groups;
                *l = false;
            }
        }
        Msg::LoadedDetail(lines, assets) => {
            if let Some(Screen::Detail {
                lines: ref mut l,
                assets: ref mut a,
                ref mut loading,
                ..
            }) = model.top_mut()
            {
                *l = lines;
                *a = assets;
                *loading = false;
            }
        }
        Msg::AssetOpen(digit) => {
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
            if let Some(Screen::Detail {
                pending_download, ..
            }) = model.top_mut()
            {
                *pending_download = false;
            }
        }
        Msg::TogglePendingDownload => {
            if let Some(Screen::Detail {
                pending_download, ..
            }) = model.top_mut()
            {
                *pending_download = !*pending_download;
            }
        }
        Msg::AssetActionResult(_msg) => {}
    }
    (model, cmds)
}

/// Page size for Detail screen scroll (PageUp/PageDown).
const PAGE_SIZE: usize = 10;

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
            let task_id = row.task_id;
            let instance = row.instance.clone();
            let project_id = resolve_project_id(stack);
            Some(SelectAction::PushDetail {
                instance,
                project_id,
                task_id,
            })
        }
        Screen::Detail { .. } => None,
    }
}

/// Walk the stack below the Tasks screen to find the selected project's id.
fn resolve_project_id(stack: &[Screen]) -> i64 {
    for screen in stack.iter().rev() {
        if let Screen::Projects {
            groups, selected, ..
        } = screen
        {
            if let Some(group) = groups.get(*selected) {
                return group.project_id;
            }
        }
    }
    0
}

/// Initial browse boot: emits Cmd::LoadTasksByProject and marks loading.
///
/// Called by the shell once at startup, not on every event — keeps the
/// shell minimal while the effect intent is declared in the pure layer.
pub fn init_browse() -> (Model, Vec<Cmd>) {
    let model = Model::browse();
    (model, vec![Cmd::LoadTasksByProject])
}

/// Render the top screen into the terminal frame.
pub fn view(model: &Model, frame: &mut Frame) {
    let Some(screen) = model.top() else { return };

    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    match screen {
        Screen::Projects {
            groups,
            selected,
            loading,
            ..
        } => {
            draw_projects(frame, chunks[0], groups, *selected, *loading);
        }
        Screen::Tasks {
            project_name,
            tasks,
            selected,
            loading,
            ..
        } => {
            draw_tasks(frame, chunks[0], project_name, tasks, *selected, *loading);
        }
        Screen::Detail {
            lines,
            assets,
            offset,
            loading,
            task_id,
            ..
        } => {
            draw_detail(frame, chunks[0], lines, assets, *offset, *loading, *task_id);
        }
    }

    let footer_text = match screen {
        Screen::Detail { assets, .. } if !assets.is_empty() => {
            t("↑/↓ scroll  Esc/b back  q quit  1-9 open asset  d+1-9 download")
        }
        _ => t("↑/↓ navigate  Enter select  Esc/b back  q quit"),
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

fn draw_projects(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    groups: &[ProjectGroup],
    selected: usize,
    loading: bool,
) {
    if loading {
        let msg = Paragraph::new(t("Loading tasks…")).block(
            Block::default()
                .borders(Borders::ALL)
                .title(t(" Projects ")),
        );
        frame.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let style = highlight_style(i == selected);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:>4}  ", g.tasks.len()), style),
                Span::styled(g.project_name.clone(), style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(t(" Projects ")),
        )
        .highlight_style(Style::default());

    use ratatui::widgets::StatefulWidget;
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(selected));
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut state);
}

fn draw_tasks(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    project_name: &str,
    tasks: &[TaskRow],
    selected: usize,
    loading: bool,
) {
    let title = format!(" {} ", project_name);

    if loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = tasks
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let style = highlight_style(i == selected);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:>5}  ", row.task_number), style),
                Span::styled(format!("{:<15}  ", row.instance), style),
                Span::styled(row.name.clone(), style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default());

    use ratatui::widgets::StatefulWidget;
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(selected));
    StatefulWidget::render(list, area, frame.buffer_mut(), &mut state);
}

fn draw_detail(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    lines: &[String],
    _assets: &[Asset],
    offset: usize,
    loading: bool,
    task_id: i64,
) {
    let title = format!(" {} #{} ", t("Task"), task_id);

    if loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    let visible_height = area.height.saturating_sub(2) as usize;
    let visible: Vec<Line> = lines
        .iter()
        .skip(offset)
        .take(visible_height)
        .map(|l| Line::from(l.clone()))
        .collect();

    let paragraph =
        Paragraph::new(visible).block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(paragraph, area);
}

fn highlight_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

/// A legacy flat-task model used by `run_mine` (R5 TEA core).
pub struct Task {
    pub project: String,
    pub id: u32,
    pub name: String,
}

/// A flat-task model built from a provided list (mine runner).
pub struct FlatModel {
    pub tasks: Vec<Task>,
    pub selected: usize,
    pub should_quit: bool,
}

/// Messages accepted by the flat mine model.
pub enum FlatMsg {
    Up,
    Down,
    ScrollUp,
    ScrollDown,
    Click(usize),
    Quit,
}

impl FlatModel {
    pub fn with_tasks(tasks: Vec<Task>) -> Self {
        FlatModel {
            tasks,
            selected: 0,
            should_quit: false,
        }
    }
}

/// Pure update for the flat mine model (used by run_mine / R5).
pub fn update_flat(mut model: FlatModel, msg: FlatMsg) -> FlatModel {
    match msg {
        FlatMsg::Up | FlatMsg::ScrollUp => {
            model.selected = model.selected.saturating_sub(1);
        }
        FlatMsg::Down | FlatMsg::ScrollDown => {
            if !model.tasks.is_empty() {
                model.selected = (model.selected + 1).min(model.tasks.len() - 1);
            }
        }
        FlatMsg::Click(row) => {
            if !model.tasks.is_empty() {
                model.selected = row.min(model.tasks.len() - 1);
            }
        }
        FlatMsg::Quit => {
            model.should_quit = true;
        }
    }
    model
}

/// Render the flat mine model (used by run_mine / R5).
pub fn view_flat(model: &FlatModel, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let items: Vec<ListItem> = model
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let style = highlight_style(i == model.selected);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:>3} ", task.id), style),
                Span::styled(format!("{:<20} ", task.project), style),
                Span::styled(task.name.clone(), style),
            ]))
        })
        .collect();

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(model.selected));

    let list = ratatui::widgets::List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ActiveCollab Tasks "),
        )
        .highlight_style(Style::default());

    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    let footer = Paragraph::new("↑/↓ navigate  click to select  q quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
